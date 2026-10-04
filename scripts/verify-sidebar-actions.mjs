// Actual pointer/menu interactions in isolated preview memory only.
import assert from 'node:assert/strict';
import { chromium } from '../.tools/browser-check/node_modules/playwright/index.mjs';
const base = process.env.PREVIEW_URL || 'http://127.0.0.1:1421';
assert(['127.0.0.1', 'localhost'].includes(new URL(base).hostname));
const browser = await chromium.launch({
  headless: true,
  executablePath:
    '/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge',
});
try {
  const page = await browser.newPage({
    viewport: { width: 1280, height: 900 },
  });
  page.setDefaultTimeout(6000);
  const errors = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto(`${base}/#/games`);
  const rail = page.locator('.navigation-rail');
  if (await rail.getByRole('button', { name: '展开游戏总览侧栏' }).count())
    await rail.getByRole('button', { name: '展开游戏总览侧栏' }).click();
  const group = (name) =>
    rail.getByRole('region', { name: `${name}分组`, exact: true });
  const create = async (name, member) => {
    await rail.getByRole('button', { name: '新建分组', exact: true }).click();
    const dialog = page.getByRole('dialog', { name: '新建分组', exact: true });
    await dialog.getByRole('textbox', { name: '分组名称' }).fill(name);
    if (member)
      await dialog.getByRole('checkbox', { name: member, exact: true }).check();
    await dialog.getByRole('button', { name: '保存分组' }).click();
    await dialog.waitFor({ state: 'hidden' });
  };
  await create('排序甲', '潮汐寄来的信');
  await create('排序乙');
  assert.equal(
    await rail.getByRole('button', { name: /^(上移|下移)/ }).count(),
    0,
  );
  const first = group('排序甲').locator('.group-open');
  const second = group('排序乙').locator('.group-heading');
  await second.scrollIntoViewIfNeeded();
  const a = await first.boundingBox(),
    b = await second.boundingBox();
  await page.mouse.move(a.x + a.width / 2, a.y + a.height / 2);
  await page.mouse.down();
  await page.mouse.move(a.x + a.width / 2, a.y + a.height / 2 + 12, {
    steps: 3,
  });
  await page.mouse.move(b.x + b.width / 2, b.y + b.height * 0.8, { steps: 12 });
  await group('排序乙').evaluate((el) => {
    if (!el.classList.contains('is-group-sort-after'))
      throw Error('No after marker');
  });
  await page.mouse.up();
  const headings = await rail
    .locator('[data-group-sort-id] .group-name')
    .allTextContents();
  assert(headings.indexOf('排序乙') < headings.indexOf('排序甲'));
  assert.equal(new URL(page.url()).hash, '#/games');
  const gameLink = () =>
    group('排序甲').getByRole('link', {
      name: '打开潮汐寄来的信详情',
      exact: true,
    });
  await gameLink().click({ button: 'right' });
  assert.deepEqual(await page.getByRole('menuitem').allTextContents(), [
    '重命名',
    '移出当前组',
    '添加到组',
    '删除',
    '管理',
  ]);
  await page.getByRole('menuitem', { name: '添加到组', exact: true }).click();
  const add = page.getByRole('dialog', { name: '添加到分组', exact: true });
  await add.getByRole('combobox').selectOption({ label: '排序乙' });
  await add.getByRole('button', { name: '添加到分组', exact: true }).click();
  await add.waitFor({ state: 'hidden' });
  assert.equal(
    await group('排序乙')
      .getByRole('link', { name: '打开潮汐寄来的信详情' })
      .count(),
    1,
  );
  await gameLink().click({ button: 'right' });
  await page.getByRole('menuitem', { name: '移出当前组' }).click();
  await gameLink().waitFor({ state: 'hidden' });
  const kept = group('排序乙').getByRole('link', {
    name: '打开潮汐寄来的信详情',
  });
  await kept.click({ button: 'right' });
  await page.getByRole('menuitem', { name: '重命名', exact: true }).click();
  const rename = page.getByRole('dialog', { name: '重命名作品' });
  await rename.getByRole('textbox', { name: '作品名称' }).fill('侧栏改名');
  await rename.getByRole('button', { name: '保存名称' }).click();
  await rename.waitFor({ state: 'hidden' });
  const renamed = group('排序乙').getByRole('link', {
    name: '打开侧栏改名详情',
  });
  await renamed.click({ button: 'right' });
  await page.getByRole('menuitem', { name: '删除', exact: true }).click();
  await page
    .getByRole('dialog')
    .filter({ hasText: '删除“侧栏改名”的库记录？' })
    .getByRole('button', { name: '取消', exact: true })
    .click();
  assert.equal(await renamed.count(), 1);
  await renamed.focus();
  await page.keyboard.press('Shift+F10');
  await page.getByRole('menuitem', { name: '管理', exact: true }).click();
  await page.waitForURL('**/games/demo-shore?panel=information');
  assert.deepEqual(errors, []);
  console.log(
    '侧栏菜单、添加/移出、重命名、删除取消、键盘管理及 Pointer 组排序通过。',
  );
} finally {
  await browser.close();
}
