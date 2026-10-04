// Isolated browser preview checks. No real library or account is accessed.
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
const evidence = resolve('.tools/library-context-evidence', String(Date.now()));
mkdirSync(evidence, { recursive: true });
try {
  const page = await browser.newPage({
    viewport: { width: 1280, height: 800 },
  });
  page.setDefaultTimeout(7000);
  const errors = [];
  page.on('pageerror', (error) => errors.push(error.message));
  page.on('console', (message) => {
    if (
      message.type() === 'warning' &&
      /non-element root|TransitionGroup.*single element/.test(message.text())
    )
      errors.push(message.text());
  });
  await page.goto(`${base}/#/games`);
  // Reproduce App.vue's desktop suppression without mocking production data.
  await page.evaluate(() => {
    document.addEventListener('contextmenu', (event) => event.preventDefault());
  });
  const rail = page.locator('.navigation-rail');
  if (await rail.getByRole('button', { name: '展开游戏总览侧栏' }).count())
    await rail.getByRole('button', { name: '展开游戏总览侧栏' }).click();
  await rail.getByRole('button', { name: '新建分组', exact: true }).click();
  const editor = page.getByRole('dialog', { name: '新建分组', exact: true });
  await editor.getByRole('textbox', { name: '分组名称' }).fill('右键验收组');
  await editor
    .getByRole('checkbox', { name: '潮汐寄来的信', exact: true })
    .check();
  await editor.getByRole('button', { name: '保存分组' }).click();
  await editor.waitFor({ state: 'hidden' });
  assert.equal(await rail.getByRole('combobox').count(), 0);
  assert.equal(
    await rail.getByRole('button', { name: /编辑.*分组/ }).count(),
    0,
  );
  const groupButton = (name) =>
    rail.getByRole('button', { name: `打开${name}分组`, exact: true });
  const menuAction = async (trigger, action) => {
    await trigger.click({ button: 'right' });
    assert.deepEqual(await page.getByRole('menuitem').allTextContents(), [
      '重命名',
      '删除',
      '管理',
    ]);
    await page.getByRole('menuitem', { name: action, exact: true }).click();
  };
  await groupButton('右键验收组').focus();
  await page.keyboard.press('Shift+F10');
  await page.getByRole('menu').waitFor();
  await page.keyboard.press('Escape');
  await page.getByRole('menu').waitFor({ state: 'hidden' });
  assert.equal(
    await groupButton('右键验收组').evaluate(
      (el) => document.activeElement === el,
    ),
    true,
  );
  await menuAction(groupButton('右键验收组'), '重命名');
  const renameGroup = page.getByRole('dialog', {
    name: '重命名分组',
    exact: true,
  });
  assert.equal(await renameGroup.getByRole('checkbox').count(), 0);
  await renameGroup.getByRole('textbox', { name: '分组名称' }).fill('收藏');
  await renameGroup.getByRole('button', { name: '保存名称' }).click();
  await renameGroup.getByRole('alert').waitFor();
  await renameGroup.getByRole('textbox', { name: '分组名称' }).fill('新的组名');
  await renameGroup.getByRole('button', { name: '保存名称' }).click();
  await renameGroup.waitFor({ state: 'hidden' });
  assert.equal(
    await rail
      .getByRole('region', { name: '新的组名分组', exact: true })
      .getByRole('link')
      .count(),
    1,
  );
  await menuAction(
    page
      .locator('.gallery-tabs')
      .getByRole('button', { name: '新的组名', exact: true }),
    '管理',
  );
  const manager = page.getByRole('dialog', { name: '管理分组', exact: true });
  assert.equal(
    await manager
      .getByRole('checkbox', { name: '潮汐寄来的信', exact: true })
      .isChecked(),
    true,
  );
  assert.equal(await manager.getByRole('combobox').count(), 0);
  assert.equal(await manager.getByRole('textbox').count(), 1);
  assert.equal(
    await manager
      .getByRole('button', { name: '删除分组', exact: true })
      .count(),
    0,
  );
  await page.screenshot({
    path: resolve(evidence, 'simplified-group-manager.png'),
    animations: 'disabled',
  });
  await manager.getByRole('button', { name: '保存分组' }).click();
  await manager.waitFor({ state: 'hidden' });
  // Existing hidden groups are fixtures, not a removed configuration control.
  await page.evaluate(async () => {
    const { preview } = await import('/src/preview/store.ts');
    const group = preview.groups.find((g) => g.name === '新的组名');
    group.hidden = true;
    group.icon = 'heart';
    group.color = '#123456';
  });
  await rail.locator('.hidden-groups summary').click();
  await menuAction(groupButton('新的组名'), '管理');
  await manager.getByRole('button', { name: '保存分组' }).click();
  await manager.waitFor({ state: 'hidden' });
  assert.deepEqual(
    await page.evaluate(async () => {
      const { preview } = await import('/src/preview/store.ts');
      const group = preview.groups.find((g) => g.name === '新的组名');
      return [group.hidden, group.icon, group.color];
    }),
    [true, 'heart', '#123456'],
  );
  await rail
    .getByRole('button', { name: '恢复新的组名分组显示', exact: true })
    .click();
  await rail.locator('.hidden-groups').waitFor({ state: 'hidden' });
  await menuAction(groupButton('新的组名'), '删除');
  const confirmation = page.getByRole('dialog', { name: /删除.*分组/ });
  await confirmation.getByRole('button', { name: '取消', exact: true }).click();
  await confirmation.waitFor({ state: 'hidden' });
  assert.equal(
    await groupButton('新的组名').evaluate(
      (el) => document.activeElement === el,
    ),
    true,
  );
  await menuAction(groupButton('新的组名'), '删除');
  await confirmation
    .getByRole('button', { name: '确认删除分组', exact: true })
    .click();
  await groupButton('新的组名').waitFor({ state: 'hidden' });
  assert.equal(
    Number(
      await page.locator('.gallery-stage').getAttribute('data-total-games'),
    ),
    6,
  );
  console.log(
    '通过：分组右键、键盘焦点、独立改名、成员保留、标签管理、隐藏恢复与确认删除',
  );
  const card = (id) => page.locator(`.gallery-stage [data-game-id="${id}"]`);
  await menuAction(card('demo-shore'), '重命名');
  const renameGame = page.getByRole('dialog', {
    name: '重命名作品',
    exact: true,
  });
  const gameName = renameGame.getByRole('textbox');
  await gameName.fill('  ');
  await renameGame.getByRole('button', { name: '保存名称' }).click();
  await renameGame.getByRole('alert').waitFor();
  await gameName.fill('右键改名作品');
  await renameGame.getByRole('button', { name: '保存名称' }).click();
  await renameGame.waitFor({ state: 'hidden' });
  await card('demo-shore').getByText('右键改名作品', { exact: true }).waitFor();
  await page.waitForFunction(
    () =>
      document.activeElement?.getAttribute('data-preview-open') ===
      'gallery:demo-shore',
  );
  assert.equal(
    await card('demo-shore')
      .locator('[data-preview-open]')
      .evaluate((el) => document.activeElement === el),
    true,
  );
  await menuAction(card('demo-shore'), '管理');
  await page.waitForURL('**/#/games/demo-shore?panel=information');
  await page.getByRole('tab', { name: '资料', exact: true }).waitFor();
  assert.equal(
    await page
      .getByRole('tab', { name: '资料', exact: true })
      .getAttribute('aria-selected'),
    'true',
  );
  await page.goto(`${base}/#/games`);
  await page.evaluate(() => {
    document.addEventListener('contextmenu', (event) => event.preventDefault());
  });
  await page.locator('.gallery-stage').waitFor();
  await page.getByRole('button', { name: '批量选择', exact: true }).click();
  const others = page.locator('.gallery-stage input[type="checkbox"]').last();
  const otherName = await others.getAttribute('aria-label');
  await others.check();
  await menuAction(card('demo-shore'), '删除');
  const gameDelete = page.getByRole('dialog', { name: /删除.*库记录/ });
  await gameDelete.getByRole('button', { name: '取消', exact: true }).click();
  await gameDelete.waitFor({ state: 'hidden' });
  await card('demo-shore').waitFor();
  await menuAction(card('demo-shore'), '删除');
  await gameDelete
    .getByRole('button', { name: '确认删除', exact: true })
    .click();
  await card('demo-shore').waitFor({ state: 'hidden' });
  assert.equal(
    await page
      .getByRole('checkbox', { name: otherName, exact: true })
      .isChecked(),
    true,
  );
  assert.equal(
    Number(
      await page.locator('.gallery-stage').getAttribute('data-total-games'),
    ),
    5,
  );
  console.log(
    '通过：游戏改名校验与焦点、管理直达资料页、删除取消/确认及无关选择保留',
  );
  await page.setViewportSize({ width: 800, height: 650 });
  await page.emulateMedia({ reducedMotion: 'reduce' });
  const firstCard = page.locator('.gallery-stage [data-game-id]').first();
  await firstCard.evaluate((element) =>
    element.dispatchEvent(
      new MouseEvent('contextmenu', {
        bubbles: true,
        cancelable: true,
        clientX: 797,
        clientY: 647,
      }),
    ),
  );
  await page.getByRole('menu').waitFor();
  await page.waitForFunction(() => {
    const b = document.querySelector('[role=menu]')?.getBoundingClientRect();
    return (
      b &&
      b.x >= 0 &&
      b.y >= 0 &&
      b.right <= innerWidth &&
      b.bottom <= innerHeight
    );
  });
  const bounds = await page.getByRole('menu').boundingBox();
  assert(
    bounds.x >= 0 &&
      bounds.y >= 0 &&
      bounds.x + bounds.width <= 800 &&
      bounds.y + bounds.height <= 650,
  );
  await page.screenshot({
    path: resolve(evidence, 'context-menu-800.png'),
    animations: 'disabled',
  });
  await page.keyboard.press('Escape');
  assert.equal(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= innerWidth,
    ),
    true,
  );
  assert.deepEqual(errors, []);
  console.log(
    '通过：800px 边缘菜单定位、减少动态偏好、无横向溢出和 Vue 动画警告',
  );
  console.log(`截图：${evidence}`);
} finally {
  await browser.close();
}
