// Isolated browser workflow with synthetic IPC; no credentials or remote game downloads.
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
const out = '.tools/hikarifield-browser';
await mkdir(out, { recursive: true });
let checks = 0;
try {
  const page = await browser.newPage({
    viewport: { width: 1280, height: 900 },
  });
  const errors = [];
  page.on('pageerror', (e) => {
    errors.push(e.message);
    console.error('Browser error:', e.message);
  });
  page.setDefaultTimeout(15000);
  const src = await (
    await page.request.get(`${base}/src/components/AccountDialog.vue`)
  ).text();
  const vue = src.match(
    /from "(\/node_modules\/\.vite\/deps\/vue\.js[^"]*)"/,
  )?.[1];
  assert(vue);
  const hfStore = src.match(
    /from "(\/src\/stores\/hikariField\.ts[^"]*)"/,
  )?.[1];
  assert(hfStore);
  const storeSource = await (
    await page.request.get(`${base}${hfStore}`)
  ).text();
  const libraryStore = storeSource.match(
    /from "(\/src\/stores\/library\.ts[^"]*)"/,
  )?.[1];
  assert(libraryStore);
  await page.addInitScript(() => {
    window.isTauri = true;
    window.requests = [];
    window.games = [];
    window.hfJobs = [];
    window.hfRoot = null;
    const fixture = {
      id: 'hf-game',
      title: '潮汐寄来的信',
      title_zh: null,
      title_ja: null,
      title_en: null,
      cover_url: '/preview/shore.svg',
      developer: null,
      publisher: null,
      release_date: null,
      source_rating: null,
      source_tags: [],
      status: 'not_started',
      favorite: false,
      hidden: false,
      user_rating: null,
      total_playtime_seconds: 0,
      last_played_at: null,
      added_at: '2026-10-06T00:00:00Z',
      metadata_status: 'local_only',
      installations: [],
      tags: [],
      has_save_backup: false,
      hikari_field: { app_id: 7, released: true },
      description: null,
      metadata: [],
      metadata_locked: false,
    };
    window.fixture = fixture;
    const signedOut = {
      status: 'signed_out',
      profile: null,
      message: '尚未登录',
    };
    window.hfAccount = signedOut;
    window.__TAURI_INTERNALS__ = {
      metadata: {
        currentWindow: { label: 'main' },
        currentWebview: { label: 'main' },
      },
      transformCallback: () => 1,
      unregisterCallback: () => {},
      convertFileSrc: (v) => v,
      invoke: async (command, args = {}) => {
        if (command === 'plugin:dialog|open') return 'D:\\Stories';
        if (command.startsWith('plugin:event|')) return 1;
        const req = args.request;
        if (!req) throw Error(command);
        window.requests.push({
          command,
          payload: {
            ...req.payload,
            password: req.payload?.password ? '[REDACTED]' : undefined,
          },
        });
        const ok = (data) => ({
          success: true,
          error_code: null,
          message: '隔离数据',
          request_id: req.request_id,
          data: structuredClone(data),
        });
        switch (command) {
          case 'hikarifield_account':
            return ok(window.hfAccount);
          case 'hikarinagi_account':
          case 'bangumi_account':
            return ok(signedOut);
          case 'get_hikarifield_settings':
            return ok({ root: window.hfRoot, uuid: 'stable-fixture' });
          case 'login_hikarifield':
            window.hfAccount = {
              status: 'authenticated',
              profile: { id: 7, name: '光与故事' },
              message: '已登录',
            };
            return ok(window.hfAccount);
          case 'sync_hikarifield':
            window.games = [window.fixture];
            return ok({ owned: 1, imported: 1 });
          case 'backend_status':
            return ok({
              data_directory: 'C:\\Fixture\\data',
              schema_version: 1,
              portable: true,
              last_scan_task_id: null,
            });
          case 'list_games':
            return ok({
              items: window.games,
              total: window.games.length,
              page: 1,
              page_size: 100,
            });
          case 'get_game':
            return ok(window.fixture);
          case 'list_collections':
          case 'get_recommendations':
          case 'list_external_sources':
            return ok([]);
          case 'get_home_summary':
            return ok({});
          case 'set_hikarifield_path':
            window.hfRoot = 'D:\\Stories\\HikariFieldGames';
            return ok({ root: window.hfRoot, uuid: 'stable-fixture' });
          case 'start_hikarifield_download': {
            const t = {
              id: 'job',
              game_id: 'hf-game',
              title: fixture.title,
              status: 'running',
              message: '正在下载游戏…',
              downloaded: 5242880,
              total: 10485760,
              speed: 1048576,
              install_path: 'D:\\Stories\\HikariFieldGames\\Fixture',
            };
            window.hfJobs = [t];
            return ok(t);
          }
          case 'list_hikarifield_downloads':
            return ok(window.hfJobs);
          case 'launch_game':
            return ok({
              session_id: 'session',
              game_id: 'hf-game',
              install_id: 'install',
              started_at: '2026-10-06T00:00:00Z',
            });
          default:
            throw Error(`Unexpected fixture command ${command}`);
        }
      },
    };
  });
  await page.route('**/__hikarifield_workflow_test', (route) =>
    route.fulfill({
      contentType: 'text/html',
      body: `<html><head><meta charset="utf-8"></head><body><div id="test-root"></div><script type="module">
 import {createApp,h,ref} from '${vue}';import {createAppRouter} from '/src/router/index.ts';
 import AccountDialog from '/src/components/AccountDialog.vue';import Detail from '/src/views/PreviewDetail.vue';import Folder from '/src/components/HikariFieldFolderDialog.vue';import Settings from '/src/components/settings/HikariFieldSettings.vue';import OperationCenter from '/src/components/OperationCenter.vue';
 import {hikariField,pollHikariDownloads} from '${hfStore}';import {refreshLibrary} from '${libraryStore}';
 import '/src/style.css';import '/src/appearance.css';import '/src/settings.css';import '/src/sidebar-tools.css';import '/src/detail-workspace.css';
 const mode=ref('account');const open=ref(true);window.hf=hikariField;window.pollHF=pollHikariDownloads;window.refreshHF=refreshLibrary;window.testMode=mode;window.accountOpen=open;
 const router=createAppRouter();await router.push('/games/hf-game');
 createApp({render:()=>h('main',{style:'padding:32px'},[h('div',{style:'position:fixed;bottom:20px;left:20px;z-index:20'},[h(OperationCenter)]),mode.value==='detail'?h(Detail):mode.value==='settings'?h(Settings):null,open.value?h(AccountDialog,{initialProvider:'hikarifield',onClose:()=>open.value=false}):null,hikariField.folder_game?h(Folder):null])}).use(router).mount('#test-root');
 </script></body></html>`,
    }),
  );
  await page.goto(`${base}/__hikarifield_workflow_test`);
  await page
    .getByRole('heading', { name: 'HIKARI FIELD', exact: true })
    .waitFor();
  assert.equal(
    await page
      .getByRole('tab', { name: 'HIKARI FIELD', exact: true })
      .getAttribute('aria-selected'),
    'true',
  );
  checks++;
  await page.screenshot({ path: `${out}/login.png` });
  await page
    .getByRole('textbox', { name: '邮箱', exact: true })
    .fill('fixture@example.test');
  await page.locator('input[type="password"]').fill('fixture-password');
  await page
    .getByRole('button', { name: '登录并导入已购游戏', exact: true })
    .click();
  await page.getByText('已同步 1 部已拥有的游戏，新增 1 部。').waitFor();
  assert.equal(await page.locator('input[type="password"]').count(), 0);
  checks++;
  await page.getByRole('button', { name: '关闭账户窗口' }).click();
  await page.evaluate(() => (window.testMode.value = 'detail'));
  await page.getByRole('button', { name: '下载游戏', exact: true }).waitFor();
  checks++;
  await page.getByRole('button', { name: '下载游戏', exact: true }).click();
  await page.getByRole('heading', { name: '为故事选一个家' }).waitFor();
  assert.equal(
    await page.evaluate(
      () =>
        window.requests.filter(
          (r) => r.command === 'start_hikarifield_download',
        ).length,
    ),
    0,
  );
  checks++;
  await page.getByRole('button', { name: '选择存放位置', exact: true }).click();
  await page
    .getByText('D:\\Stories\\HikariFieldGames', { exact: true })
    .waitFor();
  await page.locator('.hf-folder-dialog').evaluate(async (node) => {
    await Promise.all(
      node
        .getAnimations({ subtree: true })
        .map((a) => a.finished.catch(() => {})),
    );
  });
  await page.waitForFunction(
    () =>
      getComputedStyle(document.querySelector('.hf-destination')).opacity ===
      '1',
  );
  await page.screenshot({ path: `${out}/folder.png` });
  await page.getByRole('button', { name: '保存并下载游戏' }).click();
  await page.getByRole('button', { name: '正在下载…', exact: true }).waitFor();
  assert(
    await page
      .getByRole('button', { name: '正在下载…', exact: true })
      .isDisabled(),
  );
  checks++;
  await page.getByRole('button', { name: '打开后台任务' }).click();
  await page.getByRole('button', { name: '取消下载', exact: true }).waitFor();
  assert.match(
    await page.locator('.operation-popover').innerText(),
    /5.0 MB \/ 10.0 MB/,
  );
  checks++;
  await page.locator('.operation-popover').evaluate(async (node) => {
    await Promise.all(
      node
        .getAnimations({ subtree: true })
        .map((a) => a.finished.catch(() => {})),
    );
  });
  await page.screenshot({ path: `${out}/download.png` });
  await page.keyboard.press('Escape');
  await page.locator('.operation-popover').waitFor({ state: 'hidden' });
  await page.evaluate(() => {
    window.fixture.installations = [
      {
        id: 'install',
        game_id: 'hf-game',
        absolute_path: 'D:\\Stories\\HikariFieldGames\\Fixture',
        executable_path: 'D:\\Stories\\HikariFieldGames\\Fixture\\Fixture.exe',
        source: 'local',
        steam_app_id: null,
        path_valid: true,
      },
    ];
    window.hfJobs[0].status = 'completed';
    window.hfJobs[0].downloaded = window.hfJobs[0].total;
  });
  await page.evaluate(() => window.pollHF());
  await page.getByRole('button', { name: '启动游戏', exact: true }).waitFor();
  checks++;
  await page.getByRole('button', { name: '启动游戏', exact: true }).click();
  assert.equal(
    await page.evaluate(
      () => window.requests.filter((r) => r.command === 'launch_game').length,
    ),
    1,
  );
  checks++;
  await page.evaluate(() => (window.testMode.value = 'settings'));
  await page
    .getByText('D:\\Stories\\HikariFieldGames', { exact: true })
    .waitFor();
  await page.screenshot({ path: `${out}/settings.png` });
  checks++;
  await page.evaluate(() => {
    window.accountOpen.value = true;
  });
  await page
    .getByRole('heading', { name: 'HIKARI FIELD', exact: true })
    .waitFor();
  await page.getByRole('tab', { name: 'HIKARI FIELD', exact: true }).focus();
  await page.keyboard.press('ArrowRight');
  assert.equal(
    await page.evaluate(() => document.activeElement.id),
    'account-tab-bangumi',
  );
  await page.keyboard.press('End');
  assert.equal(
    await page.evaluate(() => document.activeElement.id),
    'account-tab-hikarifield',
  );
  checks += 2;
  await page.setViewportSize({ width: 390, height: 844 });
  await page.waitForFunction(
    () =>
      getComputedStyle(document.querySelector('.account-card')).opacity === '1',
  );
  await page.screenshot({ path: `${out}/narrow.png` });
  const bounds = await page.locator('dialog.account-dialog').boundingBox();
  assert(bounds.x >= 0 && bounds.x + bounds.width <= 390);
  checks++;
  await page.emulateMedia({ reducedMotion: 'reduce' });
  assert.equal(
    await page
      .locator('.account-tab-indicator')
      .evaluate((n) => getComputedStyle(n).transitionDuration),
    '0s',
  );
  checks++;
  await page.emulateMedia({ reducedMotion: 'no-preference' });
  await page.evaluate(() => {
    document.documentElement.dataset.motion = 'reduced';
    document.documentElement.dataset.theme = 'light';
    window.hf.account = {
      status: 'signed_out',
      profile: null,
      message: '尚未登录',
    };
  });
  await page.getByRole('textbox', { name: '邮箱', exact: true }).waitFor();
  assert.equal(
    await page
      .locator('.account-tab-indicator')
      .evaluate((n) => getComputedStyle(n).transitionDuration),
    '0s',
  );
  await page.screenshot({ path: `${out}/light-login.png` });
  checks++;
  assert.equal(
    await page.evaluate(() =>
      window.requests.some((r) =>
        [
          'search_metadata',
          'prepare_import_metadata',
          'start_metadata_refresh',
        ].includes(r.command),
      ),
    ),
    false,
  );
  checks++;
  assert.deepEqual(errors, []);
  checks++;
  console.log(
    `HIKARI FIELD browser workflow: ${checks} checks passed; screenshots in ${out}. Synthetic IPC only.`,
  );
} finally {
  await browser.close();
}
