// Local browser interaction checks; Windows WebView and Acrylic still need a real PC.
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
const evidence = resolve(
  import.meta.dirname,
  '../.tools/sidebar-groups-evidence',
);
mkdirSync(evidence, { recursive: true });
try {
  const page = await browser.newPage({
    viewport: { width: 1280, height: 800 },
  });
  page.setDefaultTimeout(7000);
  const errors = [];
  page.on('pageerror', (error) => errors.push(error.message));
  await page.goto(`${base}/#/games`);
  const rail = page.locator('.navigation-rail');
  if (await rail.getByRole('button', { name: '展开游戏总览侧栏' }).count())
    await rail.getByRole('button', { name: '展开游戏总览侧栏' }).click();
  const dialog = page.getByRole('dialog', { name: /分组/ });
  const group = (name) =>
    rail.getByRole('region', { name: `${name}分组`, exact: true });
  const create = async (name, members) => {
    await rail.getByRole('button', { name: '新建分组', exact: true }).click();
    await dialog.getByRole('textbox', { name: '分组名称' }).fill(name);
    for (const title of members)
      await dialog.getByRole('checkbox', { name: title, exact: true }).check();
    await dialog.getByRole('button', { name: '保存分组' }).click();
    await dialog.waitFor({ state: 'hidden' });
  };
  await group('收藏').waitFor();
  assert.equal(await group('收藏').getByRole('link').count(), 3);
  assert.equal(await group('未分组').getByRole('link').count(), 3);
  assert.equal(await rail.locator('.rail-toggle').count(), 0);

  await rail.getByRole('button', { name: '新建分组', exact: true }).click();
  const nameInput = dialog.getByRole('textbox', { name: '分组名称' });
  assert.equal(
    await nameInput.evaluate((el) => document.activeElement === el),
    true,
  );
  await page.screenshot({
    path: resolve(evidence, 'group-editor.png'),
    animations: 'disabled',
  });
  const editor_bounds = await dialog.boundingBox();
  assert(editor_bounds);
  assert(Math.abs(editor_bounds.x + editor_bounds.width / 2 - 640) < 1);
  assert(Math.abs(editor_bounds.y + editor_bounds.height / 2 - 400) < 1);
  await nameInput.press('Shift+Tab');
  await page.keyboard.press('Shift+Tab');
  assert.equal(
    await dialog
      .getByRole('button', { name: '保存分组' })
      .evaluate((el) => document.activeElement === el),
    true,
  );
  await nameInput.fill('收藏');
  await dialog.getByRole('button', { name: '保存分组' }).click();
  await dialog.getByRole('alert').waitFor();
  await dialog.getByRole('button', { name: '取消', exact: true }).click();
  assert.equal(await rail.locator('.game-group').count(), 2);
  console.log('通过：独立收藏组、标题开关、名称校验、键盘焦点循环和取消');

  await create(' 计划补完 ', ['潮汐寄来的信', '与云同行的夏天']);
  await create('短篇', ['与云同行的夏天']);
  assert.equal(await group('计划补完').getByRole('link').count(), 2);
  assert.equal(await group('短篇').getByRole('link').count(), 1);
  assert.equal(await group('未分组').getByRole('link').count(), 2);
  const heading = page.locator('.games-page [data-page-heading]');
  const gallery = page.locator('.gallery-stage');
  const openGroup = (name) =>
    group(name).getByRole('button', { name: `打开${name}分组`, exact: true });
  const deleteGroup = async (name) => {
    await openGroup(name).click({ button: 'right' });
    await page.getByRole('menuitem', { name: '删除', exact: true }).click();
    await dialog
      .getByRole('button', { name: '确认删除分组', exact: true })
      .click();
    await dialog.waitFor({ state: 'hidden' });
  };
  const assertGallery = async (name, count) => {
    await page.waitForFunction(
      ({ name, count }) =>
        document
          .querySelector('.games-page [data-page-heading]')
          ?.textContent.trim() === name &&
        Number(document.querySelector('.gallery-stage')?.dataset.totalGames) ===
          count,
      { name, count },
    );
    assert(await gallery.evaluate((el) => el.classList.contains('view-grid')));
    assert.equal(await page.locator('.app-topbar').count(), 0);
  };
  await page.evaluate(async () => {
    const { preview } = await import('/src/preview/store.ts');
    preview.gallery_view = 'list';
    preview.gallery_state.year = '1999';
  });
  await page
    .getByRole('searchbox', { name: '搜索作品' })
    .fill('没有结果的旧搜索');
  await page.getByRole('button', { name: '通关', exact: true }).click();
  await page
    .getByRole('searchbox', { name: '搜索作品' })
    .fill('没有结果的旧搜索');
  await openGroup('计划补完').click();
  await assertGallery('计划补完', 2);
  assert.equal(await page.evaluate(() => window.scrollY), 0);
  assert.equal(
    (await page.locator('.gallery-tabs button span').innerText()).trim(),
    '6',
  );
  await page.getByRole('searchbox', { name: '搜索作品' }).fill('查无此项');
  await page.getByRole('button', { name: '清除筛选', exact: true }).click();
  await assertGallery('计划补完', 2);
  assert.equal(
    await page.getByRole('searchbox', { name: '搜索作品' }).inputValue(),
    '',
  );
  assert.equal(
    await page
      .locator('.gallery-tabs')
      .getByRole('button', { name: '计划补完', exact: true })
      .getAttribute('aria-pressed'),
    'true',
  );
  assert.equal(
    await openGroup('计划补完').getAttribute('aria-pressed'),
    'true',
  );
  assert.equal(
    await group('计划补完')
      .getByRole('button', { name: '收起计划补完分组' })
      .getAttribute('aria-expanded'),
    'true',
  );
  await rail.getByRole('link', { name: '游戏', exact: true }).click();
  // Existing smart groups remain supported; the removed page save control is not used.
  await page.evaluate(async () => {
    const { saveSmartCollection, query } =
      await import('/src/stores/library.ts');
    await saveSmartCollection('智能收藏', query({ favorite: true }));
  });
  await openGroup('智能收藏').click();
  await assertGallery('智能收藏', 3);
  await page.locator('main [data-favorite="demo-shore"]').click();
  await assertGallery('智能收藏', 2);
  await rail.getByRole('link', { name: '游戏', exact: true }).click();
  await page.locator('main [data-favorite="demo-shore"]').click();
  await deleteGroup('智能收藏');
  console.log('通过：智能分组复用动态条件，组内取消收藏实时更新成员');
  await openGroup('计划补完').click();
  await group('计划补完')
    .getByRole('button', { name: '收起计划补完分组' })
    .click();
  await assertGallery('计划补完', 2);
  assert.equal(
    await group('计划补完')
      .getByRole('button', { name: '展开计划补完分组' })
      .getAttribute('aria-expanded'),
    'false',
  );
  await openGroup('计划补完').press('Enter');
  await assertGallery('计划补完', 2);
  assert.equal(
    await group('计划补完')
      .getByRole('button', { name: '展开计划补完分组' })
      .getAttribute('aria-expanded'),
    'false',
  );
  await group('计划补完')
    .getByRole('button', { name: '展开计划补完分组' })
    .press('Space');
  await openGroup('收藏').click();
  await assertGallery('收藏', 3);
  await openGroup('未分组').click();
  await assertGallery('未分组', 2);
  await create('空组', []);
  await openGroup('空组').click();
  await page.waitForFunction(
    () =>
      document
        .querySelector('.games-page [data-page-heading]')
        ?.textContent.trim() === '空组',
  );
  assert.equal(await page.locator('.gallery-stage .cover-card').count(), 0);
  await deleteGroup('空组');
  await heading.getByText('所有值得相遇的世界。', { exact: true }).waitFor();
  await rail.getByRole('link', { name: '首页', exact: true }).click();
  await page.waitForURL('**/#/');
  await group('计划补完')
    .getByRole('button', { name: '收起计划补完分组' })
    .click();
  assert(page.url().endsWith('#/'));
  await openGroup('计划补完').click();
  await assertGallery('计划补完', 2);
  assert.equal(
    await group('计划补完')
      .getByRole('button', { name: '展开计划补完分组' })
      .getAttribute('aria-expanded'),
    'false',
  );
  await page.waitForFunction(
    () => !document.querySelector('.page-enter-active, .page-leave-active'),
  );
  await page.screenshot({
    path: resolve(evidence, 'group-grid-navigation.png'),
    animations: 'disabled',
  });
  await rail.getByRole('link', { name: '游戏', exact: true }).click();
  await page.waitForFunction(
    () =>
      Number(document.querySelector('.gallery-stage')?.dataset.totalGames) ===
      6,
  );
  assert.equal((await heading.innerText()).trim(), '所有值得相遇的世界。');
  await group('计划补完')
    .getByRole('button', { name: '展开计划补完分组' })
    .click();
  console.log(
    '通过：组名打开当前分组网格/标题，旧搜索筛选清除，箭头独立折叠，键盘、收藏/未分组/空组、删除回退及游戏入口',
  );
  await group('计划补完')
    .getByRole('button', { name: '收起计划补完分组', exact: true })
    .click();
  await group('计划补完')
    .getByRole('link')
    .first()
    .waitFor({ state: 'hidden' });
  await group('计划补完')
    .getByRole('button', { name: '展开计划补完分组', exact: true })
    .click();
  await openGroup('计划补完').click({ button: 'right' });
  await page.getByRole('menuitem', { name: '管理', exact: true }).click();
  await nameInput.fill('短篇');
  await dialog.getByRole('button', { name: '保存分组' }).click();
  await dialog.getByRole('alert').waitFor();
  await nameInput.fill('正在游玩');
  await dialog
    .getByRole('checkbox', { name: '与云同行的夏天', exact: true })
    .uncheck();
  await dialog
    .getByRole('checkbox', { name: '花与未完成的诗', exact: true })
    .check();
  await dialog.getByRole('button', { name: '保存分组' }).click();
  await group('正在游玩').waitFor();
  assert.equal(await group('正在游玩').getByRole('link').count(), 2);
  assert.equal(await group('未分组').getByRole('link').count(), 1);
  await rail.getByRole('link', { name: '游戏', exact: true }).click();
  const favorite = page.locator('main [data-favorite="demo-sky"]');
  await favorite.click();
  await group('收藏')
    .getByRole('link', { name: '打开与云同行的夏天详情' })
    .waitFor();
  await favorite.click();
  await group('收藏')
    .getByRole('link', { name: '打开与云同行的夏天详情' })
    .waitFor({ state: 'hidden' });
  assert.equal(await group('短篇').getByRole('link').count(), 1);
  console.log('通过：创建、重命名、重复名称、多组成员、组折叠和收藏同步');

  await rail.getByRole('button', { name: '收起游戏总览侧栏' }).click();
  assert.equal(await rail.locator('.compact-games a').count(), 6);
  assert(page.url().endsWith('#/games'));
  await page.evaluate(() => {
    document.documentElement.dataset.windowMaterial = 'windows-acrylic';
  });
  const checkGeometry = async () => {
    await page.waitForFunction(
      () =>
        !document
          .querySelector('.navigation-rail')
          .getAnimations()
          .some((a) => a.playState === 'running'),
    );
    const value = await page.evaluate(() => ({
      rail: document.querySelector('.navigation-rail').getBoundingClientRect()
        .width,
      mask: document.querySelector('.app-ambience').getBoundingClientRect()
        .left,
      main: document.querySelector('.exhibition-main').getBoundingClientRect()
        .left,
    }));
    assert(Math.abs(value.rail - value.mask) < 1);
    assert(Math.abs(value.main - value.mask) < 1);
  };
  await checkGeometry();
  await rail.getByRole('button', { name: '展开游戏总览侧栏' }).click();
  await checkGeometry();
  await page.setViewportSize({ width: 800, height: 600 });
  await checkGeometry();
  await rail.getByRole('button', { name: '新建分组', exact: true }).waitFor();
  assert.equal(
    await rail.evaluate((el) => el.getBoundingClientRect().width),
    292,
  );
  await page.screenshot({
    path: resolve(evidence, 'groups-800.png'),
    animations: 'disabled',
  });
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.evaluate(() => {
    document.documentElement.dataset.windowMaterial = 'browser';
  });
  await page.screenshot({
    path: resolve(evidence, 'groups.png'),
    animations: 'disabled',
  });
  console.log(
    '通过：收起缩略图去重、800px 布局、模糊区域遮罩几何（浏览器模拟）',
  );

  await group('正在游玩')
    .getByRole('link', { name: '打开潮汐寄来的信详情' })
    .click();
  await page.waitForURL('**/#/games/demo-shore');
  await page.waitForFunction(
    () => !document.querySelector('.page-enter-active, .page-leave-active'),
  );
  await openGroup('短篇').click();
  await assertGallery('短篇', 1);
  await group('正在游玩')
    .getByRole('link', { name: '打开潮汐寄来的信详情' })
    .click();
  await page.waitForURL('**/#/games/demo-shore');
  await page.waitForFunction(
    () => !document.querySelector('.page-enter-active, .page-leave-active'),
  );
  await openGroup('正在游玩').click({ button: 'right' });
  await page.getByRole('menuitem', { name: '管理', exact: true }).click();
  await nameInput.fill('未保存的名字');
  await page.keyboard.press('Escape');
  await dialog.waitFor({ state: 'hidden' });
  assert(page.url().endsWith('#/games/demo-shore'));
  await group('正在游玩').waitFor();
  await openGroup('正在游玩').click({ button: 'right' });
  await page.getByRole('menuitem', { name: '管理', exact: true }).click();
  await dialog.getByRole('button', { name: '取消', exact: true }).click();
  await openGroup('正在游玩').click({ button: 'right' });
  await page.getByRole('menuitem', { name: '删除', exact: true }).click();
  assert.equal(await group('正在游玩').count(), 1);
  await dialog.getByRole('button', { name: '取消', exact: true }).click();
  await deleteGroup('正在游玩');
  assert.equal(await group('正在游玩').count(), 0);
  assert.equal(await group('收藏').getByRole('link').count(), 3);
  assert.equal(await group('未分组').getByRole('link').count(), 2);
  await page.waitForFunction(
    () => document.activeElement?.getAttribute('aria-label') === '新建分组',
  );
  await page.reload();
  await group('收藏').waitFor();
  assert.equal(await rail.locator('.game-group').count(), 2);
  assert.equal(await group('未分组').getByRole('link').count(), 3);
  assert.deepEqual(errors, []);
  console.log(
    '通过：详情页 Esc 取消、二次删除确认、删除后焦点恢复、重启重置；无页面异常',
  );
  console.log(`截图：${evidence}`);
} finally {
  await browser.close();
}
