// Isolated browser IPC fixtures: never uses the user's Steam library or database.
import assert from 'node:assert/strict';
import { mkdir } from 'node:fs/promises';
import { chromium } from '../.tools/browser-check/node_modules/playwright/index.mjs';
const base = process.env.PREVIEW_URL || 'http://127.0.0.1:1420';
assert(['127.0.0.1', 'localhost'].includes(new URL(base).hostname));
const output = new URL('../.tools/browser-check/steam-import/', import.meta.url)
  .pathname;
await mkdir(output, { recursive: true });
const browser = await chromium.launch({
  headless: true,
  executablePath:
    process.env.BROWSER_EXECUTABLE ||
    '/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge',
});
try {
  const page = await browser.newPage({
    viewport: { width: 1440, height: 1000 },
  });
  const errors = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.route('**/*', (route) =>
    ['127.0.0.1', 'localhost'].includes(new URL(route.request().url()).hostname)
      ? route.continue()
      : route.abort(),
  );
  await page.addInitScript(() => {
    const f = {
      calls: [],
      games: [],
      fail: true,
      delay: false,
      release: null,
      serial: 0,
    };
    window.__STEAM_TEST__ = f;
    window.isTauri = true;
    let callback = 0;
    window.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener() {} };
    window.__TAURI_INTERNALS__ = {
      metadata: {
        currentWindow: { label: 'main' },
        currentWebview: { label: 'main' },
      },
      transformCallback: () => ++callback,
      unregisterCallback() {},
      convertFileSrc: () => '/preview/shore.svg',
      invoke: async (command, args = {}) => {
        const p = args.request?.payload;
        f.calls.push({ command, payload: p });
        if (command === 'plugin:dialog|open') return 'C:/SteamLibrary';
        if (command === 'plugin:event|listen') return ++callback;
        if (command.startsWith('plugin:')) return null;
        let data;
        switch (command) {
          case 'backend_status':
            data = {
              data_directory: 'C:/data',
              portable: true,
              schema_version: 1,
              last_scan_task_id: null,
            };
            break;
          case 'bangumi_account':
            data = { status: 'signed_out', profile: null, message: '' };
            break;
          case 'list_collections':
          case 'get_recommendations':
            data = [];
            break;
          case 'list_games':
            data = {
              items: f.games,
              total: f.games.length,
              page: 1,
              page_size: 100,
            };
            break;
          case 'get_home_summary':
            data = {
              game_count: f.games.length,
              playing_count: 0,
              completed_count: 0,
              week_playtime_seconds: 0,
              pending_match_count: 0,
              save_issue_count: 0,
              continue_game_ids: [],
              recent_game_ids: [],
            };
            break;
          case 'scan_steam_games':
            data = {
              library_count: 2,
              warnings: ['离线库暂不可访问：D:/SteamLibrary'],
              games: [
                {
                  app_id: '1230140',
                  name: 'ATRI -My Dear Moments-',
                  directory: 'C:/SteamLibrary/steamapps/common/ATRI',
                  library_path: 'C:/SteamLibrary',
                  size_bytes: 512000000,
                  existing_game_id: null,
                },
                {
                  app_id: '124',
                  name: 'Other Genre',
                  directory: 'C:/Steam/steamapps/common/Other',
                  library_path: 'C:/Steam',
                  size_bytes: 123000000,
                  existing_game_id: null,
                },
                {
                  app_id: '125',
                  name: 'Already Imported',
                  directory: 'C:/Steam/steamapps/common/Existing',
                  library_path: 'C:/Steam',
                  size_bytes: 0,
                  existing_game_id: 'existing',
                },
              ],
            };
            break;
          case 'get_metadata_sources':
            data = {
              sources: [
                { provider: 'hikarinagi', enabled: true },
                { provider: 'bangumi', enabled: false },
                { provider: 'vndb', enabled: false },
              ],
            };
            break;
          case 'begin_import_batch':
            data = {
              batch_id: `batch-${++f.serial}`,
              sources: { sources: [] },
            };
            break;
          case 'prepare_steam_import':
            if (f.delay) {
              f.delay = false;
              await new Promise((resolve) => {
                f.release = resolve;
              });
            }
            if (p.app_id === '124' && f.fail) {
              f.fail = false;
              return {
                success: false,
                error_code: 'NETWORK_UNAVAILABLE',
                message: 'Steam 封面下载失败，请重试',
                request_id: args.request.request_id,
                data: null,
              };
            }
            data = {
              preparation_id: `prepared-${++f.serial}`,
              title:
                p.app_id === '124' ? 'Other Genre' : 'ATRI -My Dear Moments-',
              subtitle: null,
              cover_path: 'covers/fixture.png',
              provider: 'steam',
              remote_id: p.app_id,
              translation_message: null,
              supplementation_message: p.hikarinagi_remote_id
                ? '已关联 Hikarinagi 安利墙 · #897'
                : 'Steam 资料已准备；未找到唯一的安利墙关联，可手动选择作品。',
            };
            break;
          case 'search_metadata':
            data = [
              {
                provider: 'hikarinagi',
                remote_id: '897',
                title: 'ATRI -My Dear Moments-',
                subtitle: '亚托莉',
                confidence: 1,
                cover_url: null,
                has_chinese_description: true,
                matched_fields: ['title'],
                explanation: 'fixture',
                fetched_at: '2026-10-08T00:00:00Z',
                cached: false,
              },
            ];
            break;
          case 'cancel_import_batch':
          case 'discard_import_metadata':
            data = true;
            break;
          case 'import_steam_game': {
            if (!p.preparation_id) throw Error('unexpected unprepared import');
            const game = {
              id: `steam-${p.app_id}`,
              title:
                p.app_id === '124' ? 'Other Genre' : 'ATRI -My Dear Moments-',
              title_zh: null,
              title_ja: null,
              title_en: null,
              cover_url: 'covers/fixture.png',
              developer: 'Steam fixture',
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
              added_at: '2026-10-08T00:00:00Z',
              metadata_status: 'synced',
              installations: [
                {
                  id: `install-${p.app_id}`,
                  game_id: `steam-${p.app_id}`,
                  absolute_path: p.directory,
                  executable_path: null,
                  source: 'steam',
                  steam_app_id: p.app_id,
                  path_valid: true,
                },
              ],
              tags: [],
              has_save_backup: false,
              description: 'Steam 简介',
              metadata: [],
            };
            f.games.push(game);
            data = { game };
            break;
          }
          default:
            throw Error(`Unexpected IPC: ${command}`);
        }
        return {
          success: true,
          error_code: null,
          message: '',
          request_id: args.request.request_id,
          data,
        };
      },
    };
  });
  await page.goto(`${base}/#/games`);
  const add = page
    .locator('.games-page')
    .getByRole('button', { name: '添加游戏', exact: true });
  await add.click();
  const chooser = page.getByRole('dialog', { name: '添加本地游戏' });
  assert.equal(await chooser.locator('.import-choice').count(), 3);
  await page.waitForTimeout(450);
  await page.screenshot({ path: `${output}chooser.png` });
  await chooser.getByRole('button', { name: /从 Steam 导入/ }).click();
  const dialog = page.getByRole('dialog', {
    name: '从 Steam 导入',
    exact: true,
  });
  await dialog.locator('.steam-row').nth(2).waitFor();
  assert(
    await dialog
      .getByRole('button', { name: '确认导入', exact: true })
      .isDisabled(),
  );
  assert(
    await dialog
      .getByRole('checkbox', { name: '选择 Already Imported' })
      .isDisabled(),
  );
  assert.equal(
    await page.evaluate(
      () =>
        window.__STEAM_TEST__.calls.filter(
          (c) => c.command === 'prepare_steam_import',
        ).length,
    ),
    0,
    'scan is local only',
  );
  await dialog.getByRole('button', { name: '准备资料', exact: true }).click();
  await dialog
    .getByText('Steam 封面下载失败，请重试', { exact: true })
    .waitFor();
  assert(
    await dialog
      .getByRole('button', { name: '确认导入', exact: true })
      .isDisabled(),
  );
  assert.equal(
    await page.evaluate(() => window.__STEAM_TEST__.games.length),
    0,
    'preparation cannot write library',
  );
  await dialog.getByRole('button', { name: '重试准备' }).click();
  await page.waitForFunction(
    () => !document.querySelector('.steam-footer .primary-button')?.disabled,
  );
  await dialog.getByRole('button', { name: '选择安利墙关联' }).first().click();
  await dialog.getByRole('button', { name: '搜索', exact: true }).click();
  await dialog.locator('.steam-matches button').click();
  await dialog
    .getByText('已关联 Hikarinagi 安利墙 · #897', { exact: true })
    .waitFor();
  for (const [theme, width, height] of [
    ['light', 1440, 1000],
    ['dark', 1024, 768],
    ['light', 390, 844],
    ['light', 800, 450],
  ]) {
    await page.setViewportSize({ width, height });
    await page.evaluate(
      (theme) => (document.documentElement.dataset.theme = theme),
      theme,
    );
    await page.waitForTimeout(250);
    const footer = await dialog.locator('.steam-footer').boundingBox();
    assert(
      footer && footer.y + footer.height <= height + 1,
      'confirmation stays visible',
    );
    const bounds = await dialog.evaluate((d) => ({
      width: d.clientWidth,
      scroll: d.scrollWidth,
      height: d.getBoundingClientRect().height,
      bottom: d.getBoundingClientRect().bottom,
    }));
    assert(
      bounds.scroll <= bounds.width + 1,
      `no horizontal overflow at ${width}`,
    );
    assert(
      bounds.height <= height && bounds.bottom <= height + 1,
      `dialog contained at ${height}`,
    );
    await page.screenshot({ path: `${output}steam-${theme}-${width}.png` });
  }
  await page.setViewportSize({ width: 1440, height: 1000 });
  await dialog.getByRole('button', { name: '确认导入', exact: true }).click();
  await dialog.getByText('已导入 2 款', { exact: true }).waitFor();
  assert.equal(
    await page.evaluate(() => window.__STEAM_TEST__.games.length),
    2,
  );
  assert.equal(
    await page.evaluate(
      () =>
        window.__STEAM_TEST__.calls.filter((c) =>
          ['confirm_metadata_match', 'prepare_import_metadata'].includes(
            c.command,
          ),
        ).length,
    ),
    0,
    'only Steam scraping path',
  );
  await dialog.getByRole('button', { name: '关闭 Steam 导入' }).click();
  await add.click();
  await chooser.getByRole('button', { name: /从 Steam 导入/ }).click();
  await dialog.locator('.steam-row').nth(2).waitFor();
  await page.evaluate(() => {
    window.__STEAM_TEST__.delay = true;
  });
  await dialog.getByRole('button', { name: '准备资料', exact: true }).click();
  await page.waitForFunction(() => !!window.__STEAM_TEST__.release);
  await dialog.getByRole('button', { name: '关闭 Steam 导入' }).click();
  await page.evaluate(() => window.__STEAM_TEST__.release());
  await page.waitForTimeout(250);
  assert.equal(
    await dialog.count(),
    0,
    'late response cannot reopen cancelled dialog',
  );
  assert.equal(
    await page.evaluate(() => window.__STEAM_TEST__.games.length),
    2,
  );
  assert.deepEqual(errors, []);
  console.log(
    `Steam chooser, staging, retry, wall-only binding, confirm, cancellation and responsive views verified. Screenshots: ${output}`,
  );
} finally {
  await browser.close();
}
