// Optional browser verification for the local Vite preview; not a Windows test.
// Install verifier only: npm install --prefix .tools/browser-check --save-exact playwright
import assert from 'node:assert/strict';
import { existsSync, mkdirSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
const root = resolve(import.meta.dirname, '..');
const base = process.env.PREVIEW_URL || 'http://127.0.0.1:1420';
assert(
  ['127.0.0.1', 'localhost'].includes(new URL(base).hostname),
  'Only local preview URLs are allowed.',
);
const modulePath = new URL(
  '../.tools/browser-check/node_modules/playwright/index.mjs',
  import.meta.url,
);
if (!existsSync(modulePath))
  throw Error(
    '先运行 npm install --prefix .tools/browser-check --save-exact playwright。',
  );
const { chromium } = await import(modulePath.href);
const systemEdge =
  process.platform === 'darwin'
    ? '/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge'
    : process.platform === 'win32'
      ? resolve(
          process.env['PROGRAMFILES(X86)'] || 'C:/Program Files (x86)',
          'Microsoft/Edge/Application/msedge.exe',
        )
      : undefined;
const executablePath =
  process.env.BROWSER_EXECUTABLE ||
  (systemEdge && existsSync(systemEdge) ? systemEdge : undefined);
const browser = await chromium.launch({ headless: true, executablePath });
const evidence = resolve(root, '.tools/preview-evidence', String(Date.now()));
mkdirSync(evidence, { recursive: true });
const failures = [];
const passed = [];
const errors = [];
const measurements = [];
const context = await browser.newContext({
  viewport: { width: 1440, height: 1000 },
  reducedMotion: 'no-preference',
});
const page = await context.newPage();
page.setDefaultTimeout(7000);
page.setDefaultNavigationTimeout(15000);
page.on('pageerror', (error) => errors.push(error.message));
const external = [];
await context.route('**/*', async (route) => {
  const url = new URL(route.request().url());
  if (
    ['http:', 'https:'].includes(url.protocol) &&
    !['127.0.0.1', 'localhost'].includes(url.hostname)
  ) {
    external.push(url.href);
    await route.abort();
  } else await route.continue();
});
async function step(name, run) {
  try {
    if (page.url().startsWith(base))
      await page.evaluate(async () => {
        const { resetPreview } = await import('/src/preview/store.ts');
        const { selectGalleryGroup } =
          await import('/src/services/groupNavigation.ts');
        resetPreview();
        selectGalleryGroup();
      });
    await run();
    passed.push(name);
    console.log('通过：' + name);
  } catch (error) {
    failures.push({ name, error: error.message });
    console.error('失败：' + name + '\n' + error.stack);
  }
}
async function settled() {
  await page.waitForFunction(
    () => {
      const host = document.querySelector('.shared-transition-host');
      return (
        host &&
        host.getAttribute('data-overlay-count') === '0' &&
        !['opening', 'closing'].includes(
          host.getAttribute('data-transition-phase'),
        )
      );
    },
    undefined,
    { timeout: 5000 },
  );
  assert.equal(await page.locator('[data-shared-overlay]').count(), 0);
  assert.equal(
    await page
      .locator(
        '[data-shared-cover][style*="visibility: hidden"], [data-shared-title][style*="visibility: hidden"]',
      )
      .count(),
    0,
  );
}
async function storePatch(patch) {
  await page.evaluate(async (value) => {
    const { preview } = await import('/src/preview/store.ts');
    Object.assign(preview, value);
  }, patch);
}
function routeSelector(path) {
  return path === '/'
    ? '.home-page'
    : path === '/games'
      ? '.games-page'
      : path === '/settings'
        ? '.settings-page'
        : path === '/activity' || path === '/saves'
          ? '[data-page-heading]'
          : path.startsWith('/games/')
            ? '[data-detail-stage]'
            : '.unavailable-page';
}
async function waitRouteContent(path) {
  if (path === '/activity' || path === '/saves')
    await page
      .getByRole('heading', {
        name:
          path === '/activity'
            ? '留在故事里的时间。'
            : '每一次选择，都值得留存。',
        exact: true,
      })
      .waitFor();
  await page.locator(routeSelector(path)).first().waitFor({ state: 'visible' });
  await page.waitForFunction(
    () => !document.querySelector('.page-enter-active, .page-leave-active'),
    undefined,
    { timeout: 5000 },
  );
}
async function waitRoute(path) {
  await waitRouteContent(path);
  await settled();
}
async function go(path) {
  await page.goto(base + '/#' + path);
  await page.locator('.shared-transition-host').waitFor({ state: 'attached' });
  await waitRoute(path);
}
try {
  await step('首页主导航与固定顶栏设置入口；移除右上角搜索', async () => {
    await go('/');
    assert.match(await page.locator('body').innerText(), /原型|演示/);
    for (const name of ['首页', '游戏', '活动'])
      assert.equal(
        await page.getByRole('link', { name, exact: true }).count(),
        1,
      );
    assert.equal(
      await page.getByRole('link', { name: '存档', exact: true }).count(),
      0,
    );
    assert.equal(
      await page.getByRole('link', { name: '设置', exact: true }).count(),
      1,
    );
    assert.equal(
      await page
        .getByRole('link', { name: '设置', exact: true })
        .evaluate((element) =>
          element.classList.contains('sidebar-settings-button'),
        ),
      true,
    );
    assert.equal(
      await page
        .getByRole('button', { name: '打开游戏搜索', exact: true })
        .count(),
      0,
    );
    assert.equal(await page.locator('.app-topbar').count(), 0);
    assert.equal(
      await page.locator('.sidebar-settings-button svg path').getAttribute('d'),
      'M4 6h5M15 6h5M4 12h11M17 12h3M4 18h3M11 18h9M11 4v4M15 10v4M9 16v4',
    );
    await page.screenshot({
      path: resolve(evidence, 'home.png'),
      animations: 'disabled',
    });
  });
  await step('添加游戏入口显示单个游戏文件夹与批量目录两种方式', async () => {
    await page.evaluate(async () => {
      const { preview } = await import('/src/preview/store.ts');
      preview.show_empty = true;
    });
    const import_button = page.locator('.home-empty').getByRole('button', {
      name: '添加游戏',
      exact: true,
    });
    await import_button.click();
    const import_dialog = page.getByRole('dialog', { name: '添加本地游戏' });
    await import_dialog.waitFor();
    assert.equal(
      await import_dialog.getByRole('button', { name: '导入单个游戏' }).count(),
      1,
    );
    assert.equal(
      await import_dialog.getByRole('button', { name: '导入游戏目录' }).count(),
      1,
    );
    await import_dialog.getByRole('button', { name: '导入单个游戏' }).click();
    assert.match(await import_dialog.innerText(), /不能调用资源管理器/);
    await import_dialog.getByRole('button', { name: '关闭导入窗口' }).click();
    await import_dialog.waitFor({ state: 'hidden' });
    await page.evaluate(async () => {
      const { preview } = await import('/src/preview/store.ts');
      preview.show_empty = false;
    });
  });
  await step('首页区分游玩状态和运行事实，全部完成时显示收藏引导', async () => {
    await go('/');
    assert.equal(
      await page.getByRole('button', { name: /^继续游玩/ }).count(),
      1,
    );
    const saved = await page.evaluate(async () => {
      const { preview } = await import('/src/preview/store.ts');
      const statuses = preview.games.map((game) => [game.game_id, game.status]);
      preview.games.forEach((game) => {
        game.status = 'completed';
      });
      return statuses;
    });
    try {
      await page.locator('.home-empty').waitFor();
      assert.equal(await page.locator('.hero-stage').count(), 0);
      assert.match(await page.locator('.home-empty').innerText(), /重新发现/);
    } finally {
      await page.evaluate(async (statuses) => {
        const { preview } = await import('/src/preview/store.ts');
        const savedStatuses = new Map(statuses);
        preview.games.forEach((game) => {
          game.status = savedStatuses.get(game.game_id) ?? game.status;
        });
      }, saved);
    }
  });
  await step('搜索、收藏及统一网格展廊', async () => {
    await go('/games');
    const cards = page.locator('[data-preview-card][data-game-id]');
    await cards.first().waitFor();
    assert((await cards.count()) > 1);
    const first = cards.first();
    const id = await first.getAttribute('data-game-id');
    assert(id, '演示卡片必须有稳定 data-game-id。');
    const favorite = page.locator(
      `[data-preview-card-key="gallery:${id}"] [data-favorite="${id}"]`,
    );
    const original = await favorite.getAttribute('aria-pressed');
    await favorite.click();
    assert.notEqual(await favorite.getAttribute('aria-pressed'), original);
    assert(page.url().endsWith('#/games'));
    await favorite.click();
    await page
      .getByRole('searchbox', { name: '搜索作品' })
      .fill('不存在的演示故事');
    await page
      .getByRole('heading', { name: '这一页，暂时没有相遇。' })
      .waitFor();
    await page.getByRole('button', { name: '清除筛选', exact: true }).click();
    assert.equal(await page.locator('.gallery-stage.view-grid').count(), 1);
    assert.equal(await page.locator('.games-page select').count(), 0);
    assert.equal(
      await page.locator('.gallery-stage input[type=checkbox]').count(),
      0,
    );
    await page.screenshot({
      path: resolve(evidence, 'games.png'),
      animations: 'disabled',
    });
  });
  await step('真实共享展开与反向返回；按钮焦点恢复', async () => {
    const opener = page.locator('[data-preview-open^="gallery:"]').first();
    const sourceKey = await opener.getAttribute('data-preview-open');
    const id = await opener
      .locator('xpath=ancestor::*[@data-preview-card][1]')
      .getAttribute('data-game-id');
    await opener.click();
    await page.waitForFunction(
      () =>
        document
          .querySelector('.shared-transition-host')
          ?.getAttribute('data-overlay-count') === '1',
    );
    await page.waitForURL('**/#/games/' + id);
    await waitRouteContent('/games/' + id);
    const running = await page
      .locator('[data-shared-overlay="cover"]')
      .evaluate((element) =>
        element
          .getAnimations()
          .some((animation) => animation.playState === 'running'),
      );
    assert(running, '共享封面必须确实执行动画，不能总走立即降级。');
    const ratio = await page
      .locator('[data-shared-overlay="cover"]')
      .evaluate((element) => {
        const bounds = element.getBoundingClientRect();
        return bounds.width / bounds.height;
      });
    await page.evaluate(
      () =>
        new Promise((done) =>
          requestAnimationFrame(() => requestAnimationFrame(done)),
        ),
    );
    const nextRatio = await page
      .locator('[data-shared-overlay="cover"]')
      .evaluate((element) => {
        const bounds = element.getBoundingClientRect();
        return bounds.width / bounds.height;
      });
    assert(Math.abs(ratio - nextRatio) < 0.001, '飞行期间封面必须等比缩放。');
    await settled();
    assert(page.url().endsWith('#/games/' + id));
    await page.screenshot({
      path: resolve(evidence, 'detail.png'),
      animations: 'disabled',
    });
    await page.keyboard.press('Escape');
    await waitRoute('/games');
    assert(page.url().endsWith('#/games'));
    assert.equal(
      await page.evaluate(() =>
        document.activeElement?.getAttribute('data-preview-open'),
      ),
      sourceKey,
    );
  });
  await step('详情主标题与下方日文副标题；缺失副标题不占行', async () => {
    await go('/games');
    const original = await page.evaluate(async () => {
      const { preview } = await import('/src/preview/store.ts');
      const game = preview.games[0];
      const saved = {
        id: game.game_id,
        title: game.title,
        subtitle: game.subtitle,
      };
      Object.assign(game, {
        title: '中文验收作品名',
        subtitle: '日本語の作品名',
      });
      return saved;
    });
    try {
      const opener = page
        .locator(
          `[data-game-id="${original.id}"] [data-preview-open^="gallery:"]`,
        )
        .first();
      await opener.click();
      await waitRoute('/games/' + original.id);
      assert.equal(
        (await page.locator('[data-detail-heading]').innerText()).trim(),
        '中文验收作品名',
      );
      assert.equal(
        (await page.locator('.detail-subtitle').innerText()).trim(),
        '日本語の作品名',
      );
      assert(
        await page.locator('[data-detail-heading]').evaluate((heading) => {
          const subtitle = heading.nextElementSibling;
          return (
            subtitle?.classList.contains('detail-subtitle') &&
            subtitle.getBoundingClientRect().top >=
              heading.getBoundingClientRect().bottom
          );
        }),
        '日文副标题必须紧接主标题，并显示在其下方。',
      );
      await page.screenshot({
        path: resolve(evidence, 'detail-titles.png'),
        animations: 'disabled',
      });
      await page.evaluate(async (id) => {
        const { preview } = await import('/src/preview/store.ts');
        preview.games.find((game) => game.game_id === id).subtitle = '';
      }, original.id);
      assert.equal(await page.locator('.detail-subtitle').count(), 0);
      await page.keyboard.press('Escape');
      await waitRoute('/games');
    } finally {
      await page.evaluate(async (saved) => {
        const { preview } = await import('/src/preview/store.ts');
        Object.assign(
          preview.games.find((game) => game.game_id === saved.id),
          { title: saved.title, subtitle: saved.subtitle },
        );
      }, original);
    }
  });
  await step('详情五项游玩状态下拉框与侧栏同步', async () => {
    const id = await page
      .locator('[data-preview-card]')
      .first()
      .getAttribute('data-game-id');
    assert(id);
    await go('/games/' + id);
    const trigger = page.getByRole('combobox', {
      name: '游玩状态',
      exact: true,
    });
    assert.deepEqual(
      await page.locator('[data-detail-tab]').allTextContents(),
      ['概览', '游玩记录', '存档', '截图', '资料', '启动'],
    );
    const original = await trigger.innerText();
    await trigger.click();
    assert.equal(await page.getByRole('option').count(), 5);
    await page.getByRole('option', { name: '通关', exact: true }).click();
    assert.equal(await trigger.innerText(), '通关');
    assert(
      (
        await page
          .locator(`.group-game[href="/games/${id}"] .game-status`)
          .allTextContents()
      ).every((s) => s === '通关'),
    );
    await trigger.click();
    await page.getByRole('option', { name: original, exact: true }).click();
    assert.equal(await trigger.innerText(), original);
    await go('/games');
  });
  await step('打开中 Esc、连点和 resize 中断均无残留', async () => {
    const first = page.locator('[data-preview-open^="gallery:"]').first();
    await first.click();
    await page.locator('[data-detail-stage]').waitFor();
    await page.waitForFunction(
      () => !document.querySelector('.page-enter-active, .page-leave-active'),
      undefined,
      { timeout: 5000 },
    );
    await page.keyboard.press('Escape');
    await waitRoute('/games');
    assert(page.url().endsWith('#/games'));
    const ids = await page
      .locator('[data-preview-card]')
      .evaluateAll((elements) =>
        elements
          .slice(0, 2)
          .map((element) => element.getAttribute('data-game-id')),
      );
    await page
      .locator('[data-preview-open^="gallery:"]')
      .evaluateAll((elements) => {
        elements[0]?.click();
        elements[1]?.click();
      });
    await page.waitForURL('**/#/games/' + ids[1]);
    await settled();
    await page.keyboard.press('Escape');
    await settled();
    await page.locator('[data-preview-open^="gallery:"]').first().click();
    await page.locator('[data-detail-stage]').waitFor();
    await page.waitForFunction(
      () => !document.querySelector('.page-enter-active, .page-leave-active'),
      undefined,
      { timeout: 5000 },
    );
    await page.setViewportSize({ width: 1100, height: 820 });
    await settled();
    await page.keyboard.press('Escape');
    await settled();
    await page.setViewportSize({ width: 1440, height: 1000 });
  });
  await step('详情返回保持搜索；源卡隐藏时合理降级', async () => {
    await page.locator('[data-preview-open^="gallery:"]').first().click();
    await settled();
    await storePatch({ query: '源卡已被筛选隐藏' });
    await page.keyboard.press('Escape');
    await settled();
    assert.equal(
      await page.getByRole('searchbox', { name: '搜索作品' }).inputValue(),
      '源卡已被筛选隐藏',
    );
    assert.equal(await page.locator('[data-preview-card]').count(), 0);
    await page.getByRole('button', { name: '清除筛选', exact: true }).click();
  });
  await step('直达详情、无效作品 ID 和无效路由安全回退', async () => {
    const id = await page
      .locator('[data-preview-card]')
      .first()
      .getAttribute('data-game-id');
    await go('/games/' + id);
    await page.keyboard.press('Escape');
    await waitRoute('/games');
    assert(page.url().endsWith('#/games'));
    for (const invalidId of ['demo-does-not-exist', 'demo"bad]']) {
      await go('/games/' + encodeURIComponent(invalidId));
      assert.match(
        await page.locator('body').innerText(),
        /未找到|不存在|没有找到/,
      );
      await page.keyboard.press('Escape');
      await settled();
      assert(page.url().endsWith('#/games'));
    }
    await page.goto(base + '/#/route-does-not-exist');
    await page.waitForURL('**/#/');
    await waitRoute('/');
    assert(page.url().endsWith('#/'));
  });
  await step('默认完整动效契约与已移除控制入口', async () => {
    await go('/settings');
    assert.equal(await page.locator('[data-motion-option]').count(), 0);
    await page.waitForFunction(
      () => document.documentElement.dataset.motion === 'full',
    );
    await page.emulateMedia({ reducedMotion: 'reduce' });
    // Current product explicitly selects full; reduced CSS is covered in detail fixtures.
    await page.waitForFunction(
      () => document.documentElement.dataset.motion === 'full',
    );
    await page.emulateMedia({ reducedMotion: 'no-preference' });
  });
  await step('分组删除确认弹窗的键盘约束、取消与保留作品', async () => {
    await go('/games');
    const count = await page.evaluate(async () => {
      const { preview, savePreviewGroup } =
        await import('/src/preview/store.ts');
      savePreviewGroup('确认回归分组', [preview.games[0].game_id]);
      return preview.games.length;
    });
    const openDelete = async () => {
      await page
        .locator('.gallery-tabs')
        .getByRole('button', { name: '确认回归分组', exact: true })
        .click({ button: 'right' });
      await page.getByRole('menuitem', { name: '删除', exact: true }).click();
    };
    await openDelete();
    const dialog = page.getByRole('dialog');
    await dialog.waitFor();
    assert(
      await dialog.evaluate((node) => node.contains(document.activeElement)),
    );
    await page.keyboard.press('Shift+Tab');
    assert.equal(
      await page.evaluate(() => document.activeElement.textContent.trim()),
      '确认删除分组',
    );
    await page.keyboard.press('Tab');
    assert.equal(
      await page.evaluate(() => document.activeElement.textContent.trim()),
      '取消',
    );
    await page.keyboard.press('Escape');
    await dialog.waitFor({ state: 'hidden' });
    assert.equal(
      await page
        .locator('.gallery-tabs')
        .getByRole('button', { name: '确认回归分组', exact: true })
        .count(),
      1,
    );
    await openDelete();
    await dialog
      .getByRole('button', { name: '确认删除分组', exact: true })
      .click();
    await dialog.waitFor({ state: 'hidden' });
    assert.equal(
      await page.evaluate(
        async () =>
          (await import('/src/preview/store.ts')).preview.games.length,
      ),
      count,
    );
  });
  await step('空库、缺图和浅色主题仍可浏览', async () => {
    await storePatch({ show_empty: true });
    await page.getByRole('link', { name: '游戏', exact: true }).click();
    await waitRoute('/games');
    await page
      .getByRole('heading', { name: '展廊正等待你的第一段故事。' })
      .waitFor();
    await page.getByRole('link', { name: '设置', exact: true }).click();
    await waitRoute('/settings');
    await storePatch({
      show_empty: false,
      missing_cover: true,
      theme: 'light',
    });
    await page.getByRole('link', { name: '游戏', exact: true }).click();
    await waitRoute('/games');
    await page.locator('[data-preview-open^="gallery:"]').first().click();
    await settled();
    assert.match(await page.locator('body').innerText(), /缺图|封面|画面/);
    await page.keyboard.press('Escape');
    await settled();
    await storePatch({ missing_cover: false });
    await page.screenshot({
      path: resolve(evidence, 'light.png'),
      animations: 'disabled',
    });
    await page.getByRole('link', { name: '设置', exact: true }).click();
    await storePatch({ theme: 'dark' });
  });
  await step('活动和存档页面可从直接路由访问', async () => {
    await go('/activity');
    assert.match(await page.locator('body').innerText(), /留在故事里的时间/);
    await go('/saves');
    assert.match(
      await page.locator('body').innerText(),
      /每一次选择，都值得留存/,
    );
  });
  await step('统一分组标签、批量收藏与分组隐藏', async () => {
    await go('/games');
    const saved = await page.evaluate(async () => {
      const { saveSmartCollection, query, preview } =
        await import('/src/stores/library.ts');
      await saveSmartCollection(
        '保存的完整规则',
        query({ search: '潮汐', filters: { release_year: 2024 } }),
      );
      return JSON.parse(JSON.stringify(preview.groups[0]));
    });
    assert.equal(saved.query.search, '潮汐');
    assert.equal(saved.query.filters.release_year, 2024);
    assert(saved.smart);
    const tabs = page.locator('.gallery-tabs');
    await tabs
      .getByRole('button', { name: '保存的完整规则', exact: true })
      .click();
    await page.waitForFunction(
      () =>
        document.querySelector('.gallery-stage')?.dataset.totalGames === '1' &&
        !document.querySelector('.gallery-leave-active'),
    );
    assert.equal(await page.locator('.gallery-stage .cover-card').count(), 1);
    await tabs.getByRole('button', { name: /^全部作品/ }).click();
    await page.getByRole('button', { name: '批量选择', exact: true }).click();
    await page
      .getByRole('checkbox', { name: '选择潮汐寄来的信', exact: true })
      .check();
    await page
      .getByRole('checkbox', { name: '选择月光落在森林里', exact: true })
      .check();
    await page
      .locator('.batch-toolbar')
      .getByRole('button', { name: '收藏', exact: true })
      .click();
    await page.waitForFunction(async () => {
      const { preview } = await import('/src/preview/store.ts');
      return preview.games.slice(0, 2).every((g) => g.favorite);
    });
    await page.getByRole('button', { name: '取消选择', exact: true }).click();
    await tabs
      .getByRole('button', { name: '保存的完整规则', exact: true })
      .click({ button: 'right' });
    await page.getByRole('menuitem', { name: '管理', exact: true }).click();
    const editor = page.getByRole('dialog', { name: '管理分组' });
    assert.equal(await editor.getByRole('combobox').count(), 0);
    assert.equal(await editor.getByRole('textbox').count(), 1);
    const members = editor.getByRole('checkbox');
    for (const member of await members.all()) assert(await member.isDisabled());
    await editor.getByRole('button', { name: '保存分组', exact: true }).click();
    await editor.waitFor({ state: 'hidden' });
    await page.evaluate(async () => {
      const { preview } = await import('/src/preview/store.ts');
      const group = preview.groups.find((g) => g.name === '保存的完整规则');
      group.hidden = true;
      group.icon = 'heart';
    });
    assert.equal(
      await page
        .locator('.game-group[aria-label="保存的完整规则分组"]')
        .count(),
      0,
    );
    assert.equal(
      await tabs
        .getByRole('button', { name: '保存的完整规则', exact: true })
        .count(),
      0,
    );
    await page.screenshot({
      path: resolve(evidence, 'advanced-games.png'),
      animations: 'disabled',
    });
  });
  await step('千条游戏可见区域渲染、滚动、筛选及详情返回', async () => {
    await go('/games');
    await page.evaluate(async () => {
      const { preview } = await import('/src/preview/store.ts');
      const { createPreviewGames } = await import('/src/preview/data.ts');
      const source = createPreviewGames();
      preview.query = '';
      preview.status_filter = 'all';
      preview.show_empty = false;
      preview.groups = [];
      Object.assign(preview.gallery_state, {
        source: '',
        tag: '',
        year: '',
        developer: '',
        playtime: '',
        backup: '',
        multiple: '',
        recent: '',
        system: 'all',
        collection_id: '',
      });
      preview.games = Array.from({ length: 1000 }, (_, i) => ({
        ...source[i % source.length],
        game_id: `fixture-${i}`,
        title: `作品 ${i}`,
        added_order: i,
      }));
    });
    const gallery = page.locator('.virtual-gallery');
    await gallery.waitFor();
    await page.waitForFunction(
      () =>
        Number(
          document
            .querySelector('.virtual-gallery')
            ?.getAttribute('data-rendered-games'),
        ) < 60,
    );
    for (const fraction of [0, 0.25, 0.5, 0.9, 1]) {
      await page.evaluate(
        (ratio) =>
          window.scrollTo(
            0,
            (document.documentElement.scrollHeight - innerHeight) * ratio,
          ),
        fraction,
      );
      await page.waitForTimeout(100);
      const rendered = await gallery.locator('[data-preview-card]').count();
      measurements.push({
        kind: '1000-games-visible-cards',
        fraction,
        rendered,
      });
      assert(rendered > 0 && rendered < 60);
    }
    const selected = page
      .locator('.virtual-gallery [data-preview-open]')
      .last();
    await selected.scrollIntoViewIfNeeded();
    await page.waitForTimeout(100);
    const key = await selected.getAttribute('data-preview-open');
    const previousScroll = await page.evaluate(() => window.scrollY);
    await selected.click();
    await settled();
    await page.keyboard.press('Escape');
    await waitRoute('/games');
    await page.waitForTimeout(150);
    const returnedScroll = await page.evaluate(() => window.scrollY);
    measurements.push({
      kind: 'virtual-detail-return',
      previousScroll,
      returnedScroll,
    });
    assert(
      Math.abs(returnedScroll - previousScroll) < 20,
      `返回滚动偏差 ${returnedScroll - previousScroll}px`,
    );
    assert.equal(await page.locator(`[data-preview-open="${key}"]`).count(), 1);
    await page.setViewportSize({ width: 900, height: 800 });
    await page.waitForTimeout(150);
    assert(Number(await gallery.getAttribute('data-rendered-games')) < 60);
    await page.evaluate(() => window.scrollTo(0, 0));
    await page.getByRole('searchbox', { name: '搜索作品' }).fill('作品 999');
    await page.waitForFunction(
      () =>
        document.querySelectorAll('.games-page [data-preview-card]').length ===
        1,
    );
    assert.equal(
      await page
        .locator('.games-page [data-game-id]')
        .getAttribute('data-game-id'),
      'fixture-999',
    );
    await page.setViewportSize({ width: 1440, height: 1000 });
    await page.screenshot({
      path: resolve(evidence, '1000-games-filtered.png'),
      animations: 'disabled',
    });
  });
  await step('无浏览器未处理异常与外部资源请求', async () => {
    assert.deepEqual(errors, []);
    assert.deepEqual(external, []);
  });
} finally {
  const result = {
    platform: process.platform,
    browser: browser.version(),
    url: base,
    passed,
    measurements,
    failures,
    errors,
    external,
    evidence,
    windows_tested: false,
  };
  writeFileSync(
    resolve(evidence, 'result.json'),
    JSON.stringify(result, null, 2) + '\n',
  );
  console.log('结果与截图：' + evidence);
  await browser.close();
}
if (failures.length) process.exitCode = 1;
