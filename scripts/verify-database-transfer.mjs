// Isolated component regression; the adapter never reads or writes a production database.
import assert from 'node:assert/strict';
import { mkdirSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { chromium } from '../.tools/browser-check/node_modules/playwright/index.mjs';
const base = process.env.PREVIEW_URL || 'http://127.0.0.1:1420';
assert(['127.0.0.1', 'localhost'].includes(new URL(base).hostname));
const browser = await chromium.launch({
  headless: true,
  executablePath:
    process.env.BROWSER_EXECUTABLE ||
    '/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge',
});
const evidence = resolve(
  '.tools/database-transfer-evidence',
  String(Date.now()),
);
mkdirSync(evidence, { recursive: true });
try {
  const page = await browser.newPage({
    viewport: { width: 1200, height: 900 },
  });
  const errors = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.addInitScript(() => {
    window.isTauri = true;
    window.testPending = false;
    window.testConfirmFails = false;
    window.testDialogCancelled = false;
    window.testRequests = [];
    window.__TAURI_INTERNALS__ = {
      invoke: async (command, args) => {
        if (command === 'plugin:dialog|open') {
          return window.testDialogCancelled
            ? null
            : args.options.directory
              ? '/fixture/export'
              : '/fixture/backup.sqlite3';
        }
        const { request } = args;
        window.testRequests.push({ command, payload: request.payload });
        let data;
        if (command === 'database_transfer_status')
          data = {
            pending_restart: window.testPending,
            last_import_error: null,
          };
        else if (command === 'export_database')
          data = {
            path: '/fixture/export/backup.sqlite3',
            sha256: 'a'.repeat(64),
            size_bytes: 1024,
          };
        else if (command === 'preview_database_import')
          data = {
            confirmation_token: 'fixture-token',
            sha256: 'b'.repeat(64),
            expires_at: '2099-01-01T00:00:00Z',
            counts: {
              games: 10,
              installations: 12,
              collections: 2,
              sessions: 30,
            },
          };
        else if (command === 'confirm_database_import') {
          if (window.testConfirmFails)
            return {
              success: false,
              error_code: 'CONFLICT',
              request_id: request.request_id,
              message: '确认已过期，请重新预览',
              data: null,
            };
          window.testPending = true;
          data = { pending_restart: true, last_import_error: null };
        } else if (command === 'cancel_database_import') {
          window.testPending = false;
          data = { pending_restart: false, last_import_error: null };
        } else throw Error(`Unexpected test command: ${command}`);
        return {
          success: true,
          error_code: null,
          request_id: request.request_id,
          message: 'fixture',
          data,
        };
      },
    };
  });
  await page.route('**/__database_component_test', (route) =>
    route.fulfill({
      contentType: 'text/html',
      body: `<html><head><meta charset="utf-8"></head><body><main id="test-root"></main><script type="module">
    import { createApp } from '/node_modules/.vite/deps/vue.js';
    import Panel from '/src/components/DatabaseTransfer.vue';
    import '/src/style.css'; import '/src/appearance.css';
    createApp(Panel).mount('#test-root');
    </script></body></html>`,
    }),
  );
  await page.goto(`${base}/__database_component_test`);
  await page.getByRole('button', { name: '导出数据库', exact: true }).click();
  await page.getByText('已保存：/fixture/export/backup.sqlite3').waitFor();
  await page.evaluate(() => {
    window.testDialogCancelled = true;
  });
  const beforeCancel = await page.evaluate(
    () =>
      window.testRequests.filter((r) => r.command === 'preview_database_import')
        .length,
  );
  await page.getByRole('button', { name: '导入数据库', exact: true }).click();
  assert.equal(
    await page.evaluate(
      () =>
        window.testRequests.filter(
          (r) => r.command === 'preview_database_import',
        ).length,
    ),
    beforeCancel,
  );
  await page.evaluate(() => {
    window.testDialogCancelled = false;
  });
  await page.getByRole('button', { name: '导入数据库', exact: true }).click();
  const dialog = page.getByRole('dialog');
  await dialog.waitFor();
  assert.match(
    await dialog.innerText(),
    /10 个作品、12 个安装、2 个分组和 30 次/,
  );
  await page.keyboard.press('Escape');
  await dialog.waitFor({ state: 'hidden' });
  assert.equal(await page.evaluate(() => window.testPending), false);
  await page.getByRole('button', { name: '导入数据库', exact: true }).click();
  await page.evaluate(() => {
    window.testConfirmFails = true;
  });
  await dialog.getByRole('button', { name: '确认导入，重启后生效' }).click();
  await dialog.getByRole('alert').waitFor();
  assert.equal(await page.evaluate(() => window.testPending), false);
  await page.evaluate(() => {
    window.testConfirmFails = false;
  });
  await dialog.getByRole('button', { name: '确认导入，重启后生效' }).click();
  await dialog.waitFor({ state: 'hidden' });
  await page.getByRole('status').waitFor();
  assert(
    await page
      .getByRole('button', { name: '导入数据库', exact: true })
      .isDisabled(),
  );
  const confirmation = await page.evaluate(() =>
    window.testRequests
      .filter((r) => r.command === 'confirm_database_import')
      .at(-1),
  );
  assert.deepEqual(confirmation.payload, {
    confirmation_token: 'fixture-token',
    confirmed: true,
  });
  await page.getByRole('button', { name: '取消待导入任务' }).click();
  assert.equal(await page.evaluate(() => window.testPending), false);
  assert.deepEqual(errors, []);
  await page.screenshot({ path: resolve(evidence, 'database-transfer.png') });
  writeFileSync(
    resolve(evidence, 'result.json'),
    JSON.stringify(
      {
        passed: true,
        cases: [
          'export',
          'picker cancellation',
          'preview counts',
          'Esc cancellation',
          'failed confirmation',
          'confirmed restart',
          'cancel staged import',
        ],
        windows_tested: false,
        real_ipc_tested: false,
      },
      null,
      2,
    ),
  );
  console.log(`数据库组件隔离回归通过：${evidence}`);
} finally {
  await browser.close();
}
