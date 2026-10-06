// Real Vue components with isolated synthetic IPC; no account or game data is read.
import assert from 'node:assert/strict';
import { mkdir, writeFile } from 'node:fs/promises';
import { chromium } from '../.tools/browser-check/node_modules/playwright/index.mjs';
const base = process.env.PREVIEW_URL || 'http://127.0.0.1:1420';
assert(['127.0.0.1', 'localhost'].includes(new URL(base).hostname));
const out = '.tools/owned-metadata-evidence';
await mkdir(out, { recursive: true });
const browser = await chromium.launch({
  headless: true,
  executablePath:
    process.env.BROWSER_EXECUTABLE ||
    '/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge',
});
let checks = 0;
try {
  const page = await browser.newPage({
    viewport: { width: 1280, height: 900 },
  });
  const errors = [];
  page.on('pageerror', (e) => errors.push(e.message));
  page.setDefaultTimeout(12000);
  const source = await (
    await page.request.get(`${base}/src/components/OwnedMetadataDialog.vue`)
  ).text();
  const vue = source.match(/from "([^"]*\/vue\.js[^"]*)"/)?.[1];
  const ownedStore = source.match(
    /from "([^"]*\/stores\/ownedMetadata\.ts[^"]*)"/,
  )?.[1];
  const libraryStore = source.match(
    /from "([^"]*\/stores\/library\.ts[^"]*)"/,
  )?.[1];
  assert(vue && ownedStore && libraryStore);
  const hfSrc = await (
    await page.request.get(`${base}/src/components/preview/CoverCard.vue`)
  ).text();
  const hfStore = hfSrc.match(
    /from "([^"]*\/stores\/hikariField\.ts[^"]*)"/,
  )?.[1];
  assert(hfStore);
  await page.addInitScript(() => {
    window.isTauri = true;
    window.requests = [];
    window.resolveSearch = null;
    window.retryReady = false;
    const fixture = (id, title, cover) => ({
      id,
      title,
      title_zh: null,
      title_ja: null,
      title_en: null,
      cover_url: cover,
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
      added_at: '2026-10-07T00:00:00Z',
      metadata_status: 'local_only',
      installations: [],
      tags: [],
      has_save_backup: false,
      hikari_field: { app_id: 7, released: true },
      description: null,
      metadata: [
        {
          provider: 'hikarifield',
          field: 'title',
          value: JSON.stringify(title),
          remote_id: '7',
          manually_edited: false,
          fetched_at: '',
        },
      ],
      metadata_locked: false,
    });
    window.games = [
      fixture('shore', '潮汐寄来的信', '/preview/shore.svg'),
      fixture('forest', '月光落在森林里', '/preview/forest.svg'),
      fixture('sky', '与云同行的夏天', '/preview/sky.svg'),
    ];
    const installed = fixture('snow', '雪原的最后一盏灯', '/preview/snow.svg');
    installed.installations = [
      {
        id: 'native-install',
        game_id: 'snow',
        absolute_path: 'D:\\Games\\Snow',
        executable_path: 'Snow.exe',
        source: 'local',
        path_valid: true,
      },
    ];
    installed.metadata_status = 'synced';
    window.games.push(installed);
    const candidate = (provider, title, remote_id = '1') => ({
      provider,
      remote_id,
      title,
      subtitle: '原名 · 特别版',
      cover_url: '/preview/forest.svg',
      confidence: 1,
      matched_fields: ['title'],
      explanation: '',
      fetched_at: '',
      cached: false,
    });
    window.__TAURI_INTERNALS__ = {
      metadata: {
        currentWindow: { label: 'main' },
        currentWebview: { label: 'main' },
      },
      transformCallback: () => 1,
      unregisterCallback: () => {},
      convertFileSrc: (v) => v,
      invoke: async (command, args = {}) => {
        if (command === 'plugin:dialog|open') return 'D:\\Games';
        if (command.startsWith('plugin:event|')) return 1;
        const req = args.request;
        if (!req) throw Error(command);
        window.requests.push({ command, payload: req.payload });
        const ok = (data) => ({
          success: true,
          error_code: null,
          message: '隔离测试',
          request_id: req.request_id,
          data: structuredClone(data),
        });
        const p = req.payload;
        switch (command) {
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
            return ok(window.games.find((g) => g.id === p.game_id));
          case 'list_collections':
          case 'get_recommendations':
          case 'list_hikarifield_downloads':
            return ok([]);
          case 'get_home_summary':
            return ok({});
          case 'get_metadata_sources':
            return ok({
              sources: [
                { provider: 'vndb', enabled: true },
                { provider: 'bangumi', enabled: true },
                { provider: 'hikarinagi', enabled: false },
              ],
            });
          case 'cancel_metadata_search':
            return ok(true);
          case 'search_metadata': {
            const provider = p.providers[0];
            if (p.query === '潮汐寄来的信' && provider === 'vndb')
              return new Promise((resolve) => {
                window.resolveSearch = () => resolve(ok([]));
              });
            if (p.query === '潮汐寄来的信')
              return ok([candidate(provider, p.query)]);
            if (p.query === '月光落在森林里')
              return provider === 'vndb'
                ? ok([
                    candidate(provider, p.query),
                    candidate(provider, `${p.query} · 另一版本`, '2'),
                  ])
                : ok([]);
            if (window.retryReady) return ok([candidate(provider, p.query)]);
            return {
              ...ok(null),
              success: false,
              error_code: 'NETWORK_UNAVAILABLE',
              message: '资料来源暂时不可用，请稍后重试。',
            };
          }
          case 'confirm_metadata_match': {
            const game = window.games.find((g) => g.id === p.game_id);
            game.description =
              '在漫长夏日的尽头，一封没有署名的信被潮水送到岸边。沿着海岸的旧铁道，与你一同寻找那些尚未说出口的话。';
            game.developer = '虚构工作室 · 海岸';
            game.release_date = '2024-07-12';
            game.metadata_status = 'synced';
            game.metadata.push({
              provider: p.provider,
              remote_id: p.remote_id,
              field: 'description',
              value: JSON.stringify(game.description),
              manually_edited: false,
              fetched_at: '',
            });
            return ok({
              game_id: game.id,
              provider: p.provider,
              remote_id: p.remote_id,
              matched_at: '',
              cover_message: null,
              supplementation_message: '已按来源顺序补充缺失字段。',
            });
          }
          case 'set_hikarifield_path':
            return ok({ root: 'D:\\Games\\HikariFieldGames', uuid: 'fixture' });
          case 'start_hikarifield_download':
            return ok({
              id: 'download-fixture',
              game_id: p.game_id,
              title: '已购故事',
              status: 'running',
              message: '正在下载',
              downloaded: 0,
              total: 100,
              speed: 0,
              install_path: '',
            });
          default:
            throw Error(`Unexpected command: ${command}`);
        }
      },
    };
  });
  await page.route('**/__owned_metadata_test', (route) =>
    route.fulfill({
      contentType: 'text/html',
      body: `<html><head><meta charset="utf-8"></head><body><div id="root"></div><script type="module">
    import {createApp,h} from '${vue}';
    import Dialog from '/src/components/OwnedMetadataDialog.vue';import Card from '/src/components/preview/CoverCard.vue';import OperationCenter from '/src/components/OperationCenter.vue';import Folder from '/src/components/HikariFieldFolderDialog.vue';
    import {ownedMetadata,queueOwnedMetadata} from '${ownedStore}';import {preview,refreshLibrary} from '${libraryStore}';import {hikariField} from '${hfStore}';
    import '/src/style.css';import '/src/appearance.css';import '/src/sidebar-tools.css';
    window.owned=ownedMetadata;window.queueOwned=queueOwnedMetadata;window.previewStore=preview;window.hf=hikariField;
    Object.assign(hikariField,{loaded:true,account:{status:'authenticated',profile:{id:7,name:'测试'},message:''}});
    await refreshLibrary();
    createApp({render:()=>h('main',{style:'padding:40px'},[h('h1','我的游戏库'),h('div',{class:'gallery-grid',style:'display:grid;grid-template-columns:repeat(4,1fr);gap:24px'},preview.games.map(game=>h(Card,{game}))),h('div',{style:'position:fixed;bottom:20px;left:20px'},[h(OperationCenter)]),ownedMetadata.open?h(Dialog):null,hikariField.folder_game?h(Folder):null])}).mount('#root');
    </script></body></html>`,
    }),
  );
  await page.goto(`${base}/__owned_metadata_test`);
  await page.locator('.cover-card').first().waitFor();
  assert.equal(await page.locator('.card-download').count(), 3);
  checks++;
  assert.equal(
    await page.locator('[data-game-id="snow"] .card-download').count(),
    0,
  );
  checks++;
  await page.evaluate(() => window.queueOwned());
  await page.waitForFunction(() => !!window.resolveSearch);
  assert.equal(await page.locator('.owned-metadata-dialog').count(), 0);
  checks++;
  await page.getByRole('button', { name: '打开后台任务' }).click();
  await page.getByRole('button', { name: '查看刮削结果' }).click();
  await page
    .getByRole('heading', { name: '已购游戏资料', exact: true })
    .waitFor();
  checks++;
  await page.screenshot({ path: `${out}/running-dark.png` });
  await page.getByRole('button', { name: '后台继续', exact: true }).click();
  assert.equal(await page.locator('.owned-metadata-dialog').count(), 0);
  checks++;
  assert.equal(await page.evaluate(() => window.owned.running), true);
  checks++;
  await page.evaluate(() => window.resolveSearch());
  await page.waitForFunction(() => !window.owned.running);
  assert.deepEqual(
    await page.evaluate(() => window.owned.items.map((i) => i.status)),
    ['completed', 'review', 'failed'],
  );
  checks++;
  const requests = await page.evaluate(() => window.requests);
  assert.deepEqual(
    requests
      .filter((r) => r.command === 'search_metadata')
      .map((r) => r.payload.providers[0]),
    ['vndb', 'bangumi', 'vndb', 'bangumi', 'vndb', 'bangumi'],
  );
  checks++;
  assert.equal(
    requests.some(
      (r) => r.command === 'import_game' || r.command === 'preview_import',
    ),
    false,
  );
  checks++;
  assert.equal(
    requests
      .filter((r) => r.command === 'confirm_metadata_match')
      .every((r) => r.payload.manual === false),
    true,
  );
  checks++;
  await page.getByRole('button', { name: '打开后台任务' }).click();
  await page.getByRole('button', { name: '查看刮削结果' }).click();
  await page.getByRole('button', { name: /月光落在森林里.*待确认/ }).click();
  assert.equal(
    await page.getByRole('button', { name: '使用此资料', exact: true }).count(),
    2,
  );
  checks++;
  await page.screenshot({ path: `${out}/review-dark.png` });
  await page.evaluate(() => (document.documentElement.dataset.theme = 'light'));
  await page.screenshot({ path: `${out}/review-light.png` });
  await page.setViewportSize({ width: 390, height: 844 });
  await page.screenshot({ path: `${out}/review-narrow.png` });
  assert(
    await page
      .locator('.owned-metadata-dialog')
      .evaluate((el) => el.scrollWidth <= el.clientWidth + 1),
  );
  checks++;
  await page.evaluate(
    () => (document.documentElement.dataset.motion = 'reduced'),
  );
  assert.equal(
    await page
      .locator('.owned-metadata-dialog')
      .evaluate((el) => getComputedStyle(el).animationName),
    'none',
  );
  checks++;
  await page.setViewportSize({ width: 1280, height: 900 });
  await page
    .getByRole('button', { name: '使用此资料', exact: true })
    .first()
    .click();
  await page.waitForFunction(
    () => !window.owned.running && window.owned.items[1].status === 'completed',
  );
  checks++;
  await page.getByText('虚构工作室 · 海岸', { exact: true }).waitFor();
  checks++;
  await page.screenshot({ path: `${out}/completed-light.png` });
  await page.evaluate(() => (window.retryReady = true));
  await page.getByRole('button', { name: '重试未完成项', exact: true }).click();
  await page.waitForFunction(
    () =>
      !window.owned.running &&
      window.owned.items.every((i) => i.status === 'completed'),
  );
  checks++;
  await page.keyboard.press('Escape');
  assert.equal(await page.locator('.owned-metadata-dialog').count(), 0);
  checks++;
  await page
    .getByRole('button', { name: '下载潮汐寄来的信', exact: true })
    .click();
  await page
    .getByRole('heading', { name: '为故事选一个家', exact: true })
    .waitFor();
  checks++;
  assert.equal(await page.locator('.owned-metadata-dialog').count(), 0);
  checks++;
  assert.deepEqual(errors, []);
  checks++;
  await writeFile(
    `${out}/results.json`,
    JSON.stringify(
      {
        checks,
        errors,
        limitations:
          'Synthetic IPC in macOS browser; not Windows native or real-account acceptance.',
      },
      null,
      2,
    ),
  );
  console.log(`已购游戏资料界面：${checks} 项通过。截图：${out}`);
} finally {
  await browser.close();
}
