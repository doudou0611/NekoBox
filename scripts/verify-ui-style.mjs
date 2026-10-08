// Browser visual regression only: demo data, no credentials or native file writes.
import assert from 'node:assert/strict';
import { mkdir, writeFile } from 'node:fs/promises';
import { chromium } from '../.tools/browser-check/node_modules/playwright/index.mjs';
const base = process.env.PREVIEW_URL || 'http://127.0.0.1:1420';
assert(['127.0.0.1', 'localhost'].includes(new URL(base).hostname));
const folder = '.tools/ui-style-evidence';
await mkdir(folder, { recursive: true });
const browser = await chromium.launch({
  headless: true,
  executablePath:
    process.env.BROWSER_EXECUTABLE ||
    '/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge',
});
const measurements = [];
const errors = [];
let layouts = 0;
let dialogs = 0;
try {
  const page = await browser.newPage({ reducedMotion: 'reduce' });
  page.on('pageerror', (error) => errors.push(error.message));
  await page.route('https://**/*', (route) => route.abort());
  async function go(route, selector) {
    await page.goto(`${base}/#${route}`);
    await page.locator(selector).waitFor();
    await page.waitForFunction(
      () => !document.querySelector('.page-enter-active,.page-leave-active'),
    );
  }
  async function appearance(theme, palette = 'wisteria', motion = 'reduced') {
    await page.evaluate(
      async (value) => {
        const { preview } = await import('/src/preview/store.ts');
        preview.theme = value.theme;
        preview.palette = value.palette;
        const { nextTick } = await import('/node_modules/.vite/deps/vue.js');
        await nextTick();
        document.documentElement.dataset.motion = value.motion;
      },
      { theme, palette, motion },
    );
    await page.waitForFunction(
      (value) =>
        document.documentElement.dataset.theme === value.theme &&
        document.documentElement.dataset.palette === value.palette &&
        document.documentElement.dataset.motion === value.motion,
      { theme, palette, motion },
    );
  }
  for (const width of [1440, 1024, 800]) {
    await page.setViewportSize({ width, height: 1000 });
    for (const theme of ['light', 'dark']) {
      for (const [name, route, selector] of [
        ['home', '/', '.home-page'],
        ['games', '/games', '.games-page'],
        ['settings', '/settings', '.settings-page'],
        ['activity', '/activity', '.activity-page'],
        ['saves', '/saves', 'h1[data-page-heading]:has-text("每一次选择")'],
      ]) {
        await go(route, selector);
        await appearance(theme);
        const layout = await page.evaluate(() => {
          const main = document.querySelector('.exhibition-main');
          const root = document.documentElement;
          const style = getComputedStyle(root);
          return {
            viewport: innerWidth,
            document: root.scrollWidth,
            main: main.clientWidth,
            content: main.scrollWidth,
            font: getComputedStyle(document.body).fontFamily,
            space6: style.getPropertyValue('--space-6').trim(),
            space14: style.getPropertyValue('--space-14').trim(),
          };
        });
        assert(
          layout.document <= width + 1,
          `${name}/${theme}/${width}: document overflow`,
        );
        assert(
          layout.content <= layout.main + 1,
          `${name}/${theme}/${width}: main overflow`,
        );
        assert(layout.font.includes('PingFang SC'));
        assert.equal(layout.space6, '6px');
        assert.equal(layout.space14, '14px');
        measurements.push({ name, theme, width, ...layout });
        await page.screenshot({
          path: `${folder}/${name}-${theme}-${width}.png`,
        });
        layouts++;
        // Exercise real mounted shells without triggering save/import/download actions.
        const shells = page.locator('dialog.ui-dialog');
        const seen = new Set();
        for (let i = 0; i < (await shells.count()); i++) {
          const dialog = shells.nth(i);
          const key = await dialog.getAttribute('class');
          if (seen.has(key) || !(await dialog.textContent()).trim()) continue;
          seen.add(key);
          await dialog.evaluate((d) => d.showModal());
          const box = await dialog.evaluate((d) => {
            const r = d.getBoundingClientRect(),
              s = getComputedStyle(d),
              b = getComputedStyle(d, '::backdrop');
            return {
              x: r.x,
              y: r.y,
              width: r.width,
              height: r.height,
              radius: s.borderRadius,
              padding: s.padding,
              shadow: s.boxShadow,
              animation: s.animationName,
              scroll: d.scrollWidth,
              client: d.clientWidth,
              backdrop: b.backgroundColor,
              blur: b.backdropFilter,
            };
          });
          assert(
            Math.abs(box.x + box.width / 2 - width / 2) < 1,
            `${key}: horizontal center`,
          );
          assert(
            Math.abs(box.y + box.height / 2 - 500) < 1,
            `${key}: vertical center`,
          );
          assert(
            box.y >= 23 && box.y + box.height <= 978,
            `${key}: viewport bounds`,
          );
          assert.equal(box.radius, '24px', key);
          assert.equal(box.padding, '28px', key);
          assert.notEqual(box.shadow, 'none', key);
          assert.equal(box.animation, 'none', key);
          assert(box.scroll <= box.client + 1, `${key}: dialog overflow`);
          const disabled = dialog
            .locator('button.primary-button:disabled')
            .first();
          if (await disabled.count()) {
            await disabled.hover({ force: true });
            assert.equal(
              await disabled.evaluate((b) => getComputedStyle(b).transform),
              'none',
            );
          }
          if (width === 1440)
            await page.screenshot({
              path: `${folder}/${name}-${key.split(' ').slice(1).join('-')}-${theme}.png`,
            });
          measurements.push({ key, theme, width, ...box });
          dialogs++;
          await dialog.evaluate((d) => d.close());
        }
      }
    }
  }
  await go('/games', '.games-page');
  await page.setViewportSize({ width: 1280, height: 1000 });
  const group = page.locator('.group-editor');
  const palettes = await page.evaluate(async () =>
    (await import('/src/preview/palettes.ts')).PALETTES.map((p) => p.id),
  );
  for (const theme of ['light', 'dark'])
    for (const palette of palettes) {
      await appearance(theme, palette);
      await group.evaluate((d) => d.showModal());
      const colors = await group.evaluate((d) => ({
        surface: getComputedStyle(d).backgroundColor,
        backdrop: getComputedStyle(d, '::backdrop').backgroundColor,
      }));
      assert.notEqual(colors.surface, 'rgba(0, 0, 0, 0)');
      measurements.push({ theme, palette, ...colors });
      await group.evaluate((d) => d.close());
    }
  for (const motion of ['full', 'light', 'reduced']) {
    await page.emulateMedia({ reducedMotion: 'no-preference' });
    await appearance('dark', 'wisteria', motion);
    await group.evaluate((d) => d.showModal());
    const animation = await group.evaluate(
      (d) => getComputedStyle(d).animationName,
    );
    assert.equal(
      animation,
      { full: 'ui-dialog-enter', light: 'ui-dialog-fade', reduced: 'none' }[
        motion
      ],
    );
    await group.evaluate((d) => d.close());
  }
  // Native material must keep desktop composition while allowing one local
  // modal filter. Browser markers prove the CSS cascade, not the OS compositor.
  for (const material of ['windows-acrylic', 'macos-vibrancy']) {
    for (const theme of ['light', 'dark']) {
      for (const motion of ['full', 'light', 'reduced']) {
        await appearance(theme, 'wisteria', motion);
        await page.evaluate((material) => {
          document.documentElement.dataset.windowMaterial = material;
          document.documentElement.dataset.globalGlass = 'on';
        }, material);
        await group.evaluate((d) => d.showModal());
        const materialStyles = await group.evaluate((d) => ({
          blur: getComputedStyle(d).backdropFilter,
          expectedBlur: `blur(${getComputedStyle(d).getPropertyValue('--panel-blur').trim()})`,
          backdrop: getComputedStyle(d, '::backdrop').backdropFilter,
          main: getComputedStyle(document.querySelector('.exhibition-main'))
            .backdropFilter,
          canvas: getComputedStyle(document.documentElement).backgroundColor,
          button: getComputedStyle(d.querySelector('button')).backdropFilter,
        }));
        assert.equal(materialStyles.blur, materialStyles.expectedBlur);
        assert.equal(materialStyles.backdrop, 'none');
        assert.equal(materialStyles.main, 'none');
        assert.equal(materialStyles.canvas, 'rgba(0, 0, 0, 0)');
        assert.equal(materialStyles.button, 'none');
        measurements.push({ material, theme, motion, ...materialStyles });
        await group.evaluate((d) => d.close());
      }
    }
  }
  for (const fallback of ['off', 'virtual-machine']) {
    await page.evaluate((fallback) => {
      const root = document.documentElement;
      root.dataset.globalGlass = fallback === 'off' ? 'off' : 'on';
      if (fallback === 'virtual-machine') root.dataset.renderProfile = fallback;
    }, fallback);
    await group.evaluate((d) => d.showModal());
    assert.equal(
      await group.evaluate((d) => getComputedStyle(d).backdropFilter),
      'none',
    );
    await group.evaluate((d) => d.close());
  }
  await page.evaluate(() => {
    const root = document.documentElement;
    delete root.dataset.renderProfile;
    root.dataset.windowMaterial = 'browser';
    root.dataset.globalGlass = 'on';
  });
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await appearance('dark', 'wisteria', 'full');
  await group.evaluate((d) => d.showModal());
  assert.equal(
    await group.evaluate((d) => getComputedStyle(d).animationName),
    'none',
  );
  await group.evaluate((d) => d.close());
  // Short windows must scroll inside the shell while keeping its outer bounds visible.
  await page.setViewportSize({ width: 390, height: 450 });
  await group.evaluate((d) => d.showModal());
  const compact = await group.evaluate((d) => ({
    height: d.getBoundingClientRect().height,
    scroll: d.scrollHeight,
    client: d.clientHeight,
    padding: getComputedStyle(d).padding,
    radius: getComputedStyle(d).borderRadius,
  }));
  assert(compact.height <= 418);
  assert(compact.scroll > compact.client);
  assert.equal(compact.padding, '20px');
  assert.equal(compact.radius, '24px');
  await page.screenshot({ path: `${folder}/compact-group.png` });
  assert.deepEqual(errors, []);
  await writeFile(
    `${folder}/results.json`,
    JSON.stringify(
      { layouts, dialogs, palettes: palettes.length * 2, errors, measurements },
      null,
      2,
    ),
  );
  console.log(
    `全局样式验证：${layouts} 组页面、${dialogs} 组实际弹窗、${palettes.length * 2} 组配色及动画/短窗口检查通过。`,
  );
} finally {
  await browser.close();
}
