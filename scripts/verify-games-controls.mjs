// Local Edge/Vite UI checks; desktop command dispatch is checked with mocked API unit tests.
import assert from 'node:assert/strict';
import { mkdirSync } from 'node:fs';
import { resolve } from 'node:path';
import { chromium } from '../.tools/browser-check/node_modules/playwright/index.mjs';

const base = process.env.PREVIEW_URL || 'http://127.0.0.1:1420';
assert(['127.0.0.1', 'localhost'].includes(new URL(base).hostname));
const evidence = resolve(
  import.meta.dirname,
  '../.tools/games-controls-evidence',
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
    viewport: { width: 1440, height: 1000 },
  });
  const errors = [];
  page.on('pageerror', (error) => errors.push(error.message));
  page.setDefaultTimeout(7000);
  await page.goto(`${base}/#/games`);
  const games = page.locator('.games-page');
  const tabs = games.locator('.gallery-tabs');
  const search = games.getByRole('searchbox', { name: '搜索作品' });
  const batch = games.locator('.batch-toolbar');
  const cards = games.locator('.cover-card');
  await cards.first().waitFor();
  const assertCount = async (count) => {
    await page.waitForFunction(
      (count) =>
        Number(
          document.querySelector('.gallery-stage')?.dataset.totalGames ?? 0,
        ) === count && !document.querySelector('.gallery-leave-active'),
      count,
    );
    assert.equal(await cards.count(), count);
  };
  assert.deepEqual(
    (await tabs.getByRole('button').allTextContents()).map((text) =>
      text.trim(),
    ),
    ['全部作品6', '我的收藏', '在玩', '通关', '搁置', '弃坑'],
  );
  assert.equal(
    await games
      .locator(
        '.system-view-tabs, .collection-tabs, .gallery-extra-filters, select',
      )
      .count(),
    0,
  );
  assert.equal(await batch.count(), 0);
  assert.equal(await cards.locator('input[type=checkbox]').count(), 0);
  await search.fill('潮汐');
  await assertCount(1);
  await search.fill('');
  await games.getByRole('button', { name: '批量选择', exact: true }).click();
  assert.equal(await batch.getByRole('button').count(), 0);
  await games
    .getByRole('checkbox', { name: '选择与云同行的夏天', exact: true })
    .check();
  assert.deepEqual(
    (await batch.getByRole('button').allTextContents()).map((text) =>
      text.trim(),
    ),
    ['删除', '收藏', '添加到分组'],
  );
  await batch.getByRole('button', { name: '添加到分组', exact: true }).click();
  const addDialog = page.getByRole('dialog', {
    name: '添加到分组',
    exact: true,
  });
  assert(
    await addDialog.getByRole('button', { name: '确认添加' }).isDisabled(),
  );
  await addDialog.getByText(/暂无可添加的普通分组/).waitFor();
  await addDialog.getByRole('button', { name: '取消', exact: true }).click();
  await batch.getByRole('button', { name: '收藏', exact: true }).click();
  await page.waitForFunction(async () => {
    const { preview } = await import('/src/stores/library.ts');
    return preview.games.find((game) => game.title === '与云同行的夏天')
      .favorite;
  });
  assert.equal(
    await games
      .getByRole('checkbox', { name: '选择与云同行的夏天', exact: true })
      .isChecked(),
    false,
  );
  await games.getByRole('button', { name: '取消选择', exact: true }).click();
  console.log(
    '通过：精简工具栏、搜索、选择模式、三项操作、无普通组提示及批量收藏',
  );

  await page.evaluate(async () => {
    const { preview, query } = await import('/src/stores/library.ts');
    preview.groups = [
      { group_id: 'reading', name: '计划补完', game_ids: ['demo-shore'] },
      {
        group_id: 'smart',
        name: '智能收藏',
        game_ids: [],
        smart: true,
        query: query({ favorite: true }),
      },
      { group_id: 'hidden', name: '隐藏分组', game_ids: [], hidden: true },
      ...Array.from({ length: 8 }, (_, i) => ({
        group_id: `long-${i}`,
        name: `较长的自建分组 ${i}`,
        game_ids: [],
      })),
    ];
  });
  const labels = (await tabs.getByRole('button').allTextContents()).map(
    (text) => text.trim(),
  );
  assert.deepEqual(labels.slice(0, 8), [
    '全部作品6',
    '我的收藏',
    '在玩',
    '通关',
    '搁置',
    '弃坑',
    '计划补完',
    '智能收藏',
  ]);
  assert(!labels.includes('隐藏分组'));
  await tabs.getByRole('button', { name: '计划补完', exact: true }).click();
  assert.equal(
    (await games.locator('[data-page-heading]').innerText()).trim(),
    '计划补完',
  );
  await assertCount(1);
  await tabs.getByRole('button', { name: '我的收藏', exact: true }).click();
  await assertCount(4);
  await tabs.getByRole('button', { name: /^全部作品/ }).click();
  await games.getByRole('button', { name: '批量选择', exact: true }).click();
  // Clicking a cover in selection mode selects it without opening details.
  await games
    .getByRole('button', { name: '选择月光落在森林里', exact: true })
    .click();
  assert(page.url().endsWith('#/games'));
  assert(
    await games
      .getByRole('checkbox', { name: '选择月光落在森林里', exact: true })
      .isChecked(),
  );
  await batch.getByRole('button', { name: '添加到分组', exact: true }).click();
  const options = await addDialog.locator('option').allTextContents();
  assert(!options.includes('智能收藏'));
  assert(!options.includes('隐藏分组'));
  await addDialog
    .getByRole('combobox', { name: '目标分组' })
    .selectOption('reading');
  await addDialog
    .getByRole('button', { name: '确认添加', exact: true })
    .click();
  await addDialog.waitFor({ state: 'hidden' });
  const members = await page.evaluate(async () => {
    const { preview } = await import('/src/stores/library.ts');
    return preview.groups.find((group) => group.group_id === 'reading')
      .game_ids;
  });
  assert.equal(members.length, 2);
  assert(members.includes('demo-shore'));
  await games.getByRole('button', { name: '取消选择', exact: true }).click();
  await tabs.getByRole('button', { name: '计划补完', exact: true }).click();
  await assertCount(2);
  assert.equal(await cards.locator('input[type=checkbox]').count(), 0);
  console.log(
    '通过：顶部自建分组、系统标签退出组范围、分组标题、追加成员及排除智能/隐藏组',
  );

  await tabs.getByRole('button', { name: /^全部作品/ }).click();
  await games.getByRole('button', { name: '批量选择', exact: true }).click();
  await games
    .getByRole('checkbox', { name: '选择月光落在森林里', exact: true })
    .check();
  await search.fill('潮汐');
  await assertCount(1);
  assert.equal(await batch.getByRole('button').count(), 0);
  await batch
    .getByRole('checkbox', { name: '全选当前结果', exact: true })
    .check();
  assert.equal(await cards.locator('input:checked').count(), 1);
  await batch.getByRole('button', { name: '删除', exact: true }).click();
  const confirm = page.getByRole('dialog', {
    name: '删除 1 部作品的库记录？',
    exact: true,
  });
  await confirm.getByRole('button', { name: '取消', exact: true }).click();
  await confirm.waitFor({ state: 'hidden' });
  await assertCount(1);
  await batch.getByRole('button', { name: '删除', exact: true }).click();
  await confirm.getByRole('button', { name: '确认删除', exact: true }).click();
  await page.waitForFunction(async () => {
    const { preview } = await import('/src/stores/library.ts');
    return (
      preview.games.length === 5 &&
      !preview.groups
        .find((group) => group.group_id === 'reading')
        .game_ids.includes('demo-shore')
    );
  });
  await assertCount(0);
  await search.fill('');
  await games.getByRole('button', { name: '取消选择', exact: true }).click();
  console.log(
    '通过：搜索变化清理隐藏选择、全选当前结果、取消删除、确认删除和关系清理（演示内存）',
  );

  for (const theme of ['light', 'dark']) {
    await page.evaluate(async (theme) => {
      const { preview } = await import('/src/stores/library.ts');
      preview.theme = theme;
    }, theme);
    for (const width of [1440, 900, 600]) {
      await page.setViewportSize({ width, height: 1000 });
      await page.waitForTimeout(150);
      assert(
        await page.evaluate(
          () => document.documentElement.scrollWidth <= innerWidth + 1,
        ),
        `页面在 ${theme}/${width}px 横向溢出`,
      );
      const box = await search.boundingBox();
      assert(box && box.width > 100);
      await page.screenshot({
        path: resolve(evidence, `${theme}-${width}.png`),
        animations: 'disabled',
      });
    }
  }
  assert.deepEqual(errors, []);
  console.log(
    '通过：深浅主题、长分组横向滚动、1440/900/600px 布局，无页面异常',
  );
  console.log(`截图：${evidence}`);
} finally {
  await browser.close();
}
