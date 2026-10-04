// Actual App timer and detail page; all IPC fixtures stay in browser memory.
import assert from 'node:assert/strict';
import { mkdirSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { chromium } from '../.tools/browser-check/node_modules/playwright/index.mjs';
const base = process.env.PREVIEW_URL || 'http://127.0.0.1:1420';
assert(['127.0.0.1', 'localhost'].includes(new URL(base).hostname));
const evidence = resolve('.tools/detail-refresh-evidence', String(Date.now()));
mkdirSync(evidence, { recursive: true });
const browser = await chromium.launch({
  headless: true,
  executablePath:
    process.env.BROWSER_EXECUTABLE ||
    '/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge',
});
try {
  const page = await browser.newPage({
    viewport: { width: 1280, height: 900 },
  });
  const errors = [];
  page.on('pageerror', (error) => errors.push(error.message));
  const source = await (await page.request.get(`${base}/src/App.vue`)).text();
  const vueModule = source.match(
    /from "(\/node_modules\/\.vite\/deps\/vue\.js[^"]*)"/,
  )?.[1];
  const libraryModule = source.match(
    /from "(\/src\/stores\/library\.ts[^"]*)"/,
  )?.[1];
  assert(vueModule && libraryModule);
  await page.addInitScript(() => {
    window.isTauri = true;
    const description =
      '这是已经刮削保存的中文简介。\n第二段简介也应持续显示。';
    window.fixture = {
      lists: 0,
      details: 0,
      description,
      failDetail: false,
      summary: {
        id: 'detail-fixture',
        title: '简介刷新隔离测试',
        title_zh: null,
        title_ja: null,
        title_en: null,
        developer: null,
        publisher: null,
        release_date: null,
        source_rating: null,
        source_tags: [],
        status: 'not_started',
        favorite: false,
        hidden: false,
        user_rating: null,
        added_at: '2026-10-01T00:00:00Z',
        cover_url: null,
        total_playtime_seconds: 0,
        last_played_at: null,
        metadata_status: 'synced',
        installations: [],
        tags: [],
      },
      metadata: [],
    };
    window.__TAURI_INTERNALS__ = {
      metadata: {
        currentWindow: { label: 'main' },
        currentWebview: { label: 'main' },
      },
      transformCallback: () => 1,
      unregisterCallback: () => {},
      invoke: async (command, args = {}) => {
        if (!args.request) return 1;
        let data = {};
        if (command === 'get_game') {
          window.fixture.details++;
          if (window.fixture.failDetail)
            return {
              success: false,
              error_code: 'DATABASE_ERROR',
              message: '隔离测试：详情暂时读取失败',
              request_id: args.request.request_id,
              data: null,
            };
          data = {
            ...window.fixture.summary,
            description: window.fixture.description,
            metadata: window.fixture.metadata,
          };
        } else if (command === 'list_games') {
          window.fixture.lists++;
          window.fixture.summary.total_playtime_seconds =
            window.fixture.lists * 60;
          data = {
            items: [window.fixture.summary],
            total: 1,
            page: 1,
            page_size: 100,
          };
        } else if (command === 'backend_status')
          data = {
            data_directory: 'isolated-memory-only',
            schema_version: 1,
            portable: true,
            last_scan_task_id: null,
          };
        else if (command === 'get_app_settings') data = window.fixture.settings;
        else if (['list_collections', 'get_recommendations'].includes(command))
          data = [];
        else if (command === 'get_hikarinagi_rates') data = { wall: null };
        else if (['bangumi_account', 'hikarinagi_account'].includes(command))
          data = { status: 'signed_out', profile: null, message: '' };
        else if (command === 'get_metadata_refresh') data = { status: 'idle' };
        else if (command === 'get_application_backup_status')
          data = { running: false };
        return {
          success: true,
          error_code: null,
          message: '隔离内存响应',
          request_id: args.request.request_id,
          data: structuredClone(data),
        };
      },
    };
  });
  await page.route('**/__detail_refresh_test', (route) =>
    route.fulfill({
      contentType: 'text/html',
      body: `<html><head><meta charset="utf-8"></head><body><div id="test-root"></div><script type="module">
    import {createApp} from '${vueModule}';
    import App from '/src/App.vue';
    import {createAppRouter} from '/src/router/index.ts';
    import {createMemoryHistory} from '/node_modules/.vite/deps/vue-router.js';
    import {DEFAULT_SETTINGS} from '/src/types/settings.ts';
    import {loadDetail,local,preview} from '${libraryModule}';
    import '/src/style.css'; import '/src/appearance.css';
    window.fixture.settings = {...DEFAULT_SETTINGS,ui_refresh_seconds: 1};
    window.detailTest = {loadDetail,local,preview};
    const router=createAppRouter(createMemoryHistory());await router.push('/games/detail-fixture');await router.isReady();
    createApp(App).use(router).mount('#test-root');
    </script></body></html>`,
    }),
  );
  await page.goto(`${base}/__detail_refresh_test`);
  await page.getByRole('heading', { name: '作品简介', exact: true }).waitFor();
  await page.waitForFunction(() =>
    document
      .querySelector('.detail-prose')
      ?.textContent.includes('第二段简介也应持续显示。'),
  );
  await page.waitForFunction(() => window.fixture.lists >= 4);
  assert(
    (await page.locator('.detail-prose').innerText()).includes(
      '第二段简介也应持续显示。',
    ),
  );
  const state = await page.evaluate(() => ({
    lists: window.fixture.lists,
    details: window.fixture.details,
    playtime: window.detailTest.preview.games[0].playtime_seconds,
  }));
  assert.equal(state.details, 1, 'Timer must not refetch every detail');
  assert(state.playtime >= 240);
  await page.evaluate(async () => {
    window.fixture.failDetail = true;
    await window.detailTest.loadDetail('detail-fixture');
  });
  assert(
    (await page.locator('.detail-prose').innerText()).includes(
      '第二段简介也应持续显示。',
    ),
  );
  await page.evaluate(async () => {
    window.fixture.failDetail = false;
    window.fixture.description = null;
    await window.detailTest.loadDetail('detail-fixture');
  });
  await page.waitForFunction(() =>
    document
      .querySelector('.detail-prose')
      ?.textContent.includes('尚未填写作品简介'),
  );
  await page.evaluate(async () => {
    window.fixture.description = '更新后的中文简介。';
    await window.detailTest.loadDetail('detail-fixture');
  });
  await page.waitForFunction(() =>
    document
      .querySelector('.detail-prose')
      ?.textContent.includes('更新后的中文简介。'),
  );
  await page.screenshot({
    path: resolve(evidence, 'detail-after-refresh.png'),
  });
  assert.deepEqual(errors, []);
  const result = { passed: true, timer: state, errors, evidence };
  writeFileSync(
    resolve(evidence, 'result.json'),
    JSON.stringify(result, null, 2),
  );
  console.log(JSON.stringify(result, null, 2));
} finally {
  await browser.close();
}
