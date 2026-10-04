// Browser UI acceptance with an isolated IPC fixture. No real scraping or database writes.
import assert from 'node:assert/strict';
import { mkdirSync } from 'node:fs';
import { resolve } from 'node:path';
import { chromium } from '../.tools/browser-check/node_modules/playwright/index.mjs';

const base = process.env.PREVIEW_URL || 'http://127.0.0.1:1420';
assert(['127.0.0.1', 'localhost'].includes(new URL(base).hostname));
const evidence = resolve(
  import.meta.dirname,
  '../.tools/import-feedback-evidence',
);
mkdirSync(evidence, { recursive: true });
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
  page.setDefaultTimeout(7000);
  const errors = [];
  page.on('pageerror', (error) => errors.push(error.message));
  await page.route('**/*', (route) =>
    ['127.0.0.1', 'localhost'].includes(new URL(route.request().url()).hostname)
      ? route.continue()
      : route.abort(),
  );
  await page.addInitScript(() => {
    const fixture = { calls: [], release: null, manual: false };
    window.__IMPORT_FEEDBACK_TEST__ = fixture;
    window.isTauri = true;
    const titles = ['夏色四叶草', '甜蜜女友 3', '美少女万华镜'];
    const candidate = (title) => ({
      provider: 'vndb',
      remote_id: 'v17',
      title,
      subtitle: null,
      cover_url: 'https://t.vndb.org/cv/00/17.jpg',
      confidence: 1,
      matched_fields: ['title'],
      explanation: 'UI test fixture',
      fetched_at: '2026-10-02T00:00:00Z',
      cached: true,
    });
    let callbackId = 0;
    window.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener() {} };
    window.__TAURI_INTERNALS__ = {
      metadata: {
        currentWindow: { label: 'main' },
        currentWebview: { label: 'main' },
      },
      transformCallback: () => ++callbackId,
      unregisterCallback() {},
      invoke: async (command, args = {}) => {
        fixture.calls.push(command);
        if (command === 'plugin:dialog|open') return 'C:/Games';
        if (command === 'plugin:event|listen') return ++callbackId;
        if (command.startsWith('plugin:')) return null;
        const payload = args.request?.payload;
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
          case 'list_games':
            data = { items: [], total: 0, page: 1, page_size: 100 };
            break;
          case 'list_collections':
          case 'get_recommendations':
            data = [];
            break;
          case 'get_home_summary':
            data = {
              game_count: 0,
              playing_count: 0,
              completed_count: 0,
              week_playtime_seconds: 0,
              pending_match_count: 0,
              save_issue_count: 0,
              continue_game_ids: [],
              recent_game_ids: [],
            };
            break;
          case 'bangumi_account':
            data = { status: 'signed_out', profile: null, message: '' };
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
              scanned_directories: 3,
              skipped_directories: 0,
              issue_count: 0,
              issues: [],
            };
            break;
          case 'search_metadata':
            if (fixture.manual || payload.query === titles[0])
              data = [candidate(payload.query)];
            else {
              if (
                payload.query === titles[1] &&
                payload.providers[0] === 'vndb'
              )
                await new Promise((release) => {
                  fixture.release = release;
                });
              data = [];
            }
            break;
          case 'prepare_import_metadata':
            data = {
              preparation_id: `prepared-${payload.directory}`,
              title: payload.directory.split('/').at(-1),
              subtitle: null,
              cover_path: `covers/${'a'.repeat(64)}.jpg`,
              provider: payload.provider,
              remote_id: payload.remote_id,
              translation_message: null,
            };
            break;
          case 'discard_import_metadata':
            data = true;
            break;
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
  await page.locator('.games-page').waitFor();
  await page
    .locator('.games-page')
    .getByRole('button', { name: '添加游戏', exact: true })
    .click();
  const dialog = page.getByRole('dialog', { name: '添加本地游戏' });
  const centered = async () => {
    await page.waitForFunction(() => {
      const rect = document
        .querySelector('.local-import[open]')
        ?.getBoundingClientRect();
      return (
        rect &&
        Math.abs(rect.x + rect.width / 2 - innerWidth / 2) < 1 &&
        Math.abs(rect.y + rect.height / 2 - innerHeight / 2) < 1
      );
    });
  };
  await centered();
  await dialog.getByRole('button', { name: '导入游戏目录' }).click();
  const rows = dialog.locator('.review-card');
  await rows.last().waitFor();
  assert.equal(await rows.count(), 3);
  await centered();
  await dialog.getByRole('button', { name: '开始刮削', exact: true }).click();
  await rows.nth(0).locator('[data-status="scraped"]').waitFor();
  assert.equal(await rows.nth(0).getAttribute('data-scraped'), 'true');
  assert.equal(await rows.nth(0).locator('.review-success-mark').count(), 1);
  assert.equal(
    await rows.nth(1).locator('[data-status="scraping"]').count(),
    1,
  );
  assert.equal(await rows.nth(1).getAttribute('data-scraped'), 'false');
  assert.equal(await rows.nth(2).getAttribute('data-scraped'), 'false');
  assert(
    await dialog
      .getByRole('button', { name: '开始刮削', exact: true })
      .isDisabled(),
  );
  const animations = await rows.nth(0).evaluate((el) => ({
    ring: getComputedStyle(el, '::after').animationName,
    mark: getComputedStyle(el.querySelector('.review-success-mark path'))
      .animationName,
  }));
  assert(animations.ring.includes('import-success-ring'));
  assert(animations.mark.includes('import-check-draw'));
  await page.waitForFunction(() => {
    const mark = document.querySelector('.review-success-mark');
    return mark && getComputedStyle(mark).opacity === '1';
  });
  await page.screenshot({
    path: resolve(evidence, 'scrape-in-progress.png'),
    animations: 'disabled',
  });
  console.log(
    '通过：首条成功立即绿框打勾，第二条仍进行中、第三条待刮削；勾选描画与柔光已接入',
  );

  await page.evaluate(() => window.__IMPORT_FEEDBACK_TEST__.release());
  await page.waitForFunction(
    () =>
      !document.querySelector('.local-import button')?.disabled &&
      document.querySelectorAll('[data-status="no_match"]').length === 2,
  );
  assert.equal(await rows.nth(2).locator('.review-success-mark').count(), 0);
  await page.evaluate(() => {
    window.__IMPORT_FEEDBACK_TEST__.manual = true;
  });
  await rows.nth(1).getByRole('button', { name: '手动匹配' }).click();
  await dialog.getByRole('button', { name: '搜索', exact: true }).click();
  await dialog.getByRole('button', { name: '使用', exact: true }).click();
  await rows.nth(1).locator('[data-status="manual"]').waitFor();
  assert.equal(await rows.nth(1).getAttribute('data-scraped'), 'true');
  assert.equal(await rows.nth(1).locator('.review-success-mark').count(), 1);
  await rows.nth(0).locator('.review-search-name input').fill('新的搜索名称');
  assert.equal(await rows.nth(0).getAttribute('data-scraped'), 'false');
  await rows
    .nth(0)
    .locator('.review-success-mark')
    .waitFor({ state: 'detached' });
  console.log('通过：手动匹配同样高亮，未匹配不冒充成功，改名立即恢复待刮削');

  for (const viewport of [
    { width: 1280, height: 720 },
    { width: 800, height: 600 },
    { width: 480, height: 640 },
  ]) {
    await page.setViewportSize(viewport);
    await centered();
    const box = await dialog.boundingBox();
    assert(
      box.x >= 0 &&
        box.y >= 0 &&
        box.x + box.width <= viewport.width + 1 &&
        box.y + box.height <= viewport.height + 1,
    );
  }
  for (const motion of ['light', 'reduced']) {
    await page.evaluate((value) => {
      document.documentElement.dataset.motion = value;
    }, motion);
    const state = await rows.nth(1).evaluate((el) => ({
      ring: getComputedStyle(el, '::after').animationName,
      ringDuration: getComputedStyle(el, '::after').animationDuration,
      mark: getComputedStyle(el.querySelector('.review-success-mark path'))
        .animationName,
    }));
    if (motion === 'light') assert.equal(state.ringDuration, '0.28s');
    else {
      assert.equal(state.ring, 'none');
      assert.equal(state.mark, 'none');
    }
    assert.equal(await rows.nth(1).getAttribute('data-scraped'), 'true');
  }
  await page.setViewportSize({ width: 1440, height: 1000 });
  await page.evaluate(() => {
    // Use the real theme handler to refresh both root attributes and palette tokens.
    document.querySelector('button[aria-label="切换主题"]').click();
  });
  await page.waitForFunction(
    () => document.documentElement.dataset.theme === 'dark',
  );
  await centered();
  await page.screenshot({
    path: resolve(evidence, 'dark-reduced-motion.png'),
    animations: 'disabled',
  });
  assert.deepEqual(errors, []);
  const calls = await page.evaluate(
    () => window.__IMPORT_FEEDBACK_TEST__.calls,
  );
  assert(
    !calls.includes('import_game') && !calls.includes('confirm_metadata_match'),
  );
  console.log(
    '通过：多尺寸弹窗居中、轻量/减少动画与深色主题；仅浏览器夹具，未写正式数据库',
  );
  console.log(`截图：${evidence}`);
} finally {
  await browser.close();
}
