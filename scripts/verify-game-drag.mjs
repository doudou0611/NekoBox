// Actual pointer interaction against the isolated browser preview.
import assert from 'node:assert/strict';
import { mkdirSync } from 'node:fs';
import { resolve } from 'node:path';
import { chromium } from '../.tools/browser-check/node_modules/playwright/index.mjs';
const base = process.env.PREVIEW_URL || 'http://127.0.0.1:1421';
assert(['127.0.0.1', 'localhost'].includes(new URL(base).hostname));
const browser = await chromium.launch({
  headless: true,
  executablePath:
    process.env.BROWSER_EXECUTABLE ||
    '/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge',
});
const evidence = resolve('.tools/game-drag-evidence', String(Date.now()));
mkdirSync(evidence, { recursive: true });
try {
  const page = await browser.newPage({
    viewport: { width: 1280, height: 900 },
  });
  page.setDefaultTimeout(7000);
  const errors = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto(`${base}/#/games`);
  const rail = page.locator('.navigation-rail');
  if (await rail.getByRole('button', { name: '展开游戏总览侧栏' }).count())
    await rail.getByRole('button', { name: '展开游戏总览侧栏' }).click();
  const group = (name) =>
    rail.getByRole('region', { name: `${name}分组`, exact: true });
  const create = async (name, members = []) => {
    await rail.getByRole('button', { name: '新建分组', exact: true }).click();
    const dialog = page.getByRole('dialog', { name: '新建分组', exact: true });
    await dialog.getByRole('textbox', { name: '分组名称' }).fill(name);
    for (const title of members)
      await dialog.getByRole('checkbox', { name: title, exact: true }).check();
    await dialog.getByRole('button', { name: '保存分组' }).click();
    await dialog.waitFor({ state: 'hidden' });
  };
  await create('拖入甲组', ['潮汐寄来的信']);
  await create('拖入乙组');
  const target = (name) =>
    group(name).getByRole('button', { name: `打开${name}分组`, exact: true });
  const source = (id) =>
    page.locator(`.gallery-stage [data-game-id="${id}"] .cover-open`);
  const begin = async (element) => {
    await element.scrollIntoViewIfNeeded();
    const b = await element.boundingBox();
    await page.mouse.move(b.x + b.width / 2, b.y + Math.min(35, b.height / 2));
    await page.mouse.down();
    await page.mouse.move(
      b.x + b.width / 2 - 20,
      b.y + Math.min(35, b.height / 2),
      { steps: 4 },
    );
    await page.locator('.game-drag-overlay').waitFor();
  };
  const hover = async (name) => {
    await target(name).scrollIntoViewIfNeeded();
    const b = await target(name).boundingBox();
    await page.mouse.move(b.x + b.width / 2, b.y + b.height / 2, { steps: 10 });
  };
  const drop = async (element, name) => {
    await begin(element);
    await hover(name);
    await group(name).locator('.group-heading').waitFor();
    assert.equal(
      await group(name).evaluate((el) =>
        el.classList.contains('is-game-drop-target'),
      ),
      true,
    );
    await page.mouse.up();
    await page.locator('.game-drag-overlay').waitFor({ state: 'hidden' });
    assert.equal(new URL(page.url()).hash, '#/games');
  };
  await group('拖入甲组')
    .getByRole('button', { name: '收起拖入甲组分组' })
    .click();
  await drop(source('demo-sky'), '拖入甲组');
  await group('拖入甲组')
    .getByRole('link', { name: '打开与云同行的夏天详情', exact: true })
    .waitFor();
  assert.equal(await group('拖入甲组').getByRole('link').count(), 2);
  assert.equal(
    await group('拖入甲组').getByRole('link').first().isVisible(),
    true,
  );
  await drop(
    group('拖入甲组').getByRole('link', {
      name: '打开与云同行的夏天详情',
      exact: true,
    }),
    '拖入乙组',
  );
  await group('拖入乙组')
    .getByRole('link', { name: '打开与云同行的夏天详情', exact: true })
    .waitFor();
  assert.equal(await group('拖入甲组').getByRole('link').count(), 2);
  await drop(source('demo-sky'), '拖入乙组');
  assert.equal(await group('拖入乙组').getByRole('link').count(), 1);
  assert.equal(
    Number(
      await page.locator('.gallery-stage').getAttribute('data-total-games'),
    ),
    6,
  );
  console.log(
    '通过：游戏库和侧栏拖入、即时归组、保留原组与已有成员、重复去重且不打开详情',
  );
  await begin(source('demo-shore'));
  await hover('收藏');
  assert.equal(
    await group('收藏').evaluate((el) =>
      el.classList.contains('is-game-drop-target'),
    ),
    false,
  );
  await page.mouse.up();
  assert.equal(await group('收藏').getByRole('link').count(), 3);
  await begin(source('demo-shore'));
  await hover('拖入乙组');
  await page.keyboard.press('Escape');
  await page.mouse.up();
  await page.locator('.game-drag-overlay').waitFor({ state: 'hidden' });
  assert.equal(await group('拖入乙组').getByRole('link').count(), 1);
  await begin(source('demo-shore'));
  await page.mouse.move(1200, 100, { steps: 10 });
  await page.mouse.up();
  assert.equal(await group('拖入乙组').getByRole('link').count(), 1);
  await target('拖入乙组').dispatchEvent('drop', {
    dataTransfer: await page.evaluateHandle(() => {
      const d = new DataTransfer();
      d.setData('text/plain', 'demo-shore');
      return d;
    }),
  });
  assert.equal(await group('拖入乙组').getByRole('link').count(), 1);
  console.log(
    '通过：系统组拒绝直接加成员、Esc取消、组外放下和外部文字不修改分组',
  );
  await page.evaluate(async () => {
    const { saveSmartCollection, query } =
      await import('/src/stores/library.ts');
    await saveSmartCollection('智能拖入验收', query({ favorite: true }));
  });
  await begin(source('demo-shore'));
  await hover('智能拖入验收');
  assert.equal(
    await group('智能拖入验收').evaluate((el) =>
      el.classList.contains('is-game-drop-target'),
    ),
    false,
  );
  await page.mouse.up();
  assert.equal(await group('智能拖入验收').getByRole('link').count(), 3);
  await begin(source('demo-shore'));
  const original = await source('demo-shore').boundingBox();
  await page.mouse.move(original.x + original.width / 2, original.y + 35, {
    steps: 4,
  });
  await page.keyboard.press('Escape');
  await page.mouse.up();
  assert.equal(new URL(page.url()).hash, '#/games');
  // Ordinary click and the existing context menu must continue to work.
  await source('demo-sky').click({ button: 'right' });
  await page.getByRole('menuitem', { name: '管理', exact: true }).click();
  await page.waitForURL('**/#/games/demo-sky?panel=information');
  await page.getByRole('tab', { name: '资料', exact: true }).waitFor();
  await page.goto(`${base}/#/games`);
  await source('demo-shore').click();
  await page.waitForURL('**/#/games/demo-shore');
  await page.getByRole('button', { name: '返回原展廊位置' }).click();
  await page.waitForURL('**/#/games');
  // Group ordering uses the same native-safe Pointer interaction.
  for (const name of ['拖入甲组', '拖入乙组']) {
    const collapse = group(name).getByRole('button', {
      name: `收起${name}分组`,
      exact: true,
    });
    if (await collapse.count()) await collapse.click();
  }
  const headingA = group('拖入甲组').locator('.group-heading');
  const headingB = group('拖入乙组').locator('.group-heading');
  await headingB.scrollIntoViewIfNeeded();
  await headingA.scrollIntoViewIfNeeded();
  const groupBoxA = await headingA.boundingBox(),
    groupBoxB = await headingB.boundingBox();
  await page.mouse.move(
    groupBoxB.x + groupBoxB.width / 2,
    groupBoxB.y + groupBoxB.height / 2,
  );
  await page.mouse.down();
  await page.mouse.move(
    groupBoxB.x + groupBoxB.width / 2,
    groupBoxB.y + groupBoxB.height / 2 - 12,
    {
      steps: 3,
    },
  );
  await page.mouse.move(
    groupBoxA.x + groupBoxA.width / 2,
    groupBoxA.y + groupBoxA.height * 0.2,
    { steps: 10 },
  );
  await page.mouse.up();
  await page.waitForFunction(() => {
    const names = [...document.querySelectorAll('.game-group .group-name')].map(
      (el) => el.textContent,
    );
    return names.indexOf('拖入乙组') < names.indexOf('拖入甲组');
  });
  const names = await rail.locator('.game-group .group-name').allTextContents();
  assert(names.indexOf('拖入乙组') < names.indexOf('拖入甲组'));
  console.log('通过：普通点击、游戏右键管理、详情返回与 Pointer 组排序');
  await page.setViewportSize({ width: 800, height: 650 });
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await begin(source('demo-shore'));
  await hover('拖入乙组');
  await page.screenshot({
    path: resolve(evidence, 'drag-800.png'),
    animations: 'disabled',
  });
  const b = await page.locator('.game-drag-overlay').boundingBox();
  assert(b.x >= 0 && b.y >= 0 && b.x + b.width <= 800 && b.y + b.height <= 650);
  await page.mouse.up();
  await group('拖入乙组')
    .getByRole('link', { name: '打开潮汐寄来的信详情', exact: true })
    .waitFor();
  assert.equal(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= innerWidth,
    ),
    true,
  );
  for (let i = 0; i < 7; i++) await create(`边缘滚动 ${i}`);
  const list = rail.locator('.groups-scroll').first();
  await list.evaluate((el) => {
    el.scrollTop = 0;
  });
  await begin(source('demo-shore'));
  const listBounds = await list.boundingBox();
  await page.mouse.move(
    listBounds.x + listBounds.width / 2,
    listBounds.y + listBounds.height - 8,
    { steps: 10 },
  );
  await page.waitForFunction(
    () =>
      document.querySelector('#sidebar-game-overview .groups-scroll')
        .scrollTop > 30,
  );
  await page.keyboard.press('Escape');
  await page.mouse.up();
  await page.locator('.game-drag-overlay').waitFor({ state: 'hidden' });
  assert.deepEqual(errors, []);
  console.log(
    '通过：800px拖动反馈、侧栏边缘自动滚动、减少动态效果、无横向溢出或未处理异常',
  );
  console.log(`截图：${evidence}`);
} finally {
  await browser.close();
}
