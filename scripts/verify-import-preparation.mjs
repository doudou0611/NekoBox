// Real browser interactions with isolated IPC responses; no production database/network writes.
import assert from 'node:assert/strict';
import { chromium } from '../.tools/browser-check/node_modules/playwright/index.mjs';
const base = process.env.PREVIEW_URL || 'http://127.0.0.1:1420';
assert(['127.0.0.1', 'localhost'].includes(new URL(base).hostname));
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
  page.setDefaultTimeout(10000);
  const errors = [];
  page.on('pageerror', (error) => errors.push(error.message));
  await page.route('**/*', (route) =>
    ['127.0.0.1', 'localhost'].includes(new URL(route.request().url()).hostname)
      ? route.continue()
      : route.abort(),
  );
  await page.addInitScript(() => {
    const fixture = {
      calls: [],
      games: [],
      tokens: {},
      serial: 0,
      release: null,
      failLast: true,
      manualFail: false,
      manual: false,
    };
    window.__PREPARATION_TEST__ = fixture;
    window.isTauri = true;
    const titles = Array.from({ length: 10 }, (_, i) => `中文作品${i + 1}`);
    const candidate = (title) => ({
      provider: 'bangumi',
      remote_id: String(
        100 + (titles.indexOf(title) < 0 ? 9 : titles.indexOf(title)),
      ),
      title,
      cover_url: 'https://lain.bgm.tv/cover.jpg',
      confidence: 1,
      has_chinese_description: true,
      matched_fields: ['title'],
      explanation: 'fixture',
      fetched_at: '2026-10-02T00:00:00Z',
      cached: false,
    });
    let callback = 0;
    window.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener() {} };
    window.__TAURI_INTERNALS__ = {
      metadata: {
        currentWindow: { label: 'main' },
        currentWebview: { label: 'main' },
      },
      transformCallback: () => ++callback,
      unregisterCallback() {},
      invoke: async (command, args = {}) => {
        const payload = args.request?.payload;
        fixture.calls.push({ command, payload });
        if (command === 'plugin:dialog|open') return 'C:/Games';
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
              items: fixture.games,
              total: fixture.games.length,
              page: 1,
              page_size: 100,
            };
            break;
          case 'get_home_summary':
            data = {
              game_count: fixture.games.length,
              playing_count: 0,
              completed_count: 0,
              week_playtime_seconds: 0,
              pending_match_count: 0,
              save_issue_count: 0,
              continue_game_ids: [],
              recent_game_ids: [],
            };
            break;
          case 'preview_import':
            data = {
              items: titles.map((title) => ({
                directory: `C:/Games/${title}`,
                folder_name: title,
                search_name: title,
                executables: [
                  {
                    path: `C:/Games/${title}/game.exe`,
                    fingerprint: 'fixture',
                  },
                ],
                selected_executable: `C:/Games/${title}/game.exe`,
                existing_game_id: null,
                duplicate_reason: null,
              })),
              scanned_directories: 10,
              skipped_directories: 0,
              issue_count: 0,
              issues: [],
            };
            break;
          case 'search_metadata':
            data = [candidate(payload.query)];
            break;
          case 'prepare_import_metadata': {
            if (fixture.serial === 0)
              await new Promise((resolve) => {
                fixture.release = resolve;
              });
            fixture.serial += 1;
            if (
              (payload.remote_id === '109' && fixture.failLast) ||
              fixture.manualFail
            ) {
              fixture.failLast = false;
              fixture.manualFail = false;
              return {
                success: false,
                error_code: 'NETWORK_UNAVAILABLE',
                message: '封面下载失败，请重试该条',
                request_id: args.request.request_id,
                data: null,
              };
            }
            data = {
              preparation_id: `prepared-${fixture.serial}`,
              title: fixture.manual
                ? '手动选择作品'
                : titles[Number(payload.remote_id) - 100],
              subtitle: '日本語',
              cover_path: `covers/${'a'.repeat(64)}.jpg`,
              provider: payload.provider,
              remote_id: payload.remote_id,
              translation_message: null,
            };
            fixture.tokens[data.preparation_id] = data;
            break;
          }
          case 'discard_import_metadata':
            payload.preparation_ids.forEach((id) => delete fixture.tokens[id]);
            data = true;
            break;
          case 'import_prepared_game': {
            if (!fixture.tokens[payload.preparation_id])
              throw new Error('Missing prepared data');
            const ready = fixture.tokens[payload.preparation_id];
            data = {
              id: `game-${fixture.games.length}`,
              title: ready.title,
              title_zh: ready.title,
              title_ja: ready.subtitle,
              title_en: null,
              developer: '厂商',
              publisher: '发行商',
              release_date: '2025-01-01',
              source_rating: 8,
              source_tags: ['恋爱'],
              cover_url: null,
              status: 'not_started',
              favorite: false,
              hidden: false,
              user_rating: null,
              total_playtime_seconds: 0,
              last_played_at: null,
              added_at: '2026-10-02T00:00:00Z',
              metadata_status: 'synced',
              installations: [
                {
                  id: `install-${fixture.games.length}`,
                  game_id: `game-${fixture.games.length}`,
                  absolute_path: payload.directory,
                  executable_path: payload.executable_path,
                  source: 'manual',
                  path_valid: true,
                },
              ],
              tags: [],
              has_save_backup: false,
              description: '完整中文资料',
              metadata: [],
            };
            fixture.games.push(data);
            delete fixture.tokens[payload.preparation_id];
            break;
          }
          default:
            throw new Error(`Unexpected IPC: ${command}`);
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
  await page
    .locator('.games-page')
    .getByRole('button', { name: '添加游戏', exact: true })
    .click();
  const dialog = page.getByRole('dialog', { name: '添加本地游戏' });
  await dialog.getByRole('button', { name: '导入游戏目录' }).click();
  const rows = dialog.locator('.review-card');
  await rows.nth(9).waitFor();
  const scrape = dialog.getByRole('button', { name: '开始刮削', exact: true });
  const calls = () => page.evaluate(() => window.__PREPARATION_TEST__.calls);
  await scrape.click();
  await page.waitForFunction(() => !!window.__PREPARATION_TEST__.release);
  assert.equal(
    await rows.nth(0).getAttribute('data-scraped'),
    'false',
    'candidate search alone cannot show success',
  );
  assert.equal(
    (await calls()).filter((c) => c.command === 'import_prepared_game').length,
    0,
  );
  await page.evaluate(() => window.__PREPARATION_TEST__.release());
  await rows.nth(9).locator('[data-status="failed"]').waitFor();
  await scrape.waitFor({ state: 'visible' });
  assert.equal(await dialog.locator('[data-scraped="true"]').count(), 9);
  const initial = await calls();
  assert.equal(
    initial.filter((c) => c.command === 'prepare_import_metadata').length,
    10,
  );
  assert.equal(
    initial.filter((c) => c.command === 'import_prepared_game').length,
    0,
  );
  assert.equal(
    await page.evaluate(() => window.__PREPARATION_TEST__.games.length),
    0,
  );
  console.log(
    '通过：搜索命中仍不亮绿框，完整资料和封面准备成功才亮；首轮九成功、一失败，正式库仍为空',
  );

  await rows.nth(9).locator('.review-search-name input').fill('修正中文名');
  await scrape.click();
  await rows.nth(9).locator('[data-status="scraped"]').waitFor();
  const retried = (await calls()).slice(initial.length);
  assert.equal(
    retried.filter((c) => c.command === 'search_metadata').length,
    1,
  );
  assert.equal(
    retried.filter((c) => c.command === 'prepare_import_metadata').length,
    1,
  );
  assert(
    retried
      .filter((c) => c.command === 'prepare_import_metadata')
      .every((c) => c.payload.directory.endsWith('中文作品10')),
  );
  const beforeSkip = (await calls()).length;
  await scrape.click();
  assert.equal((await calls()).length, beforeSkip);
  console.log(
    '通过：改名后只重试失败的一条；九条成功结果保持不变；全成功后点击刮削不创建任务或网络请求',
  );

  await page.evaluate(() => {
    window.__PREPARATION_TEST__.manualFail = true;
    window.__PREPARATION_TEST__.manual = true;
  });
  await rows.nth(9).getByRole('button', { name: '手动匹配' }).click();
  await dialog.getByRole('button', { name: '搜索', exact: true }).click();
  await dialog.getByRole('button', { name: '使用', exact: true }).click();
  await rows.nth(9).locator('[data-status="failed"]').waitFor();
  const beforeManualRetry = (await calls()).length;
  await scrape.click();
  await rows.nth(9).locator('[data-status="manual"]').waitFor();
  const manualRetry = (await calls()).slice(beforeManualRetry);
  assert.equal(
    manualRetry.filter((c) => c.command === 'search_metadata').length,
    0,
  );
  assert.equal(
    manualRetry.filter((c) => c.command === 'prepare_import_metadata').length,
    1,
  );
  assert.equal(await dialog.locator('[data-scraped="true"]').count(), 10);
  const beforeManualSkip = (await calls()).length;
  await scrape.click();
  assert.equal((await calls()).length, beforeManualSkip);
  console.log(
    '通过：手动匹配的下载失败保留候选，重试只准备这一项且不重新搜索，成功后同样跳过',
  );

  const beforeImport = (await calls()).length;
  await dialog.getByRole('button', { name: '确认导入', exact: true }).click();
  await dialog.waitFor({ state: 'hidden' });
  const importCalls = (await calls()).slice(beforeImport);
  assert.equal(
    importCalls.filter((c) => c.command === 'import_prepared_game').length,
    10,
  );
  assert(
    importCalls
      .filter((c) => c.command === 'import_prepared_game')
      .every((c) => c.payload.preparation_id),
  );
  for (const command of [
    'prepare_import_metadata',
    'search_metadata',
    'confirm_metadata_match',
    'import_game',
  ])
    assert(!importCalls.some((c) => c.command === command));
  assert.equal(
    await page.evaluate(() => window.__PREPARATION_TEST__.games.length),
    10,
  );
  assert.equal(
    await page.evaluate(
      () => Object.keys(window.__PREPARATION_TEST__.tokens).length,
    ),
    0,
  );
  assert.deepEqual(errors, []);
  console.log(
    '通过：确认导入只提交十条准备结果并刷新游戏库，不再次搜索、抓取或下载；浏览器 IPC 夹具，未写真实数据库',
  );
} finally {
  await browser.close();
}
