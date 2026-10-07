// Browser geometry and CSS checks only: no claim of actual Windows Acrylic rendering.
import assert from 'node:assert/strict';
import { mkdirSync } from 'node:fs';
import { resolve } from 'node:path';
import { chromium } from '../.tools/browser-check/node_modules/playwright/index.mjs';
const base = process.env.PREVIEW_URL || 'http://127.0.0.1:1420';
assert(['127.0.0.1', 'localhost'].includes(new URL(base).hostname));
const browser = await chromium.launch({
  headless: true,
  executablePath:
    process.env.BROWSER_EXECUTABLE ||
    '/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge',
});
const evidence = resolve(import.meta.dirname, '../.tools/appearance-evidence');
mkdirSync(evidence, { recursive: true });
try {
  const page = await browser.newPage({
    viewport: { width: 1280, height: 900 },
  });
  page.setDefaultTimeout(7000);
  const errors = [];
  page.on('pageerror', (error) => errors.push(error.message));
  await page.goto(`${base}/#/settings`);
  const rail = page.locator('.navigation-rail');
  if (await rail.getByRole('button', { name: '展开游戏总览侧栏' }).count())
    await rail.getByRole('button', { name: '展开游戏总览侧栏' }).click();
  const widthHandle = page.getByRole('separator', {
    name: '侧栏宽度',
    exact: true,
  });
  const split = page.getByRole('separator', {
    name: '导航与游戏列表占比',
    exact: true,
  });
  const settle = async () => {
    await page.waitForFunction(
      () =>
        !['.navigation-rail', '.app-ambience', '.exhibition-main'].some(
          (selector) =>
            document
              .querySelector(selector)
              .getAnimations()
              .some((a) => a.playState === 'running'),
        ),
    );
  };
  const values = () =>
    page.evaluate(() => {
      const mask = document.querySelector('.app-ambience');
      const rail = document.querySelector('.navigation-rail');
      const style = getComputedStyle(mask);
      return {
        canvas: getComputedStyle(document.documentElement).backgroundColor,
        width: rail.getBoundingClientRect().width,
        left: mask.getBoundingClientRect().left,
        main: document.querySelector('.exhibition-main').getBoundingClientRect()
          .left,
        tint: getComputedStyle(document.querySelector('.exhibition-main'))
          .backgroundColor,
        railTint: getComputedStyle(rail).backgroundColor,
        railFilter: getComputedStyle(rail).backdropFilter,
        panelFilter: getComputedStyle(
          document.querySelector('.settings-card') ?? rail,
        ).backdropFilter,
        maskTransition: style.transitionDuration,
      };
    });
  const drag = async (locator, dx, dy, check) => {
    const rect = await locator.boundingBox();
    assert(rect);
    const x = rect.x + rect.width / 2,
      y = rect.y + rect.height / 2;
    await page.mouse.move(x, y);
    await page.mouse.down();
    await page.mouse.move(x + dx, y + dy, { steps: 6 });
    if (check) await check();
    await page.mouse.up();
    await settle();
  };
  await settle();
  assert.equal((await values()).width, 292);
  await page.evaluate(() => {
    document.documentElement.dataset.windowMaterial = 'windows-acrylic';
  });
  await settle();
  await drag(widthHandle, 110, 0, async () => {
    const value = await values();
    assert(Math.abs(value.width - 402) < 1);
    assert(Math.abs(value.width - value.left) < 1);
    assert(Math.abs(value.width - value.main) < 1);
    assert.equal(value.maskTransition, '0s');
  });
  const initialTop = Number(await split.getAttribute('aria-valuenow'));
  await drag(split, 0, -80);
  assert(Number(await split.getAttribute('aria-valuenow')) < initialTop);
  await split.press('Home');
  assert.equal(
    await split.getAttribute('aria-valuenow'),
    await split.getAttribute('aria-valuemin'),
  );
  await split.press('End');
  assert.equal(
    await split.getAttribute('aria-valuenow'),
    await split.getAttribute('aria-valuemax'),
  );
  await split.dblclick();
  assert.equal(Number(await split.getAttribute('aria-valuenow')), 34);
  await widthHandle.press('End');
  await settle();
  assert.equal((await values()).width, 460);
  await widthHandle.press('ArrowLeft');
  await settle();
  assert.equal((await values()).width, 448);
  await rail.getByRole('button', { name: '收起游戏总览侧栏' }).click();
  await settle();
  assert.equal((await values()).width, 84);
  await rail.getByRole('button', { name: '展开游戏总览侧栏' }).click();
  await settle();
  assert.equal((await values()).width, 448);
  await page.setViewportSize({ width: 800, height: 600 });
  await settle();
  await page.waitForFunction(
    () =>
      window.innerWidth === 800 &&
      Math.round(
        document.querySelector('.navigation-rail').getBoundingClientRect()
          .width,
      ) === 320,
  );
  assert.equal(Math.round((await values()).width), 320);
  await widthHandle.press('Home');
  await settle();
  assert.equal((await values()).width, 240);
  await widthHandle.dblclick();
  await settle();
  assert.equal((await values()).width, 292);
  await page.setViewportSize({ width: 1280, height: 900 });
  console.log(
    '通过：鼠标拖宽度/分区、同步遮罩、键盘边界、双击、收起记忆和最小窗口',
  );

  const global = page.getByRole('checkbox', {
    name: '全局模糊效果',
    exact: true,
  });
  assert.equal(
    await page
      .getByRole('checkbox', { name: '桌面模糊侧栏', exact: true })
      .count(),
    0,
  );
  assert.equal(await page.locator('.app-topbar').count(), 0);
  for (const marker of ['windows-acrylic', 'macos-vibrancy']) {
    await page.evaluate((marker) => {
      document.documentElement.dataset.windowMaterial = marker;
    }, marker);
    // Compatibility modes are CSS-only checks; the current app always defaults to full.
    for (const motion of ['full', 'light', 'reduced']) {
      await page.evaluate((mode) => {
        document.documentElement.dataset.motion = mode;
      }, motion);
      for (const all of [true, false]) {
        await global.setChecked(all);
        await settle();
        const value = await values();
        assert.equal(value.canvas === 'rgba(0, 0, 0, 0)', all);
        if (all) {
          assert.equal(value.panelFilter, 'none');
          assert.equal(value.railFilter, 'none');
          assert(Math.abs(value.left - value.width) < 1);
        }
        assert.equal(/0\.62/.test(value.tint), all);
      }
    }
  }
  await global.check();
  assert.equal(
    await page.getByRole('button', { name: '完整动效', exact: true }).count(),
    0,
  );
  await page.evaluate(() => {
    document.documentElement.dataset.motion = 'full';
  });
  const backgrounds = new Set();
  for (const name of ['紫藤', '海盐', '森屿', '樱霞', '琥珀', '石墨']) {
    await page
      .getByRole('button', { name: `${name}配色`, exact: true })
      .click();
    assert.equal(
      await page
        .getByRole('button', { name: `${name}配色`, exact: true })
        .getAttribute('aria-pressed'),
      'true',
    );
    backgrounds.add(
      await page.evaluate(() =>
        getComputedStyle(document.documentElement).getPropertyValue(
          '--background',
        ),
      ),
    );
  }
  assert.equal(backgrounds.size, 6);
  await page.getByRole('button', { name: '森屿配色', exact: true }).click();
  for (const theme of ['浅色展厅', '深色展厅']) {
    await page
      .getByRole('radio', { name: `主题：${theme}`, exact: true })
      .check();
    assert.equal(
      await page.evaluate(() => document.documentElement.dataset.palette),
      'forest',
    );
    assert.equal((await values()).canvas, 'rgba(0, 0, 0, 0)');
  }
  console.log(
    '通过：两类原生标记 × 三档 CSS 动效 × 模糊开关；六套配色与深浅外观',
  );
  const scrollbar = await page.evaluate(() => {
    const list = document.querySelector('.groups-scroll');
    const thumb = getComputedStyle(list, '::-webkit-scrollbar-thumb');
    return {
      radius: thumb.borderRadius,
      color: thumb.backgroundColor,
      width: getComputedStyle(list, '::-webkit-scrollbar').width,
    };
  });
  assert.equal(scrollbar.width, '9px');
  assert.equal(scrollbar.radius, '999px');
  assert.notEqual(scrollbar.color, 'rgba(0, 0, 0, 0)');
  await page.screenshot({
    path: resolve(evidence, 'settings-forest.png'),
    animations: 'disabled',
    fullPage: true,
  });
  for (const path of [
    '/',
    '/games',
    '/games/demo-shore',
    '/activity',
    '/saves',
    '/settings',
  ]) {
    await page.evaluate((path) => {
      window.location.hash = `#${path}`;
    }, path);
    await page.waitForURL(`**/#${path}`);
    await settle();
    assert.equal((await values()).canvas, 'rgba(0, 0, 0, 0)');
    const expectedTint = await page.evaluate(() =>
      document.documentElement.dataset.theme === 'light' ? '0.62' : '0.6',
    );
    assert.match((await values()).tint, new RegExp(expectedTint));
  }
  await global.uncheck();
  await page.evaluate(() => {
    document.documentElement.dataset.windowMaterial = 'browser';
  });
  assert.notEqual((await values()).canvas, 'rgba(0, 0, 0, 0)');
  await page.screenshot({
    path: resolve(evidence, 'settings-browser.png'),
    animations: 'disabled',
  });
  assert.deepEqual(errors, []);
  console.log(
    '通过：所有六条页面路由的材质背景、浏览器实色回退与主题圆角滚动条；无页面异常',
  );
} finally {
  await browser.close();
}
