// Focused annual-calendar checks in the real app; no production data or Windows GUI claim.
import assert from 'node:assert/strict';
import { existsSync, mkdirSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { chromium } from '../.tools/browser-check/node_modules/playwright/index.mjs';

const base = process.env.PREVIEW_URL || 'http://127.0.0.1:1420';
assert(['127.0.0.1', 'localhost'].includes(new URL(base).hostname));
const edge = '/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge';
const browser = await chromium.launch({
  headless: true,
  executablePath:
    process.env.BROWSER_EXECUTABLE || (existsSync(edge) ? edge : undefined),
});
const evidence = resolve('.tools/annual-calendar-evidence', String(Date.now()));
mkdirSync(evidence, { recursive: true });
const results = [],
  errors = [],
  layouts = [];
try {
  const page = await browser.newPage({
    viewport: { width: 1280, height: 800 },
  });
  page.on('pageerror', (e) => errors.push(e.message));
  const settled = () =>
    page.waitForFunction(
      () =>
        document
          .querySelector('.activity-content')
          ?.getAttribute('aria-busy') === 'false',
    );
  await page.goto(`${base}/#/activity`);
  await settled();
  const first = await page
    .locator('.calendar-cell')
    .first()
    .getAttribute('data-date');
  const year = Number(first.slice(0, 4));
  const days = (Date.UTC(year + 1, 0, 1) - Date.UTC(year, 0, 1)) / 86400000;
  const activity = await page
    .locator('.calendar-cell:not(.heat-0):not(.is-future)')
    .evaluateAll((cells) => cells.map((c) => [c.dataset.date, c.className]));
  for (const range of ['week', 'days30', 'month', 'year', 'all']) {
    await page.locator(`[data-range="${range}"]`).click();
    await settled();
    assert.equal(await page.locator('.calendar-cell').count(), days);
    assert.equal(
      await page.locator('.calendar-cell').last().getAttribute('data-date'),
      `${year}-12-31`,
    );
    assert.deepEqual(
      await page.locator('.calendar-months span').allTextContents(),
      Array.from({ length: 12 }, (_, i) => `${i + 1}月`),
    );
    assert.deepEqual(
      await page
        .locator('.calendar-cell:not(.heat-0):not(.is-future)')
        .evaluateAll((cells) =>
          cells.map((c) => [c.dataset.date, c.className]),
        ),
      activity,
    );
    assert.equal(await page.getByText('故事不止在结局，').count(), 0);
  }
  results.push(
    '五范围均为完整年度，十二个月标签和全年色阶保持一致，右侧文案移除',
  );

  for (const width of [800, 1024, 1280, 1440, 1920]) {
    await page.setViewportSize({ width, height: 900 });
    const geometry = await page.evaluate(() => {
      const scroll = document.querySelector('.calendar-scroll');
      const cell = document
        .querySelector('.calendar-cell')
        .getBoundingClientRect();
      return {
        width: innerWidth,
        overflow: document.documentElement.scrollWidth > innerWidth,
        scroll: scroll.scrollWidth > scroll.clientWidth,
        cellWidth: cell.width,
        cellHeight: cell.height,
      };
    });
    assert.equal(geometry.overflow, false);
    assert.ok(Math.abs(geometry.cellWidth - geometry.cellHeight) < 1);
    if (width >= 1280) assert.equal(geometry.scroll, false);
    if (width === 800) assert.equal(geometry.scroll, true);
    layouts.push(geometry);
    if ([800, 1280, 1920].includes(width))
      await page
        .locator('.activity-calendar')
        .screenshot({ path: resolve(evidence, `calendar-${width}.png`) });
  }
  results.push('800—1920px 无整页溢出，日期保持正方形，1280px 起完整可见');

  await page.locator('[data-range="month"]').click();
  await settled();
  const outside = page
    .locator('.calendar-cell:disabled:not(.heat-0):not(.is-future)')
    .first();
  assert.ok(await outside.count());
  await outside.hover();
  await page.waitForFunction(() =>
    [...document.querySelectorAll('.activity-tooltip')].some((e) =>
      e.textContent.includes('不在当前统计范围内'),
    ),
  );
  assert.equal(await outside.evaluate((e) => getComputedStyle(e).opacity), '1');
  const future = page.locator('.calendar-cell.is-future').last();
  assert.equal(await future.isDisabled(), true);
  await future.hover();
  await page.waitForFunction(() =>
    [...document.querySelectorAll('.activity-tooltip')].some((e) =>
      e.textContent.includes('未来日期'),
    ),
  );
  results.push('范围外历史日期保留真实颜色及提示，未来日期禁用并明确标记');

  const total = await page.locator('[data-metric="total"]').textContent();
  await page.locator('.calendar-cell:not(:disabled)').first().focus();
  await page.keyboard.press('End');
  assert.equal(await page.locator('.calendar-cell[tabindex="0"]').count(), 1);
  await page.keyboard.press('Enter');
  await settled();
  assert.equal(
    await page.locator('[data-metric="total"]').textContent(),
    total,
  );
  assert.equal(await page.locator('.activity-date-filter').count(), 1);
  await page.getByRole('button', { name: /清除筛选/ }).click();
  await settled();
  results.push('键盘浏览和日期筛选可用，局部筛选不改变总统计');

  await page.locator('[data-range="all"]').click();
  await settled();
  const allTotal = await page.locator('[data-metric="total"]').textContent();
  await page
    .locator('.activity-year-picker select')
    .selectOption(String(year - 1));
  await settled();
  assert.equal(
    await page.locator('.calendar-cell').first().getAttribute('data-date'),
    `${year - 1}-01-01`,
  );
  assert.equal(
    await page.locator('.calendar-cell').last().getAttribute('data-date'),
    `${year - 1}-12-31`,
  );
  assert.equal(
    await page.locator('[data-metric="total"]').textContent(),
    allTotal,
  );
  assert.equal(await page.locator('.activity-date-filter').count(), 0);
  results.push('历史年度切换仅改变日历窗口，并清除日期筛选');
  assert.deepEqual(errors, []);
  writeFileSync(
    resolve(evidence, 'result.json'),
    JSON.stringify(
      {
        results,
        layouts,
        errors,
        environment:
          'macOS Edge headless; isolated preview; Windows GUI unverified',
      },
      null,
      2,
    ),
  );
  for (const result of results) console.log('通过：' + result);
  console.log('证据：' + evidence);
} finally {
  await browser.close();
}
