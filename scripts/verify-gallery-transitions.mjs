// Browser preview regression; never reads the production database or builds packages.
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
  '.tools/gallery-transition-regression',
  String(Date.now()),
);
mkdirSync(evidence, { recursive: true });
const results = [];
const errors = [];
try {
  for (const [width, motion] of [
    [800, 'full'],
    [1280, 'full'],
    [1440, 'full'],
    [1280, 'light'],
    [1280, 'reduced'],
  ]) {
    const page = await browser.newPage({
      viewport: { width, height: 1000 },
      reducedMotion: 'no-preference',
    });
    page.on('pageerror', (error) => errors.push(error.message));
    await page.goto(base + '/#/games');
    await page.locator('.gallery-stage').waitFor();
    await page.evaluate(async (mode) => {
      const { preview } = await import('/src/stores/library.ts');
      const ids = preview.games.map((game) => game.game_id);
      preview.motion_preference = mode;
      preview.groups = [
        { group_id: 'test-a', name: '测试组 A', game_ids: ids.slice(0, 3) },
        { group_id: 'test-b', name: '测试组 B', game_ids: ids.slice(3, 6) },
        { group_id: 'test-c', name: '交集组', game_ids: [ids[1], ids[4]] },
      ];
    }, motion);
    const tabs = page.locator('.gallery-tabs');
    const tab = (name) => tabs.getByRole('button', { name, exact: true });
    async function settled() {
      await page.waitForFunction(
        () =>
          !document.querySelector(
            '.gallery-leave-active, .gallery-enter-active, .gallery-move',
          ),
      );
      const frozen = await page
        .locator(
          '.gallery-stage > .context-card[style*="width"], .gallery-stage > .context-card[style*="left"]',
        )
        .count();
      assert.equal(
        frozen,
        0,
        'active cards must not retain frozen leave styles',
      );
    }
    await tab('测试组 A').click();
    await settled();
    async function verify(name, click, requireLeave = motion !== 'reduced') {
      console.log(`检查：${width}px / ${motion} / ${name}`);
      await page.evaluate(() => {
        const original = [
          ...document.querySelectorAll('.gallery-stage > .context-card'),
        ].map((element) => ({
          element,
          bounds: element.getBoundingClientRect(),
        }));
        const probe = {
          done: false,
          samples: 0,
          maxWidthDelta: 0,
          maxHeightDelta: 0,
          maxLeftDelta: 0,
        };
        window.galleryProbe = probe;
        let frames = 0;
        function sample() {
          for (const { element, bounds } of original) {
            if (
              !element.isConnected ||
              !element.classList.contains('gallery-leave-active')
            )
              continue;
            const current = element.getBoundingClientRect();
            probe.samples++;
            probe.maxWidthDelta = Math.max(
              probe.maxWidthDelta,
              Math.abs(current.width - bounds.width),
            );
            probe.maxHeightDelta = Math.max(
              probe.maxHeightDelta,
              Math.abs(current.height - bounds.height),
            );
            probe.maxLeftDelta = Math.max(
              probe.maxLeftDelta,
              Math.abs(current.left - bounds.left),
            );
          }
          if (++frames < 40) requestAnimationFrame(sample);
          else probe.done = true;
        }
        requestAnimationFrame(sample);
      });
      await click();
      await page.waitForFunction(() => window.galleryProbe?.done);
      const probe = await page.evaluate(() => window.galleryProbe);
      if (requireLeave)
        assert(probe.samples > 0, `${name}: must observe actual leaving cards`);
      assert(
        probe.maxWidthDelta < 1,
        `${name}: width changed by ${probe.maxWidthDelta}px`,
      );
      assert(
        probe.maxHeightDelta < 1,
        `${name}: height changed by ${probe.maxHeightDelta}px`,
      );
      assert(
        probe.maxLeftDelta < 1,
        `${name}: horizontal position jumped by ${probe.maxLeftDelta}px`,
      );
      await settled();
      results.push({ width, motion, name, ...probe });
    }
    await verify('顶栏不同分组', () => tab('测试组 B').click());
    await verify('侧边栏不同分组', () =>
      page
        .getByRole('button', { name: '打开测试组 A分组', exact: true })
        .click(),
    );
    await verify('包含共同游戏的分组', () => tab('交集组').click());
    await tabs.getByRole('button', { name: /^全部作品/ }).click();
    await settled();
    await verify('全部作品切换收藏', () => tab('我的收藏').click());
    await verify('搜索过滤', () =>
      page.getByRole('searchbox', { name: '搜索作品' }).fill('潮汐'),
    );
    await page.getByRole('searchbox', { name: '搜索作品' }).fill('');
    await settled();
    await tab('测试组 A').click();
    await settled();
    await verify('快速连续切换', async () => {
      for (const name of ['测试组 B', '测试组 A', '交集组', '测试组 B']) {
        await tab(name).click();
        await page.waitForTimeout(40);
      }
    });
    const ids = await page
      .locator('.gallery-stage .cover-card')
      .evaluateAll((cards) => cards.map((card) => card.dataset.gameId));
    assert.equal(ids.length, 3);
    assert.equal(new Set(ids).size, 3);
    assert.equal(await page.locator('[data-shared-overlay]').count(), 0);
    await page.screenshot({
      path: resolve(evidence, `${width}-${motion}.png`),
    });
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
    `分组动画回归：${results.length} 个场景通过，退场尺寸及位置稳定，无残留样式。`,
  );
  console.log(`浏览器证据：${evidence}`);
} finally {
  await browser.close();
}
