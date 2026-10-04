import assert from 'node:assert/strict';
import { mkdirSync, writeFileSync } from 'node:fs';
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
const evidence = resolve('.tools/home-v1-evidence', String(Date.now()));
mkdirSync(evidence, { recursive: true });
const passed = [],
  failures = [],
  errors = [];
async function scenario(name, run, timezoneId = 'Asia/Shanghai') {
  const context = await browser.newContext({
    viewport: { width: 1280, height: 800 },
    timezoneId,
  });
  await context.route('**/*', (r) => {
    const u = new URL(r.request().url());
    return /^https?:$/.test(u.protocol) &&
      !['localhost', '127.0.0.1'].includes(u.hostname)
      ? r.abort()
      : r.continue();
  });
  const page = await context.newPage();
  page.setDefaultTimeout(5000);
  page.on('pageerror', (e) => errors.push(`${name}: ${e.message}`));
  try {
    await page.goto(base);
    await page.locator('.home-v1').waitFor();
    await page.waitForTimeout(1000);
    await run(page);
    passed.push(name);
    console.log('通过：' + name);
  } catch (e) {
    failures.push({ name, error: e.stack });
    console.error('失败：' + name + '\n' + e.stack);
  } finally {
    await context.close();
  }
}
async function patch(page, fn) {
  await page.evaluate(fn);
}
async function settled(page) {
  await page.waitForFunction(
    () =>
      document
        .querySelector('.shared-transition-host')
        ?.getAttribute('data-overlay-count') === '0',
  );
}
await scenario('主视觉/紧凑模块/首屏按钮与侧栏工具', async (page) => {
  await page.locator('.hero-stage').waitFor();
  const button = await page
    .locator('.hero-stage .primary-button')
    .boundingBox();
  assert(button.y + button.height <= 800);
  assert.equal(await page.locator('.app-topbar').count(), 0);
  assert.equal(await page.locator('.home-counts button').count(), 4);
  assert.equal(await page.locator('.home-week-day').count(), 7);
  assert((await page.locator('.home-memory').innerText()).includes('当日'));
  assert.equal(await page.locator('.home-calendar-row').count(), 2);
  await page.screenshot({
    path: resolve(evidence, 'home-light.png'),
    fullPage: true,
  });
  await page.evaluate(() => scrollTo(0, 1200));
  const dock = await page.locator('.sidebar-dock').boundingBox();
  assert(dock.y > 650 && dock.y + dock.height <= 800);
  const expected = await page.evaluate(async () => {
    const { preview } = await import('/src/preview/store.ts');
    return [...preview.games]
      .filter((g) => g.last_played_order > 0)
      .sort((a, b) => b.last_played_order - a.last_played_order)
      .slice(0, 4)
      .map((g) => g.game_id);
  });
  assert.deepEqual(
    await page
      .locator('#home-recent-heading')
      .locator('..')
      .locator('..')
      .locator('.cover-card')
      .evaluateAll((cards) => cards.map((card) => card.dataset.gameId)),
    expected,
  );
  for (const material of ['browser', 'macos-vibrancy', 'windows-acrylic']) {
    await page.evaluate(async (material) => {
      const { global_glass_enabled } =
        await import('/src/composables/useDesktopMaterial.ts');
      global_glass_enabled.value = true;
      document.documentElement.dataset.windowMaterial = material;
    }, material);
    await page.waitForFunction(
      () => document.documentElement.dataset.globalGlass === 'on',
    );
    if (material !== 'browser') {
      const background = await page
        .locator('.exhibition-main')
        .evaluate((el) => getComputedStyle(el).backgroundColor);
      assert(
        background.startsWith('rgba('),
        `Native main is opaque: ${background}`,
      );
    }
    await page.evaluate(async () => {
      const { global_glass_enabled } =
        await import('/src/composables/useDesktopMaterial.ts');
      global_glass_enabled.value = false;
    });
    await page.waitForFunction(
      () =>
        getComputedStyle(document.querySelector('.navigation-rail'))
          .backdropFilter === 'none',
    );
  }
  await page.evaluate(async () => {
    const { global_glass_enabled } =
      await import('/src/composables/useDesktopMaterial.ts');
    global_glass_enabled.value = true;
    document.documentElement.dataset.windowMaterial = 'browser';
  });
});
await scenario(
  '本地日期跨夏令时仍为自然日，七日连续而非固定24小时',
  async (page) => {
    const days = await page.evaluate(async () => {
      const { homeWindow } = await import('/src/services/homeDashboard.ts');
      return [new Date(2026, 2, 8), new Date(2026, 10, 1)].map((d) => {
        const w = homeWindow(d);
        return {
          hours:
            (Date.parse(w.today.end_at) - Date.parse(w.today.start_at)) /
            3600000,
          count: w.week_days.length,
          contiguous: w.week_days.every(
            (day, i) => i === 0 || day.start_at === w.week_days[i - 1].end_at,
          ),
        };
      });
    });
    assert.deepEqual(days, [
      { hours: 23, count: 7, contiguous: true },
      { hours: 25, count: 7, contiguous: true },
    ]);
  },
  'America/New_York',
);
await scenario('搜索跳转只聚焦游戏页已有输入', async (page) => {
  await page.getByRole('button', { name: '搜索游戏', exact: true }).click();
  await page.locator('[data-library-search]').waitFor();
  await page.waitForFunction(() =>
    document.activeElement?.hasAttribute('data-library-search'),
  );
  assert.equal(await page.locator('[type=search]').count(), 1);
});
await scenario('状态计数与待游玩组合筛选清理旧条件', async (page) => {
  await patch(page, async () => {
    const { preview } = await import('/src/preview/store.ts');
    preview.query = '无结果';
    preview.gallery_state.collection_id = '不存在';
  });
  const expected = await page.evaluate(async () => {
    const { preview } = await import('/src/preview/store.ts');
    return preview.games.filter((g) => g.favorite && g.status === 'not_started')
      .length;
  });
  await page.getByRole('button', { name: '查看全部待游玩作品' }).click();
  await page.locator('.games-page').waitFor();
  assert.equal(
    await page.locator('.games-page [data-game-id][data-preview-card]').count(),
    expected,
  );
  assert.equal(await page.locator('[data-library-search]').inputValue(), '');
});
await scenario(
  '随机只推荐，换一个/关闭焦点/演示启动没有真实进程',
  async (page) => {
    const random = page.getByRole('button', { name: '随机一个', exact: true });
    await random.click();
    const dialog = page.getByRole('dialog', {
      name: '下一段旅程，会是哪一部？',
    });
    await dialog.waitFor();
    assert.match(await dialog.innerText(), /抽取不会启动/);
    await page.screenshot({ path: resolve(evidence, 'random.png') });
    await dialog.getByRole('button', { name: '开始游玩 · 演示' }).click();
    await page.getByText('原型演示：没有运行游戏。', { exact: true }).waitFor();
    assert.equal(await page.locator('dialog[open]').count(), 0);
    await random.click();
    await page.keyboard.press('Escape');
    await page.waitForFunction(() =>
      document.activeElement?.textContent?.includes('随机一个'),
    );
    assert.equal(await page.locator('dialog[open]').count(), 0);
  },
);
await scenario('推荐排除及恢复，不重新增加设置栏目', async (page) => {
  const card = page.locator('.home-recommendation').first();
  await card.waitFor();
  await card.locator('summary').click();
  await card.getByRole('button', { name: '不感兴趣', exact: true }).click();
  await page.getByRole('button', { name: /已排除作品/ }).click();
  const dialog = page.getByRole('dialog', { name: '已排除作品', exact: true });
  await dialog.waitFor();
  assert.equal(
    await dialog.getByRole('button', { name: '恢复推荐', exact: true }).count(),
    1,
  );
  await dialog.getByRole('button', { name: '恢复推荐', exact: true }).click();
  await dialog.getByText('还没有已排除的作品。', { exact: true }).waitFor();
});
await scenario(
  '推荐换批完整遍历并可重新浏览，主视觉离开视口暂停光晕',
  async (page) => {
    const first = await page
      .locator('.home-recommendation [data-game-id]')
      .evaluateAll((es) => es.map((e) => e.getAttribute('data-game-id')));
    await page.getByRole('button', { name: '换一批', exact: true }).click();
    await page.getByRole('button', { name: '重新浏览', exact: true }).waitFor();
    await page.waitForFunction(
      () => document.querySelectorAll('.home-recommendation').length === 0,
    );
    await page.getByRole('button', { name: '重新浏览', exact: true }).click();
    await page.locator('.home-recommendation').first().waitFor();
    assert.deepEqual(
      await page
        .locator('.home-recommendation [data-game-id]')
        .evaluateAll((es) => es.map((e) => e.getAttribute('data-game-id'))),
      first,
    );
    await page.evaluate(() => scrollTo(0, 1200));
    await page.waitForFunction(
      () =>
        document
          .querySelector('.home-v1')
          ?.getAttribute('data-hero-visible') === 'false',
    );
    assert.equal(
      await page
        .locator('.hero-halo')
        .evaluate((el) => getComputedStyle(el).animationPlayState),
      'paused',
    );
  },
);
await scenario('缺图、真正空库、无记录、全部完成均无虚构启动', async (page) => {
  await patch(page, async () => {
    const { preview } = await import('/src/preview/store.ts');
    preview.missing_cover = true;
  });
  assert.equal(await page.locator('.hero-scenery').count(), 0);
  await page.locator('.hero-cover .cover-placeholder').waitFor();
  await patch(page, async () => {
    const { preview } = await import('/src/preview/store.ts');
    preview.show_empty = true;
  });
  await page.locator('.home-empty').waitFor();
  assert.equal(await page.locator('.home-counts').count(), 0);
  await page.screenshot({ path: resolve(evidence, 'empty.png') });
  await patch(page, async () => {
    const { preview } = await import('/src/preview/store.ts');
    preview.show_empty = false;
    preview.games.forEach((g) => {
      g.status = 'completed';
    });
  });
  await page.locator('.home-empty').waitFor();
  assert.equal(await page.locator('.hero-stage').count(), 0);
  await patch(page, async () => {
    const { preview } = await import('/src/preview/store.ts');
    preview.games.forEach((g) => {
      g.status = 'not_started';
      g.last_played_order = 0;
    });
  });
  await page.getByRole('heading', { name: '最近游玩', exact: true }).waitFor();
  assert.equal(
    await page.locator('[data-preview-open^="home-recent:"]').count(),
    0,
  );
  await page
    .getByText('还没有启动过游戏，开启一段故事后会显示在这里。')
    .waitFor();
  assert.match(await page.locator('.hero-kicker').innerText(), /开启新故事/);
});
await scenario('共享转场、返回位置与焦点，没有透明残留', async (page) => {
  const card = page.locator('[data-preview-open^="home-recent:"]').first();
  await card.scrollIntoViewIfNeeded();
  const y = await page.evaluate(() => scrollY);
  await card.click();
  await page.locator('[data-detail-stage]').waitFor();
  await settled(page);
  await page.getByRole('button', { name: /返回原展廊位置/ }).click();
  await page.locator('.home-v1').waitFor();
  await settled(page);
  assert(Math.abs((await page.evaluate(() => scrollY)) - y) < 5);
  assert.equal(await page.locator('[data-shared-overlay]').count(), 0);
  await page.waitForFunction(() =>
    document.activeElement?.hasAttribute('data-preview-open'),
  );
});
await scenario('十套配色/深浅与默认完整动效', async (page) => {
  for (const theme of ['light', 'dark'])
    for (const palette of [
      'wisteria',
      'sea',
      'forest',
      'rose',
      'amber',
      'graphite',
      'black',
      'white',
      'jade',
      'coral',
    ]) {
      await page.evaluate(
        async ({ theme, palette }) => {
          const { preview } = await import('/src/preview/store.ts');
          preview.theme = theme;
          preview.palette = palette;
        },
        { theme, palette },
      );
      await page.waitForFunction(
        ({ theme, palette }) =>
          document.documentElement.dataset.theme === theme &&
          document.documentElement.dataset.palette === palette,
        { theme, palette },
      );
      assert(await page.locator('.hero-stage .primary-button').isVisible());
      assert.equal(
        await page
          .locator('.home-v1')
          .evaluate((el) => el.scrollWidth <= el.clientWidth),
        true,
      );
    }
  // UpdateSettingV1 removed the animation preference; the app now defaults to full.
  assert.equal(
    await page.evaluate(() => document.documentElement.dataset.motion),
    'full',
  );
  await page.screenshot({
    path: resolve(evidence, 'home-dark.png'),
    fullPage: true,
  });
});
await scenario(
  '自适应与侧栏宽度：800/1280/1440/1920/500，无溢出',
  async (page) => {
    for (const [width, height] of [
      [800, 600],
      [1280, 800],
      [1440, 900],
      [1920, 1080],
      [500, 800],
    ]) {
      await page.setViewportSize({ width, height });
      await page.waitForTimeout(450);
      assert.equal(
        await page.evaluate(
          () => document.documentElement.scrollWidth <= innerWidth,
        ),
        true,
      );
      assert.equal(
        await page
          .locator('.home-v1')
          .evaluate((el) => el.scrollWidth <= el.clientWidth),
        true,
      );
      await page.screenshot({
        path: resolve(evidence, `home-${width}.png`),
        fullPage: true,
      });
    }
    await page.setViewportSize({ width: 1000, height: 800 });
    await page.evaluate(async () => {
      const { sidebar_layout } =
        await import('/src/composables/useSidebarLayout.ts');
      sidebar_layout.width = 460;
    });
    await page.waitForTimeout(450);
    assert.equal(
      await page
        .locator('.home-v1')
        .evaluate((el) => el.scrollWidth <= el.clientWidth),
      true,
    );
  },
);
await scenario('长中日英标题、生日缺口与静态减少模式', async (page) => {
  await page.evaluate(async () => {
    const { preview } = await import('/src/preview/store.ts');
    preview.motion_preference = 'reduced';
    preview.games[0].title =
      '这是一个很长的作品名称 日本語の物語 Long English Story Name '.repeat(4);
  });
  await page.setViewportSize({ width: 800, height: 600 });
  await page.waitForTimeout(100);
  assert.equal(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= innerWidth,
    ),
    true,
  );
  assert.equal(await page.getByText(/角色生日/).count(), 0);
  assert.equal(await page.getByText(/你通关了/).count(), 0);
});
writeFileSync(
  resolve(evidence, 'result.json'),
  JSON.stringify(
    { platform: process.platform, base, passed, failures, errors },
    null,
    2,
  ),
);
await browser.close();
console.log(evidence);
assert.equal(failures.length, 0);
assert.equal(errors.length, 0);
