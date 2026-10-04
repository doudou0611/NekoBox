// Isolated desktop IPC fixture. Never accesses the user's database or account.
import assert from 'node:assert/strict';
import { mkdirSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { chromium } from '../.tools/browser-check/node_modules/playwright/index.mjs';
const base = process.env.PREVIEW_URL || 'http://127.0.0.1:1420';
assert(['127.0.0.1', 'localhost'].includes(new URL(base).hostname));
const evidence = resolve('.tools/hikarinagi-rates-evidence');
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
  const errors = [];
  page.on('pageerror', (error) => errors.push(error.message));
  const source = await (
    await page.request.get(`${base}/src/views/PreviewDetail.vue`)
  ).text();
  const modulePath = (name) =>
    source.match(
      new RegExp(`from "(${name.replaceAll('.', '\\.')}[^" ]*)"`),
    )?.[1];
  const vue = modulePath('/node_modules/.vite/deps/vue.js');
  const router = modulePath('/node_modules/.vite/deps/vue-router.js');
  const library = modulePath('/src/stores/library.ts');
  assert(vue && router && library);
  await page.addInitScript(() => {
    window.isTauri = true;
    window.ratesMode = 'normal';
    window.ratesRequests = [];
    window.reviewAccount = {
      status: 'signed_out',
      profile: null,
      message: '尚未登录',
    };
    window.myReview = null;
    window.submitMode = 'normal';
    window.ratesFixture = {
      remote_id: '897',
      source_url: 'https://www.hikarinagi.org/galgames/897/rates',
      average: 8.2,
      rated_count: 9,
      distribution: [
        { score: 6, count: 2 },
        { score: 7, count: 0 },
        { score: 8, count: 3 },
        { score: 9, count: 2 },
        { score: 10, count: 2 },
      ],
      status_counts: { completed: 18, going: 1, on_hold: 1, dropped: 2 },
      keywords: [
        { word: 'CG', count: 4 },
        { word: '妹妹', count: 3 },
        { word: '体力', count: 2 },
        { word: '剧情', count: 2 },
        { word: '玩法', count: 2 },
        { word: '画风', count: 2 },
      ],
      fetched_at: '2026-10-03T01:00:00Z',
      cached: false,
      stale: false,
      message: null,
    };
    window.ratesGame = {
      id: 'rates-test',
      title: '安利墙隔离测试',
      title_ja: null,
      developer: null,
      publisher: null,
      release_date: null,
      source_rating: null,
      source_tags: [],
      tags: [],
      added_at: '2026-10-03T00:00:00Z',
      last_played_at: null,
      total_playtime_seconds: 0,
      favorite: false,
      hidden: false,
      status: 'not_started',
      cover_url: null,
      user_rating: null,
      metadata_status: 'matched',
      description: '作品简介。\n社区评分显示在右侧，数据与本地用户评分分开。',
      metadata: [
        {
          field: 'title',
          value: '作品',
          provider: 'hikarinagi',
          fetched_at: '2026-10-03T01:00:00Z',
          cached: false,
          manually_edited: false,
        },
      ],
      installations: [],
    };
    window.__TAURI_INTERNALS__ = {
      invoke: async (command, { request }) => {
        window.ratesRequests.push({ command, payload: request.payload });
        let data;
        if (command === 'get_game') data = window.ratesGame;
        else if (command === 'get_hikarinagi_rates') {
          if (window.ratesMode === 'error')
            return {
              success: false,
              error_code: 'NETWORK_UNAVAILABLE',
              message: 'Hikarinagi 安利墙暂时无法获取，请稍后重试。',
              request_id: request.request_id,
              data: null,
            };
          data = {
            wall: window.ratesMode === 'unbound' ? null : window.ratesFixture,
          };
        } else if (command === 'hikarinagi_account')
          data = window.reviewAccount;
        else if (command === 'get_hikarinagi_review') {
          if (window.deferNextReview) {
            window.deferNextReview = false;
            await new Promise((resolve) => (window.finishReviewRead = resolve));
            data = {
              remote_id: '897',
              review: {
                score: 1,
                comment: '过期的加载结果',
                updated_at: '2020-01-01T00:00:00Z',
              },
            };
          } else data = { remote_id: '897', review: window.myReview };
        } else if (command === 'submit_hikarinagi_review') {
          if (window.submitMode === 'failure')
            return {
              success: false,
              error_code: 'NETWORK_UNAVAILABLE',
              message: '未能确认评论是否提交，输入已保留。',
              request_id: request.request_id,
              data: null,
            };
          if (window.submitMode === 'pending')
            await new Promise(
              (resolve) => (window.finishReviewSubmit = resolve),
            );
          window.myReview = {
            score: request.payload.score,
            comment: request.payload.comment.trim(),
            updated_at: '2026-10-03T02:00:00Z',
          };
          data = { review: window.myReview, cache_warning: null };
        } else throw Error(`Unexpected fixture command: ${command}`);
        return {
          success: true,
          error_code: null,
          request_id: request.request_id,
          message: 'isolated fixture',
          data,
        };
      },
    };
  });
  await page.route('**/__rates_detail_test', (route) =>
    route.fulfill({
      contentType: 'text/html',
      body: `<html><head><meta charset="utf-8"></head><body><div id="test-root"></div><script type="module">
  import {createApp} from '${vue}'; import {createRouter,createMemoryHistory,RouterView} from '${router}';
  import Detail from '/src/views/PreviewDetail.vue'; import {local,preview,displayGame} from '${library}';
  import '/src/style.css'; import '/src/appearance.css';
  import {usePreviewAppearance} from '/src/composables/usePreviewAppearance.ts'; usePreviewAppearance();
  local.records['rates-test']=window.ratesGame; preview.games=[displayGame(window.ratesGame,window.ratesGame)];
  window.setRatesTheme=theme=>{preview.theme=theme;document.documentElement.dataset.theme=theme;}; window.setRatesTheme('dark');
  window.setRatesPalette=palette=>{preview.palette=palette;};
  window.rebindRates=()=>{local.records['rates-test']={...local.records['rates-test'],metadata:[]};};
  window.saveLocalRates=()=>{local.records['rates-test']={...local.records['rates-test'],favorite:true,status:'completed'};};
  window.stripRatesMetadata=()=>{const {metadata,...summary}=local.records['rates-test']; local.records['rates-test']=summary;};
  const router=createRouter({history:createMemoryHistory(),routes:[{path:'/games/:game_id',component:Detail}]}); await router.push('/games/rates-test'); await router.isReady();
  createApp(RouterView).use(router).mount('#test-root');
  </script></body></html>`,
    }),
  );
  await page.goto(`${base}/__rates_detail_test`);
  const wall = page.locator('.hikarinagi-wall');
  await wall.getByText('9 人评分', { exact: true }).waitFor();
  assert.equal(await wall.locator('.wall-rating strong').innerText(), '8.2');
  assert.equal(await wall.locator('.wall-stars svg').count(), 10);
  assert.equal(await wall.locator('.wall-stars .filled').count(), 8);
  assert.equal(await wall.locator('.wall-bar-column').count(), 4);
  assert.equal(
    await wall.locator('.wall-bar-column.peak span').innerText(),
    '8',
  );
  assert.equal(
    await wall.locator('.wall-statuses').innerText(),
    '通关\n18\n在玩\n1\n搁置\n1\n弃坑\n2',
  );
  assert.equal(await wall.locator('.wall-keywords li').count(), 6);
  const readCount = () =>
    page.evaluate(
      () =>
        window.ratesRequests.filter((r) => r.command === 'get_hikarinagi_rates')
          .length,
    );
  const beforeSave = await readCount();
  await page.evaluate(() => window.saveLocalRates());
  await page.waitForTimeout(100);
  assert.equal(await readCount(), beforeSave);
  await page.evaluate(() => window.stripRatesMetadata());
  await page.waitForTimeout(100);
  assert.equal(await readCount(), beforeSave);
  await wall.getByText('9 人评分', { exact: true }).waitFor();
  await wall.getByRole('button', { name: '我来评分', exact: true }).click();
  const dialog = page.getByRole('dialog', { name: '为这部作品评分' });
  await dialog.getByText(/尚未登录 Hikarinagi/).waitFor();
  const center = await dialog.boundingBox();
  assert(Math.abs(center.x + center.width / 2 - 720) < 2);
  assert(Math.abs(center.y + center.height / 2 - 500) < 2);
  assert.equal(
    await dialog
      .getByRole('button', { name: '评论', exact: true })
      .isDisabled(),
    true,
  );
  assert.equal(await dialog.getByRole('radio').count(), 10);
  assert.equal(
    await dialog.getByLabel('评论', { exact: true }).isDisabled(),
    true,
  );
  await dialog.getByRole('button', { name: '关闭评分窗口' }).click();
  await page.evaluate(() => {
    window.reviewAccount = {
      status: 'authenticated',
      profile: {
        id: 7,
        username: 'fixture',
        nickname: '测试用户',
        avatar_url: null,
      },
      message: '已登录',
    };
  });
  await wall.getByRole('button', { name: '我来评分', exact: true }).click();
  await dialog.getByText('评论账号：测试用户').waitFor();
  await dialog.getByRole('radio', { name: '8 颗星', exact: true }).click();
  await page.mouse.move(0, 0);
  assert.equal(await dialog.locator('.review-stars .filled').count(), 8);
  await dialog
    .getByRole('radio', { name: '8 颗星', exact: true })
    .press('ArrowRight');
  assert.equal(
    await dialog
      .getByRole('radio', { name: '9 颗星', exact: true })
      .getAttribute('aria-checked'),
    'true',
  );
  await dialog.getByRole('radio', { name: '9 颗星', exact: true }).press('End');
  assert.equal(await dialog.locator('.review-stars .filled').count(), 10);
  await dialog
    .getByRole('radio', { name: '10 颗星', exact: true })
    .press('Home');
  assert.equal(await dialog.locator('.review-stars .filled').count(), 1);
  await dialog.getByRole('radio', { name: '8 颗星', exact: true }).click();
  await dialog
    .getByLabel('评论', { exact: true })
    .fill('这是隔离测试评论，不会发布到网站。');
  for (const theme of ['dark', 'light']) {
    await page.evaluate((theme) => window.setRatesTheme(theme), theme);
    assert.equal(
      await dialog
        .getByRole('button', { name: '评论', exact: true })
        .isDisabled(),
      false,
    );
    await page.waitForTimeout(350);
    await dialog.screenshot({
      path: resolve(evidence, `${theme}-review-dialog.png`),
    });
  }
  await page.evaluate(() => (window.submitMode = 'failure'));
  await dialog.getByRole('button', { name: '评论', exact: true }).click();
  await dialog
    .getByRole('alert')
    .getByText(/输入已保留/)
    .waitFor();
  assert.equal(
    await dialog.getByLabel('评论', { exact: true }).inputValue(),
    '这是隔离测试评论，不会发布到网站。',
  );
  assert.equal(
    await dialog
      .getByRole('radio', { name: '8 颗星', exact: true })
      .getAttribute('aria-checked'),
    'true',
  );
  await page.evaluate(() => (window.submitMode = 'pending'));
  await dialog.getByRole('button', { name: '评论', exact: true }).click();
  const close = dialog.getByRole('button', { name: '关闭评分窗口' });
  assert.equal(await close.isDisabled(), true);
  await page.keyboard.press('Escape');
  assert.equal(await dialog.isVisible(), true);
  assert.equal(
    await page.evaluate(
      () =>
        window.ratesRequests.filter(
          (r) => r.command === 'submit_hikarinagi_review',
        ).length,
    ),
    2,
  );
  await page.waitForFunction(
    () => typeof window.finishReviewSubmit === 'function',
  );
  await page.evaluate(() => window.finishReviewSubmit());
  await dialog.waitFor({ state: 'hidden' });
  assert.equal(await readCount(), beforeSave);
  await wall.getByText('9 人评分', { exact: true }).waitFor();
  assert.deepEqual(
    await page.evaluate(
      () =>
        window.ratesRequests.filter(
          (r) => r.command === 'submit_hikarinagi_review',
        )[0].payload,
    ),
    {
      game_id: 'rates-test',
      remote_id: '897',
      account_id: 7,
      score: 8,
      comment: '这是隔离测试评论，不会发布到网站。',
      expected_updated_at: null,
    },
  );
  await wall.getByRole('button', { name: '我来评分', exact: true }).click();
  await dialog.getByText(/已载入你在该作品下的评分/).waitFor();
  assert.equal(
    await dialog.getByLabel('评论', { exact: true }).inputValue(),
    '这是隔离测试评论，不会发布到网站。',
  );
  assert.equal(
    await dialog
      .getByRole('radio', { name: '8 颗星', exact: true })
      .getAttribute('aria-checked'),
    'true',
  );
  await dialog.getByRole('button', { name: '关闭评分窗口' }).click();
  // Changing the selected palette updates the histogram immediately.
  await page.evaluate(() => (window.deferNextReview = true));
  await wall.getByRole('button', { name: '我来评分', exact: true }).click();
  await page.waitForFunction(
    () => typeof window.finishReviewRead === 'function',
  );
  await page.evaluate(() => {
    document.querySelector('[aria-label="关闭评分窗口"]').click();
    document.querySelector('.wall-rating-action').click();
  });
  await dialog.getByText(/已载入你在该作品下的评分/).waitFor();
  await page.evaluate(() => window.finishReviewRead());
  await page.waitForTimeout(100);
  assert.equal(
    await dialog.getByLabel('评论', { exact: true }).inputValue(),
    '这是隔离测试评论，不会发布到网站。',
  );
  assert.equal(
    await dialog
      .getByRole('button', { name: '评论', exact: true })
      .isDisabled(),
    false,
  );
  await dialog.getByRole('button', { name: '关闭评分窗口' }).click();
  const colors = [];
  for (const palette of ['wisteria', 'forest']) {
    await page.evaluate((palette) => window.setRatesPalette(palette), palette);
    await page.waitForTimeout(50);
    const color = await wall
      .locator('.wall-bar-column.peak .wall-bar')
      .evaluate((el) => ({
        actual: getComputedStyle(el).backgroundColor,
        expected: getComputedStyle(document.documentElement)
          .getPropertyValue('--accent')
          .trim(),
      }));
    const expected = await page.evaluate((hex) => {
      const element = document.createElement('span');
      element.style.color = hex;
      document.body.append(element);
      const value = getComputedStyle(element).color;
      element.remove();
      return value;
    }, color.expected);
    assert.equal(color.actual, expected);
    colors.push(color.actual);
  }
  assert.notEqual(colors[0], colors[1]);
  const layouts = [];
  for (const theme of ['dark', 'light']) {
    await page.evaluate((theme) => window.setRatesTheme(theme), theme);
    for (const width of [1440, 1100, 900, 720]) {
      await page.setViewportSize({ width, height: 1000 });
      const layout = await page.evaluate(() => {
        const wall = document
          .querySelector('.hikarinagi-wall')
          .getBoundingClientRect();
        const intro = document
          .querySelector('.detail-panel--overview > div')
          .getBoundingClientRect();
        return {
          wall: { x: wall.x, y: wall.y, right: wall.right, width: wall.width },
          intro: {
            x: intro.x,
            y: intro.y,
            bottom: intro.bottom,
            right: intro.right,
          },
          viewport: innerWidth,
        };
      });
      assert(layout.wall.right <= width + 1, JSON.stringify(layout));
      if (width > 1000) assert(layout.wall.x >= layout.intro.right - 1);
      else assert(layout.wall.y >= layout.intro.bottom - 1);
      layouts.push({ theme, width, ...layout });
      await wall.screenshot({
        path: resolve(evidence, `${theme}-${width}-wall.png`),
      });
      if (width === 1440)
        await page
          .locator('.detail-panel--overview')
          .screenshot({ path: resolve(evidence, `${theme}-overview.png`) });
    }
  }
  await page.evaluate(() => {
    window.ratesMode = 'unbound';
    window.rebindRates();
  });
  await wall.getByText(/尚未匹配 Hikarinagi/).waitFor();
  assert.equal(await wall.locator('.wall-rating').count(), 0);
  await page.evaluate(() => (window.ratesMode = 'error'));
  await wall.getByRole('button', { name: '刷新 Hikarinagi 安利墙' }).click();
  await wall.getByRole('alert').waitFor();
  assert.equal(await wall.locator('.wall-rating').count(), 0);
  await page.evaluate(() => {
    window.ratesMode = 'normal';
    window.ratesFixture = {
      ...window.ratesFixture,
      cached: true,
      stale: true,
      message: '正在显示上次获取的数据。',
    };
  });
  await wall.getByRole('button', { name: '刷新 Hikarinagi 安利墙' }).click();
  await wall.getByText('正在显示上次获取的数据。', { exact: true }).waitFor();
  assert.match(
    await wall.locator('.wall-source').innerText(),
    /缓存于.*2026.*待更新/s,
  );
  await page.evaluate(() => {
    window.ratesFixture = {
      ...window.ratesFixture,
      average: null,
      rated_count: 0,
      distribution: [],
      status_counts: { completed: 0, going: 0, on_hold: 0, dropped: 0 },
      keywords: [],
      cached: false,
      stale: false,
      message: null,
    };
  });
  await wall.getByRole('button', { name: '刷新 Hikarinagi 安利墙' }).click();
  await wall.getByText('0 人评分', { exact: true }).waitFor();
  assert.equal(await wall.locator('.wall-rating strong').innerText(), '—');
  assert.equal(
    await wall.locator('.wall-histogram,.wall-statuses,.wall-keywords').count(),
    0,
  );
  const requests = await page.evaluate(() => window.ratesRequests);
  assert(
    requests.some(
      (r) => r.command === 'get_hikarinagi_rates' && r.payload.refresh === true,
    ),
  );
  assert.deepEqual(errors, []);
  const preview = await browser.newPage();
  await preview.goto(`${base}/#/games/demo-shore`);
  // Production browser preview must neither fetch community statistics nor invent them.
  await preview
    .locator('.hikarinagi-wall')
    .getByText(/浏览器预览不获取社区评分/)
    .waitFor();
  assert.equal(await preview.locator('.wall-rating').count(), 0);
  writeFileSync(
    resolve(evidence, 'result.json'),
    JSON.stringify(
      { layouts, requests, errors, previewNoStatistics: true },
      null,
      2,
    ),
  );
  console.log(
    `Hikarinagi wall detail/browser regressions passed; evidence: ${evidence}`,
  );
} finally {
  await browser.close();
}
