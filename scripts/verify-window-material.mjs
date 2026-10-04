// CSS composition checks only. A browser cannot prove an OS backdrop is rendered.
import assert from 'node:assert/strict';
import { chromium } from '../.tools/browser-check/node_modules/playwright/index.mjs';
const browser = await chromium.launch({
  headless: true,
  executablePath:
    process.env.BROWSER_EXECUTABLE ||
    '/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge',
});
try {
  const page = await browser.newPage({
    viewport: { width: 1280, height: 800 },
  });
  const errors = [];
  page.on('pageerror', (error) => errors.push(error.message));
  await page.goto('http://127.0.0.1:1420/#/settings');
  const toggle = page.getByRole('checkbox', {
    name: '全局桌面模糊',
    exact: true,
  });
  await toggle.waitFor();
  const values = () =>
    page.evaluate(() => {
      const rail = document.querySelector('.navigation-rail');
      const ambience = document.querySelector('.app-ambience');
      const main = document.querySelector('.exhibition-main');
      return {
        mode: document.documentElement.dataset.windowMaterial,
        canvas: getComputedStyle(document.documentElement).backgroundColor,
        rail: getComputedStyle(rail).backgroundColor,
        filter: getComputedStyle(rail).backdropFilter,
        railWidth: rail.getBoundingClientRect().width,
        contentLeft: main.getBoundingClientRect().left,
        maskLeft: ambience.getBoundingClientRect().left,
        mask: getComputedStyle(ambience).backgroundColor,
      };
    });
  let value = await values();
  assert.equal(value.mode, 'browser');
  assert.notEqual(value.canvas, 'rgba(0, 0, 0, 0)');
  for (const mode of ['macos-vibrancy', 'windows-acrylic']) {
    await page.evaluate((mode) => {
      document.documentElement.dataset.windowMaterial = mode;
    }, mode);
    await page.waitForFunction(
      () =>
        !document
          .querySelector('.app-ambience')
          ?.getAnimations()
          .some((a) => a.playState === 'running'),
    );
    value = await values();
    assert.equal(value.canvas, 'rgba(0, 0, 0, 0)');
    assert.equal(value.filter, 'none');
    assert.notEqual(value.mask, 'rgba(0, 0, 0, 0)');
    assert(Math.abs(value.contentLeft - value.maskLeft) < 1);
    assert(Math.abs(value.railWidth - value.maskLeft) < 1);
  }
  await toggle.uncheck();
  value = await values();
  assert.notEqual(value.canvas, 'rgba(0, 0, 0, 0)');
  await toggle.check();
  await page.getByRole('button', { name: '浅色主题', exact: true }).click();
  value = await values();
  assert.match(value.rail, /0\.62/);
  await page
    .getByRole('button', { name: /^(收起|展开)(游戏总览)?侧栏$/ })
    .click();
  await page.waitForFunction(
    () =>
      !document
        .querySelector('.navigation-rail')
        ?.getAnimations()
        .some((a) => a.playState === 'running'),
  );
  value = await values();
  assert(Math.abs(value.railWidth - value.maskLeft) < 1);
  assert(Math.abs(value.contentLeft - value.maskLeft) < 1);
  await page.evaluate(() => {
    document.documentElement.dataset.windowMaterial = 'solid';
  });
  value = await values();
  assert.notEqual(value.canvas, 'rgba(0, 0, 0, 0)');
  assert.deepEqual(errors, []);
  console.log(
    'PASS: browser fallback, native CSS mask simulation, toggle, light theme, collapsed geometry, error fallback. OS compositor is NOT validated.',
  );
} finally {
  await browser.close();
}
