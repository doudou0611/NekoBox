// Serve built assets: validate the header-free sidebar tools and their popovers.
// Native markers alone still do not prove OS desktop composition.
import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { extname, resolve, sep } from 'node:path';
import { chromium } from '../.tools/browser-check/node_modules/playwright/index.mjs';

const dist = resolve(import.meta.dirname, '../dist');
assert(existsSync(resolve(dist, 'index.html')), 'Run pnpm build first');
const types = {
  '.html': 'text/html',
  '.js': 'text/javascript',
  '.css': 'text/css',
  '.svg': 'image/svg+xml',
  '.png': 'image/png',
  '.jpg': 'image/jpeg',
  '.woff2': 'font/woff2',
};
const server = createServer((request, response) => {
  const pathname = decodeURIComponent(
    new URL(request.url, 'http://127.0.0.1').pathname,
  );
  const path = resolve(dist, `.${pathname === '/' ? '/index.html' : pathname}`);
  if (!path.startsWith(dist + sep) || !existsSync(path)) {
    response.writeHead(404).end();
    return;
  }
  response.writeHead(200, {
    'Content-Type': types[extname(path)] || 'application/octet-stream',
    'Cache-Control': 'no-store',
  });
  response.end(readFileSync(path));
});
await new Promise((done) => server.listen(0, '127.0.0.1', done));
const base = `http://127.0.0.1:${server.address().port}`;
const evidence = resolve(
  import.meta.dirname,
  '../.tools/sidebar-tools-evidence',
  String(Date.now()),
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
    viewport: { width: 1280, height: 800 },
  });
  page.setDefaultTimeout(6000);
  const errors = [];
  const passed = [];
  page.on('pageerror', (error) => errors.push(error.message));
  const inside = async (selector) => {
    await page.waitForFunction((selector) => {
      const node = document.querySelector(selector);
      return (
        node && node.getAnimations().every((a) => a.playState === 'finished')
      );
    }, selector);
    const r = await page.locator(selector).boundingBox();
    const v = page.viewportSize();
    assert(
      r &&
        r.x >= 0 &&
        r.y >= 0 &&
        r.x + r.width <= v.width + 1 &&
        r.y + r.height <= v.height + 1,
      JSON.stringify(r),
    );
  };
  const avatar = () =>
    page.getByRole('button', { name: '打开快捷菜单', exact: true });
  const menu = () => page.locator('.sidebar-quick-panel');
  const settle = async () =>
    page.waitForFunction(
      () =>
        document
          .querySelector('.shared-transition-host')
          ?.getAttribute('data-overlay-count') === '0',
    );
  await page.goto(`${base}/#/`);
  await page.locator('.home-v1').waitFor();
  assert.equal(await page.locator('.app-topbar').count(), 0);
  assert.equal(await page.locator('.rail-caption').count(), 0);
  assert.equal(await page.locator('.sidebar-tools .sidebar-tool').count(), 4);
  assert.equal(
    await page
      .getByRole('button', { name: '返回原展廊位置', exact: true })
      .count(),
    0,
  );
  await page.waitForFunction(() =>
    document
      .querySelector('.home-v1')
      .getAnimations({ subtree: true })
      .every(
        (animation) =>
          animation.effect.getTiming().iterations === Infinity ||
          animation.playState === 'finished',
      ),
  );
  await page.screenshot({ path: resolve(evidence, 'expanded-light.png') });
  await page.evaluate(() => scrollTo(0, 1000));
  await inside('.sidebar-dock');
  const capsule = await page
    .locator('.sidebar-dock-expanded .sidebar-tools')
    .boundingBox();
  const dock = await page.locator('.sidebar-dock-expanded').boundingBox();
  assert(Math.abs(capsule.width / dock.width - 0.75) < 0.01);
  assert(Math.abs(capsule.x - dock.x - (dock.width - capsule.width) / 2) < 1);
  const resize = page.getByRole('separator', { name: '侧栏宽度', exact: true });
  await resize.focus();
  await page.keyboard.press('Home');
  await page.waitForFunction(
    () =>
      document.querySelector('.navigation-rail').getBoundingClientRect()
        .width === 240,
  );
  const compact = await page
    .locator('.sidebar-dock-expanded .sidebar-tools')
    .boundingBox();
  const buttonRects = await page
    .locator('.sidebar-dock .sidebar-tool')
    .evaluateAll((nodes) =>
      nodes.map((n) => {
        const r = n.getBoundingClientRect();
        return { left: r.left, right: r.right };
      }),
    );
  assert(
    buttonRects.every(
      (r) => r.left >= compact.x && r.right <= compact.x + compact.width,
    ),
  );
  await page.screenshot({ path: resolve(evidence, 'minimum-sidebar.png') });
  await resize.dblclick();
  passed.push(
    '无顶栏；四入口固定底部，胶囊居中为原宽度 75%，240px 侧栏按钮完整',
  );

  const theme = await page.evaluate(
    () => document.documentElement.dataset.theme,
  );
  await page.getByRole('button', { name: '切换主题', exact: true }).click();
  await page.waitForFunction(
    (theme) => document.documentElement.dataset.theme !== theme,
    theme,
  );
  await page.getByRole('button', { name: '打开后台任务', exact: true }).click();
  await inside('.operation-popover');
  assert(
    await page
      .locator('.operation-popover')
      .evaluate((el) => !el.closest('.navigation-rail')),
  );
  await page.keyboard.press('Escape');
  await page.locator('.operation-popover').waitFor({ state: 'hidden' });
  assert(
    await page
      .getByRole('button', { name: '打开后台任务', exact: true })
      .evaluate((el) => el === document.activeElement),
  );
  passed.push('主题与任务入口；任务 Portal 避让及 Esc 焦点恢复');

  await page
    .getByRole('button', { name: '收起游戏总览侧栏', exact: true })
    .click();
  await avatar().waitFor();
  assert.equal(
    await page.getByRole('button', { name: '切换主题', exact: true }).count(),
    0,
  );
  await avatar().click();
  await inside('.sidebar-quick-panel');
  assert.equal(await menu().locator('.sidebar-tool').count(), 4);
  await menu().getByRole('button', { name: '切换主题', exact: true }).click();
  await menu().waitFor();
  await menu()
    .getByRole('button', { name: '打开后台任务', exact: true })
    .click();
  await inside('.operation-popover');
  await page.keyboard.press('Escape');
  await page.locator('.operation-popover').waitFor({ state: 'hidden' });
  assert(await menu().isVisible());
  await page.keyboard.press('Escape');
  await menu().waitFor({ state: 'hidden' });
  assert(await avatar().evaluate((el) => el === document.activeElement));
  passed.push('收起头像、四入口框、嵌套任务框逐层关闭');

  await avatar().focus();
  await page.keyboard.press('Enter');
  await menu().waitFor();
  await inside('.sidebar-quick-panel');
  await page.screenshot({ path: resolve(evidence, 'collapsed-menu.png') });
  await menu()
    .getByRole('button', { name: '打开账户与同步', exact: true })
    .click();
  await page.locator('.account-dialog[open]').waitFor();
  assert(
    await page
      .locator('.account-dialog')
      .evaluate((el) => el.contains(document.activeElement)),
  );
  await page.keyboard.press('Escape');
  await page.locator('.account-dialog').waitFor({ state: 'hidden' });
  assert(await avatar().evaluate((el) => el === document.activeElement));
  await avatar().click();
  await inside('.sidebar-quick-panel');
  await page.mouse.click(900, 40);
  await menu().waitFor({ state: 'hidden' });
  passed.push('键盘开框、账户焦点交接/恢复、点击外部关闭');

  await avatar().click();
  await menu().getByRole('link', { name: '设置', exact: true }).click();
  await page.locator('.settings-v1').waitFor();
  await menu().waitFor({ state: 'hidden' });
  await page.locator('.settings-v1-nav button').nth(2).click();
  await page.waitForFunction(
    () =>
      Math.abs(
        document.querySelector('.settings-v1-nav').getBoundingClientRect().top,
      ) <= 1,
  );
  assert.equal(await page.locator('.app-topbar').count(), 0);
  passed.push('快捷设置导航及章节吸顶归零');

  await page.keyboard.press('Control+k');
  await page.locator('[data-library-search]').waitFor();
  assert(
    await page
      .locator('[data-library-search]')
      .evaluate((el) => el === document.activeElement),
  );
  const card = page.locator('.gallery-stage [data-preview-open]').first();
  await card.click();
  await page.locator('.detail-page').waitFor();
  await settle();
  assert.equal(
    await page
      .getByRole('button', { name: '返回原展廊位置', exact: true })
      .count(),
    1,
  );
  assert(await page.locator('.detail-page .detail-navigation').isVisible());
  await avatar().click();
  await page.keyboard.press('Escape');
  await menu().waitFor({ state: 'hidden' });
  assert(await page.locator('.detail-page').isVisible());
  await avatar().click();
  await menu()
    .getByRole('button', { name: '打开账户与同步', exact: true })
    .click();
  await page.locator('.account-dialog[open]').waitFor();
  await page.keyboard.press('Escape');
  await page.locator('.account-dialog').waitFor({ state: 'hidden' });
  assert(await page.locator('.detail-page').isVisible());
  await menu().waitFor({ state: 'hidden' });
  await page.screenshot({ path: resolve(evidence, 'detail-return.png') });
  await page
    .getByRole('button', { name: '返回原展廊位置', exact: true })
    .click();
  await page.locator('.gallery-stage').waitFor();
  await settle();
  assert.equal(
    await page
      .getByRole('button', { name: '返回原展廊位置', exact: true })
      .count(),
    0,
  );
  passed.push('搜索快捷键保留；详情返回与浮层 Esc 互不干扰');

  for (const width of [800, 500, 1280, 1920]) {
    await page.setViewportSize({ width, height: 800 });
    if (
      width > 800 &&
      (await page
        .getByRole('button', { name: '展开游戏总览侧栏', exact: true })
        .count())
    )
      await page
        .getByRole('button', { name: '展开游戏总览侧栏', exact: true })
        .click();
    if (width <= 800) {
      await avatar().waitFor();
      await avatar().click();
      await inside('.sidebar-quick-panel');
      await menu()
        .getByRole('button', { name: '打开后台任务', exact: true })
        .click();
      await inside('.operation-popover');
      await page.keyboard.press('Escape');
      await page.locator('.operation-popover').waitFor({ state: 'hidden' });
      await page.keyboard.press('Escape');
      await menu().waitFor({ state: 'hidden' });
    }
    assert(
      await page.evaluate(
        () => document.documentElement.scrollWidth <= innerWidth,
      ),
    );
    await inside('.sidebar-dock');
  }
  passed.push('500/800/1280/1920 响应布局与浮层避让');
  await page.emulateMedia({ reducedMotion: 'reduce' });

  await page.setViewportSize({ width: 500, height: 800 });
  await avatar().waitFor();
  await avatar().click();
  await inside('.sidebar-quick-panel');
  assert.equal(
    await menu().evaluate((el) => getComputedStyle(el).animationName),
    'none',
  );
  await menu()
    .getByRole('button', { name: '打开后台任务', exact: true })
    .click();
  await inside('.operation-popover');
  assert.equal(
    await page
      .locator('.operation-popover')
      .evaluate((el) => getComputedStyle(el).animationName),
    'none',
  );
  passed.push('系统减少动态效果：快捷框与任务面板静态显示');
  assert.deepEqual(errors, []);
  writeFileSync(
    resolve(evidence, 'result.json'),
    JSON.stringify(
      {
        passed,
        errors,
        assets: 'dist production bundle',
        native_os_verified: false,
      },
      null,
      2,
    ),
  );
  console.log(
    JSON.stringify(
      { passed: true, scenarios: passed.length, evidence },
      null,
      2,
    ),
  );
} finally {
  await browser.close();
  await new Promise((done) => server.close(done));
}
