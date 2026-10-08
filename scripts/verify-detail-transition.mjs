// Local preview only; this verifier never reads the production library.
import assert from 'node:assert/strict';
import { existsSync, mkdirSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
const base = process.env.PREVIEW_URL || 'http://127.0.0.1:1420';
assert(['127.0.0.1', 'localhost'].includes(new URL(base).hostname));
const { chromium } =
  await import('../.tools/browser-check/node_modules/playwright/index.mjs');
const edge = '/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge';
const browser = await chromium.launch({
  headless: true,
  executablePath:
    process.env.BROWSER_EXECUTABLE || (existsSync(edge) ? edge : undefined),
});
const evidence = resolve(
  '.tools/detail-transition-evidence',
  String(Date.now()),
);
mkdirSync(evidence, { recursive: true });
const results = [];
const errors = [];
async function settled(page) {
  await page.waitForFunction(
    () =>
      document.querySelector('.shared-transition-host')?.dataset
        .overlayCount === '0',
  );
  assert.equal(await page.locator('[data-shared-overlay]').count(), 0);
  assert.equal(
    await page
      .locator(
        '[data-shared-title][style*="visibility"], [data-shared-cover][style*="visibility"]',
      )
      .count(),
    0,
  );
}
async function setup(width, theme, title, pause = true) {
  const page = await browser.newPage({ viewport: { width, height: 1000 } });
  page.on('pageerror', (error) => errors.push(error.message));
  await page.goto(base + '/#/games');
  await page.locator('.gallery-stage').waitFor();
  await page.evaluate(
    async ({ theme, title, pause }) => {
      const { preview } = await import('/src/stores/library.ts');
      preview.games = preview.games.slice(0, 1);
      preview.games[0].title = title;
      preview.theme = theme;
      window.titleAnimations = [];
      window.pauseTitleAnimations = pause;
      const original = Element.prototype.animate;
      Element.prototype.animate = function (...args) {
        const animation = original.apply(this, args);
        if (this.matches('[data-shared-overlay], [data-shared-title]')) {
          if (window.pauseTitleAnimations) animation.pause();
          window.titleAnimations.push({ animation, element: this });
        }
        return animation;
      };
    },
    { theme, title, pause },
  );
  return page;
}
async function sample(page, progress) {
  return page.evaluate((progress) => {
    const flight = window.titleAnimations.find(
      ({ element }) => element.dataset.sharedOverlay === 'cover',
    );
    const duration = flight.animation.effect.getTiming().duration;
    for (const { animation } of window.titleAnimations)
      animation.currentTime = duration * progress;
    function geometry(element) {
      const style = getComputedStyle(element);
      const bounds = element.getBoundingClientRect();
      const range = document.createRange();
      range.selectNodeContents(element);
      return {
        width: bounds.width,
        height: bounds.height,
        left: bounds.left,
        top: bounds.top,
        font: style.fontSize,
        lineHeight: style.lineHeight,
        fontWeight: style.fontWeight,
        opacity: Number(style.opacity),
        clamp: style.webkitLineClamp,
        radius:
          (parseFloat(style.borderRadius) * bounds.width) /
          parseFloat(style.width),
        lines: range.getClientRects().length,
      };
    }
    const elements = Object.fromEntries(
      window.titleAnimations.map(({ element }) => [
        element.dataset.sharedOverlay || 'destination',
        geometry(element),
      ]),
    );
    return { progress, ...elements };
  }, progress);
}
async function finish(page) {
  await page.evaluate(() =>
    window.titleAnimations.forEach(({ animation }) => animation.finish()),
  );
  await settled(page);
}
try {
  for (const [width, theme, title] of [
    [1440, 'light', '在夏日的尽头与你重逢'],
    [1280, 'dark', '在夏日的尽头与你重逢'],
    [800, 'light', '在夏日的尽头与你重逢'],
    [1440, 'dark', '在那个漫长夏日的尽头我们终于再次相遇并写下未曾寄出的信'],
    [
      1280,
      'light',
      'AnExtremelyLongUnbrokenGameTitleThatMustStayClampedDuringTheTransition',
    ],
    [1440, 'dark', '潮汐'],
  ]) {
    const page = await setup(width, theme, title);
    const source = await page
      .locator('.gallery-stage [data-shared-title="demo-shore"]')
      .evaluate((element) => {
        const style = getComputedStyle(element),
          bounds = element.getBoundingClientRect();
        return {
          width: bounds.width,
          height: bounds.height,
          font: style.fontSize,
          fontWeight: style.fontWeight,
        };
      });
    const sourceRadius = await page
      .locator('.gallery-stage [data-shared-cover="demo-shore"]')
      .evaluate((element) =>
        parseFloat(getComputedStyle(element).borderRadius),
      );
    await page.locator('[data-preview-open="gallery:demo-shore"]').click();
    await page.waitForFunction(() => window.titleAnimations.length === 3);
    // Keep CSS route fades out of deterministic snapshots of paused WAAPI.
    await page.waitForFunction(
      () => !document.querySelector('.page-enter-active, .page-leave-active'),
    );
    const samples = [];
    for (const progress of [0, 0.1, 0.3, 0.65, 0.999]) {
      const frame = await sample(page, progress);
      samples.push(frame);
      assert(
        Math.abs(frame.title.width - source.width) < 0.1,
        'source title width must never reflow',
      );
      assert(
        Math.abs(frame.title.height - source.height) < 0.1,
        'source title clamp height must remain fixed',
      );
      assert.equal(frame.title.font, source.font);
      assert.equal(frame.title.fontWeight, source.fontWeight);
      assert.equal(frame.title.clamp, '2');
      assert(
        Math.abs(frame.cover.radius - sourceRadius) < 0.08,
        'cover must keep the same visual corner radius throughout scaling',
      );
      if (samples.length > 1) {
        assert(
          Math.abs(frame.destination.width - samples[0].destination.width) <
            0.1,
        );
        assert(
          Math.abs(frame.destination.height - samples[0].destination.height) <
            0.1,
        );
        assert.equal(frame.destination.font, samples[0].destination.font);
      }
      if (progress === 0.3)
        await page.screenshot({
          path: resolve(
            evidence,
            `${width}-${theme}-${results.length}-mid.png`,
          ),
        });
    }
    assert.equal(
      samples[0].destination.opacity,
      0,
      'incoming title must be invisible during its delay',
    );
    assert.equal(samples.at(-1).title.opacity, 0);
    assert.equal(samples.at(-1).destination.opacity, 1);
    if (width === 1440 && title === '在夏日的尽头与你重逢') {
      assert.equal(
        samples[0].title.lines,
        2,
        'fixture must reproduce a two-line grid title',
      );
      assert.equal(
        samples[0].destination.lines,
        1,
        'fixture must reproduce a one-line detail title',
      );
    }
    await finish(page);
    const final = await page
      .locator('[data-detail-heading]')
      .evaluate((element) => {
        const b = element.getBoundingClientRect();
        return {
          width: b.width,
          height: b.height,
          left: b.left,
          top: b.top,
          opacity: Number(getComputedStyle(element).opacity),
        };
      });
    for (const key of ['width', 'height', 'left', 'top'])
      assert(
        Math.abs(final[key] - samples.at(-1).destination[key]) < 0.1,
        `title must not jump at cleanup: ${key}`,
      );
    assert.equal(final.opacity, 1);
    assert.equal(
      await page
        .locator('.detail-cover')
        .evaluate((element) =>
          parseFloat(getComputedStyle(element).borderRadius),
        ),
      sourceRadius,
    );
    // Reverse flight uses the same choreography and leaves the opener focused.
    await page.evaluate(() => {
      window.titleAnimations = [];
    });
    await page.keyboard.press('Escape');
    await page.waitForFunction(() => window.titleAnimations.length === 3);
    const returning = await sample(page, 0.999);
    assert.equal(returning.title.opacity, 0);
    assert.equal(returning.destination.opacity, 1);
    assert(Math.abs(returning.cover.radius - sourceRadius) < 0.08);
    await finish(page);
    assert.equal(
      await page.evaluate(() => document.activeElement?.dataset.previewOpen),
      'gallery:demo-shore',
    );
    results.push({ width, theme, title, samples, returning, final });
    await page.close();
  }
  for (const interruption of ['escape', 'resize', 'wheel']) {
    const page = await setup(1440, 'light', '在夏日的尽头与你重逢');
    await page.locator('[data-preview-open="gallery:demo-shore"]').click();
    await page.waitForFunction(() => window.titleAnimations.length === 3);
    await sample(page, 0.2);
    await page.evaluate(() => {
      window.pauseTitleAnimations = false;
    });
    if (interruption === 'escape') {
      await page.keyboard.press('Escape');
      await page.waitForURL('**/#/games');
    } else if (interruption === 'resize')
      await page.setViewportSize({ width: 1280, height: 900 });
    else await page.mouse.wheel(0, 80);
    await settled(page);
    assert.equal(
      await page
        .locator(
          '[data-detail-heading], .gallery-stage [data-shared-title="demo-shore"]',
        )
        .first()
        .evaluate((element) => getComputedStyle(element).visibility),
      'visible',
    );
    results.push({ interruption, overlayCleaned: true });
    await page.close();
  }
  assert.deepEqual(errors, []);
  writeFileSync(
    resolve(evidence, 'report.json'),
    JSON.stringify(
      { results, errors, windows_native_verified: false },
      null,
      2,
    ),
  );
  console.log(
    `详情过渡回归：${results.length} 个场景通过，标题排版及封面视觉圆角稳定，返回及中断无残留。`,
  );
  console.log(`证据：${evidence}`);
} finally {
  await browser.close();
}
