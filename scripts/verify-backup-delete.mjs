// Isolated UI adapter: never accesses real backup files, database or WebDAV.
import assert from 'node:assert/strict';
import { mkdirSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { chromium } from '../.tools/browser-check/node_modules/playwright/index.mjs';
const base = process.env.PREVIEW_URL || 'http://127.0.0.1:1420';
assert(['127.0.0.1', 'localhost'].includes(new URL(base).hostname));
const evidence = resolve('.tools/backup-delete-evidence');
mkdirSync(evidence, { recursive: true });
const browser = await chromium.launch({
  headless: true,
  executablePath:
    process.env.BROWSER_EXECUTABLE ||
    '/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge',
});
try {
  const page = await browser.newPage({
    viewport: { width: 1200, height: 1000 },
  });
  const errors = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.addInitScript(() => {
    window.isTauri = true;
    window.requests = [];
    window.failDelete = false;
    window.blockDelete = false;
    window.backupState = {
      running: false,
      pending_restore: false,
      phase: 'idle',
      message: '',
      last_backup: null,
      cloud_status: '',
      last_upload_at: null,
    };
    window.backups = [1, 2].map((n) => ({
      backup_id: `fixture-${n}`,
      path: `/fixture/backup-${n}.gmbak`,
      created_at: `2026-10-04T04:29:5${n}+08:00`,
      reason: 'manual',
      size_bytes: 1024,
      sha256: 'a'.repeat(64),
      categories: ['metadata'],
      encrypted: n === 2,
    }));
    window.__TAURI_INTERNALS__ = {
      invoke: async (command, args) => {
        const { request } = args;
        window.requests.push({ command, payload: request.payload });
        let data;
        if (command === 'get_backup_settings')
          data = {
            categories: ['metadata'],
            directory: '/fixture',
            automatic: false,
            on_startup: false,
            on_game_exit: false,
            interval_minutes: 0,
            retention: 10,
            upload_local: false,
            webdav_url: '',
            webdav_directory: '',
            webdav_username: '',
            has_backup_password: false,
            has_webdav_password: false,
          };
        else if (command === 'list_application_backups') data = window.backups;
        else if (command === 'get_application_backup_status')
          data = window.backupState;
        else if (command === 'preview_application_restore')
          data = {
            preview_id: 'fixture-preview',
            confirmation_token: 'fixture-token',
            created_at: window.backups[0].created_at,
            categories: ['metadata'],
            game_count: 1,
            attachment_count: 0,
            size_bytes: 1024,
            save_paths: {},
          };
        else if (command === 'delete_application_backup') {
          if (window.blockDelete)
            await new Promise((done) => {
              window.finishDelete = done;
            });
          if (window.failDelete)
            return {
              success: false,
              error_code: 'PERMISSION_DENIED',
              request_id: request.request_id,
              message: '备份文件被占用，未删除。',
              data: null,
            };
          window.backups = window.backups.filter(
            (v) => v.path !== request.payload.path,
          );
          data = {
            trash_path: `/fixture/.trash/deleted/${request.payload.backup_id}.gmbak`,
          };
        } else throw Error(`Unexpected fixture command: ${command}`);
        return {
          success: true,
          request_id: request.request_id,
          error_code: null,
          message: 'fixture',
          data: structuredClone(data),
        };
      },
    };
  });
  await page.route('**/__backup_delete_test', (route) =>
    route.fulfill({
      contentType: 'text/html',
      body: `<html data-theme="light"><head><meta charset="utf-8"></head><body><main id="test-root"></main><script type="module">
    import { createApp, h } from '/node_modules/.vite/deps/vue.js';
    import Panel from '/src/components/ApplicationBackup.vue';
    import Feedback from '/src/components/preview/PreviewFeedback.vue';
    import '/src/style.css'; import '/src/appearance.css'; import '/src/settings.css';
    createApp({ render: () => h('div', { class: 'settings-v1', style: 'padding: 24px; max-width: 1100px; margin: auto' }, [h(Panel), h(Feedback)]) }).mount('#test-root');
    </script></body></html>`,
    }),
  );
  await page.goto(`${base}/__backup_delete_test`);
  const rows = page.locator('.settings-backup-history .settings-backup-item');
  await rows.nth(1).waitFor();
  const firstDelete = rows.first().getByRole('button', { name: /删除/ });
  const dialog = page.getByRole('dialog');
  await firstDelete.click();
  await dialog.waitFor();
  assert.match(await dialog.innerText(), /backup-1.gmbak.*\.trash.*云端副本/);
  await dialog.getByRole('button', { name: '取消', exact: true }).click();
  assert.equal(await rows.count(), 2);
  assert.equal(
    await page.evaluate(
      () =>
        window.requests.filter((r) => r.command === 'delete_application_backup')
          .length,
    ),
    0,
  );
  await firstDelete.click();
  await page.keyboard.press('Escape');
  await dialog.waitFor({ state: 'hidden' });
  assert.equal(
    await page.evaluate(
      () =>
        window.requests.filter((r) => r.command === 'delete_application_backup')
          .length,
    ),
    0,
  );
  await rows
    .first()
    .getByRole('button', { name: '恢复预览', exact: true })
    .click();
  await page.getByRole('button', { name: '关闭预览', exact: true }).waitFor();
  assert(await firstDelete.isDisabled());
  await page.getByRole('button', { name: '关闭预览', exact: true }).click();
  await page.evaluate(() => {
    window.failDelete = true;
  });
  await firstDelete.click();
  await dialog
    .getByRole('button', { name: '确认删除备份', exact: true })
    .click();
  await page.getByRole('alert').filter({ hasText: '备份文件被占用' }).waitFor();
  assert.equal(await rows.count(), 2);
  const confirmation = await page.evaluate(() =>
    window.requests
      .filter((r) => r.command === 'delete_application_backup')
      .at(-1),
  );
  assert.deepEqual(confirmation.payload, {
    backup_id: 'fixture-1',
    path: '/fixture/backup-1.gmbak',
    confirmed: true,
  });
  await page.evaluate(() => {
    window.failDelete = false;
    window.blockDelete = true;
  });
  await firstDelete.click();
  await dialog
    .getByRole('button', { name: '确认删除备份', exact: true })
    .click();
  await page.waitForFunction(() => typeof window.finishDelete === 'function');
  assert(await firstDelete.isDisabled());
  assert(await rows.last().getByRole('button', { name: /删除/ }).isDisabled());
  await page.evaluate(() => {
    window.finishDelete();
  });
  await page.waitForFunction(
    () =>
      document.querySelectorAll(
        '.settings-backup-history .settings-backup-item',
      ).length === 1,
  );
  assert.match(await rows.first().innerText(), /密码加密/);
  await page.getByText('备份已移入回收目录：', { exact: false }).waitFor();
  await page.locator('.modal-scrim').waitFor({ state: 'hidden' });
  await rows
    .first()
    .screenshot({ path: resolve(evidence, 'backup-row-light.png') });
  await page.evaluate(() => {
    document.documentElement.dataset.theme = 'dark';
  });
  await rows
    .first()
    .screenshot({ path: resolve(evidence, 'backup-row-dark.png') });
  await page.setViewportSize({ width: 390, height: 900 });
  await rows.first().scrollIntoViewIfNeeded();
  assert.equal(
    await page.evaluate(
      () => document.documentElement.scrollWidth > window.innerWidth,
    ),
    false,
  );
  await rows
    .first()
    .screenshot({ path: resolve(evidence, 'backup-row-narrow.png') });
  await page.evaluate(() => {
    window.backupState.pending_restore = true;
  });
  await page.waitForFunction(
    () =>
      [...document.querySelectorAll('button')].find(
        (v) => v.textContent.trim() === '删除',
      )?.disabled,
  );
  assert(await rows.first().getByRole('button', { name: /删除/ }).isDisabled());
  await page.evaluate(() => {
    window.backupState.pending_restore = false;
    window.backupState.running = true;
  });
  await page.waitForFunction(
    () =>
      [...document.querySelectorAll('button')].find(
        (v) => v.textContent.trim() === '恢复预览',
      )?.disabled,
  );
  assert(await rows.first().getByRole('button', { name: /删除/ }).isDisabled());
  assert.deepEqual(errors, []);
  writeFileSync(
    resolve(evidence, 'result.json'),
    JSON.stringify(
      {
        passed: true,
        cases: [
          'cancel and Escape do not delete',
          'restore preview blocks delete',
          'failure preserves row',
          'confirmation binds ID and path',
          'in-flight controls disabled',
          'success removes selected row',
          'light/dark/narrow layout',
          'pending restore and running task protected',
        ],
        errors,
      },
      null,
      2,
    ),
  );
  console.log(`备份删除浏览器隔离回归通过：${evidence}`);
} finally {
  await browser.close();
}
