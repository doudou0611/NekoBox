// Isolated account UI regression: no user credentials, DB, or remote writes.
import assert from 'node:assert/strict';
import { mkdir } from 'node:fs/promises';
import { chromium } from '../.tools/browser-check/node_modules/playwright/index.mjs';

const base = process.env.PREVIEW_URL || 'http://127.0.0.1:1420';
assert(['127.0.0.1', 'localhost'].includes(new URL(base).hostname));
const browser = await chromium.launch({
  headless: true,
  executablePath:
    process.env.BROWSER_EXECUTABLE ||
    '/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge',
});
let checks = 0;
try {
  const page = await browser.newPage({
    viewport: { width: 1280, height: 1000 },
  });
  const errors = [];
  page.on('pageerror', (error) => errors.push(error.message));
  const source = await (
    await page.request.get(`${base}/src/components/AccountDialog.vue`)
  ).text();
  const path = (name) => source.match(new RegExp(`from "(${name}[^"]*)"`))?.[1];
  const vue = path('/node_modules/\\.vite/deps/vue\\.js');
  const bangumi = path('/src/stores/bangumi\\.ts');
  const hikari = path('/src/stores/hikarinagiAccount\\.ts');
  const sync = path('/src/stores/accountSync\\.ts');
  assert(vue && bangumi && hikari && sync);
  const syncSource = await (await page.request.get(`${base}${sync}`)).text();
  const operations = syncSource.match(
    /from "(\/src\/stores\/operations\.ts[^"]*)"/,
  )?.[1];
  assert(operations);
  await page.addInitScript(() => {
    window.isTauri = true;
    window.requests = [];
    window.windows = [];
    window.pendingSync = [];
    window.loginPending = false;
    window.loginIndex = 0;
    const signedOut = {
      status: 'signed_out',
      profile: null,
      message: '尚未登录。',
    };
    window.account = signedOut;
    const authenticated = {
      status: 'authenticated',
      profile: {
        id: 27,
        username: 'fixture-hikari',
        nickname: '测试账户',
        avatar_url: null,
      },
      message: '已登录自己的 Hikarinagi 账户。',
    };
    window.__TAURI_INTERNALS__ = {
      metadata: {
        currentWindow: { label: 'main' },
        currentWebview: { label: 'main' },
      },
      transformCallback: () => 1,
      unregisterCallback: () => {},
      convertFileSrc: (value) => value,
      invoke: async (command, args = {}) => {
        if (command === 'plugin:event|listen') return 1;
        if (command.startsWith('plugin:event|')) return;
        if (command === 'plugin:webview|create_webview_window') {
          window.windows.push(args.options);
          return;
        }
        if (command === 'plugin:window|get_all_windows')
          return window.windows.map((w) => w.label);
        if (command === 'plugin:window|close') {
          window.requests.push({ command, payload: args });
          return;
        }
        const request = args.request;
        window.requests.push({ command, payload: request.payload });
        const success = (data) => ({
          success: true,
          error_code: null,
          message: '隔离测试',
          request_id: request.request_id,
          data,
        });
        if (command === 'hikarinagi_account') return success(window.account);
        if (command === 'begin_hikarinagi_login') {
          window.windows.push({
            label: `hikarinagi-login-fixture-${window.loginIndex + 1}`,
            url: 'https://id.hikarinagi.org/oidc/auth?client_id=fixture-app',
          });
          return success({
            flow_id: `fixture-${++window.loginIndex}`,
            authorization_url:
              'https://id.hikarinagi.org/oidc/auth?client_id=fixture-app&code_challenge=fixture&state=fixture',
            expires_in_seconds: 180,
          });
        }
        if (command === 'poll_hikarinagi_login') {
          if (!window.loginPending) window.account = authenticated;
          return success({
            flow_id: request.payload.flow_id,
            status: window.loginPending ? 'pending' : 'completed',
            message: window.loginPending ? '等待授权' : '已完成授权',
            account: window.loginPending ? null : authenticated,
          });
        }
        if (command === 'cancel_hikarinagi_login') return success(true);
        if (command === 'logout_hikarinagi') {
          window.account = signedOut;
          return success(signedOut);
        }
        if (command === 'sync_account_play_data')
          return new Promise((resolve) => {
            window.pendingSync.push(() =>
              resolve(
                success({
                  processed: 2,
                  synced: 1,
                  skipped: 2,
                  failed: 1,
                  failures: [{ title: '测试作品', message: '隔离服务失败' }],
                  message: '同步已结束',
                }),
              ),
            );
          });
        throw Error(`Unexpected account fixture command ${command}`);
      },
    };
  });
  await page.route('**/__accounts_workflow_test', (route) =>
    route.fulfill({
      contentType: 'text/html',
      body: `<html><head><meta charset="utf-8"></head><body><div id="test-root"></div><script type="module">
      import { createApp,h,ref } from '${vue}';
      import AccountDialog from '/src/components/AccountDialog.vue';
      import OperationCenter from '/src/components/OperationCenter.vue';
      import { bangumi } from '${bangumi}';
      import { hikariAccount } from '${hikari}';
      import { operations } from '${operations}';
      import '/src/style.css'; import '/src/appearance.css';
      bangumi.account={status:'authenticated',profile:{id:14,username:'fixture-bgm',nickname:'测试 Bangumi',avatar_url:null},message:'已登录 Bangumi。'};
      window.fixtureHikari=hikariAccount; window.fixtureOperations=operations;
      const open=ref(true);
      createApp({render:()=>h('main',{},[h('button',{id:'outside',onClick:()=>{open.value=true}},'重新打开账户'),h(OperationCenter),open.value?h(AccountDialog,{onClose:()=>{open.value=false}}):null])}).mount('#test-root');
    </script></body></html>`,
    }),
  );
  await page.goto(`${base}/__accounts_workflow_test`);
  const dialog = page.locator('dialog.account-dialog[open]');
  await dialog.waitFor();
  const bounds = await dialog.boundingBox();
  assert(bounds && bounds.x > 100 && bounds.y > 0);
  checks++;
  assert.equal(
    await page
      .getByRole('tab', { name: 'Bangumi' })
      .getAttribute('aria-selected'),
    'true',
  );
  assert.equal(await page.getByRole('tabpanel').count(), 1);
  assert.equal(await page.getByRole('dialog').count(), 1);
  checks += 3;
  await page.getByRole('tab', { name: 'Bangumi' }).focus();
  await page.keyboard.press('ArrowRight');
  await page
    .getByRole('heading', { name: '登录 Hikarinagi', exact: true })
    .waitFor();
  assert.equal(
    await page
      .getByRole('tab', { name: 'Hikarinagi' })
      .getAttribute('aria-selected'),
    'true',
  );
  assert.equal(
    await page.evaluate(() => document.activeElement.id),
    'account-tab-hikarinagi',
  );
  assert.equal(
    await page
      .getByRole('button', { name: '同步游玩数据', exact: true })
      .isDisabled(),
    true,
  );
  assert.notEqual(
    await page
      .locator('.account-tab-indicator')
      .evaluate((node) => getComputedStyle(node).transitionDuration),
    '0s',
  );
  checks += 4;
  await page
    .getByRole('button', { name: '登录 Hikarinagi 账户', exact: true })
    .click();
  await page.getByText('@fixture-hikari · ID 27', { exact: true }).waitFor();
  const login = await page.evaluate(() => window.windows[0]);
  assert.equal(new URL(login.url).hostname, 'id.hikarinagi.org');
  assert.equal(/secret|verifier|access_token/.test(login.url), false);
  await page.waitForFunction(() =>
    window.requests.some((r) => r.command === 'plugin:window|close'),
  );
  assert.equal(
    await page
      .getByRole('button', { name: '同步游玩数据', exact: true })
      .isDisabled(),
    false,
  );
  checks += 4;
  await page.getByRole('button', { name: '同步游玩数据', exact: true }).click();
  assert.equal(
    await page.evaluate(
      () =>
        window.requests.filter((r) => r.command === 'sync_account_play_data')
          .length,
    ),
    0,
  );
  await page
    .getByRole('button', { name: '确认同步到此账户', exact: true })
    .click();
  await page.waitForFunction(() => window.pendingSync.length === 1);
  assert.deepEqual(
    await page.evaluate(
      () =>
        window.requests.find((r) => r.command === 'sync_account_play_data')
          .payload,
    ),
    { provider: 'hikarinagi', confirmed: true },
  );
  assert.equal(
    await page
      .getByRole('button', { name: '退出登录', exact: true })
      .isDisabled(),
    true,
  );
  await page.getByRole('button', { name: '关闭账户窗口' }).click();
  await page.getByRole('button', { name: '打开后台任务' }).click();
  assert.equal(
    await page.locator('.operation-popover li[data-status="running"]').count(),
    1,
  );
  await page.evaluate(() => window.pendingSync.shift()());
  await page.locator('.operation-popover li[data-status="failed"]').waitFor();
  assert.equal(
    await page
      .locator('.operation-popover li[data-status="completed"]')
      .count(),
    0,
  );
  assert.match(
    await page.locator('.operation-popover').innerText(),
    /成功 1 项，失败 1 项，跳过 2 项/,
  );
  checks += 6;
  await page.getByRole('button', { name: '打开后台任务' }).click();
  await page.locator('#outside').click();
  await page.getByRole('tab', { name: 'Hikarinagi' }).click();
  await page
    .getByText('成功 1 项 · 失败 1 项 · 跳过 2 项', { exact: true })
    .waitFor();
  await page.getByText('查看失败原因', { exact: true }).click();
  await page.getByText('测试作品：隔离服务失败', { exact: true }).waitFor();
  assert.equal(await page.evaluate(() => window.fixtureOperations.length), 1);
  checks += 2;
  await mkdir('.tools/account-evidence', { recursive: true });
  await page.screenshot({
    path: '.tools/account-evidence/hikarinagi-desktop.png',
  });
  await page.getByRole('button', { name: '退出登录', exact: true }).click();
  await page
    .getByRole('heading', { name: '登录 Hikarinagi', exact: true })
    .waitFor();
  await page.evaluate(() => {
    window.loginPending = true;
  });
  await page
    .getByRole('button', { name: '登录 Hikarinagi 账户', exact: true })
    .click();
  await page.getByRole('button', { name: '取消登录', exact: true }).click();
  assert.equal(await page.evaluate(() => window.fixtureHikari.flow_id), '');
  assert.equal(await page.evaluate(() => window.fixtureHikari.busy), false);
  assert.equal(
    await page.evaluate(() =>
      window.requests.some((r) => r.command === 'cancel_hikarinagi_login'),
    ),
    true,
  );
  checks += 3;
  await page.setViewportSize({ width: 390, height: 844 });
  await page.emulateMedia({ reducedMotion: 'reduce', colorScheme: 'dark' });
  await page.evaluate(() => {
    document.documentElement.dataset.motion = 'reduced';
    document.documentElement.dataset.theme = 'dark';
  });
  await page.getByRole('tab', { name: 'Bangumi' }).focus();
  await page.keyboard.press('End');
  await page
    .getByRole('heading', { name: '登录 Hikarinagi', exact: true })
    .waitFor();
  assert.equal(
    await page
      .locator('.account-tab-indicator')
      .evaluate((node) => getComputedStyle(node).transitionDuration),
    '0s',
  );
  assert.equal(
    await dialog.evaluate((node) => node.scrollWidth <= node.clientWidth + 1),
    true,
  );
  assert.equal(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= window.innerWidth,
    ),
    true,
  );
  assert.equal(
    await page.evaluate(() => document.activeElement.id),
    'account-tab-hikarinagi',
  );
  for (let i = 0; i < 12; i++) {
    await page.keyboard.press('Tab');
    assert.equal(
      await dialog.evaluate((node) => node.contains(document.activeElement)),
      true,
    );
  }
  checks += 5;
  await page.screenshot({
    path: '.tools/account-evidence/hikarinagi-mobile-reduced.png',
  });
  await page.keyboard.press('Escape');
  await page.waitForFunction(() => document.querySelector('dialog') === null);
  assert.deepEqual(errors, []);
  checks += 2;
  await page.route('**/__review_login_test', (route) =>
    route.fulfill({
      contentType: 'text/html',
      body: `<html><head><meta charset="utf-8"></head><body><div id="test-root"></div><script type="module">
      import { createApp,h,ref } from '${vue}';
      import AccountDialog from '/src/components/AccountDialog.vue';
      import ReviewDialog from '/src/components/HikarinagiReviewDialog.vue';
      import { accountDialog } from '/src/stores/accountDialog.ts';
      import '/src/style.css'; import '/src/appearance.css';
      const review=ref();
      createApp({render:()=>h('main',{},[h('button',{id:'review-opener',onClick:()=>review.value.open()},'打开作品评分'),h(ReviewDialog,{ref:review,gameId:'fixture-game',remoteId:'1'}),accountDialog.open?h(AccountDialog,{initialProvider:accountDialog.provider,onClose:()=>{accountDialog.open=false}}):null])}).mount('#test-root');
    </script></body></html>`,
    }),
  );
  await page.goto(`${base}/__review_login_test`);
  await page.getByRole('button', { name: '打开作品评分' }).click();
  await page
    .getByRole('button', { name: '登录 Hikarinagi 账户', exact: true })
    .click();
  await page.locator('dialog.account-dialog[open]').waitFor();
  assert.equal(
    await page
      .getByRole('tab', { name: 'Hikarinagi', exact: true })
      .getAttribute('aria-selected'),
    'true',
  );
  assert.equal(new URL(page.url()).pathname, '/__review_login_test');
  assert.equal(
    await page.evaluate(() =>
      window.requests.some(
        (r) =>
          r.command === 'submit_hikarinagi_review' ||
          r.command === 'begin_hikarinagi_login',
      ),
    ),
    false,
  );
  await page.getByRole('button', { name: '关闭账户窗口' }).click();
  await page.getByRole('button', { name: '打开作品评分' }).click();
  await page
    .getByRole('button', { name: '登录 Hikarinagi 账户', exact: true })
    .waitFor();
  assert.deepEqual(errors, []);
  checks += 5;
  await page.close();
  console.log(
    `账户与同步浏览器回归：${checks} 项通过（隔离 IPC；非真实登录或 Windows 验收）。`,
  );
} finally {
  await browser.close();
}
