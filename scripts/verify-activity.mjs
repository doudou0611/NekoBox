// Real Activity.vue in the app and with isolated IPC fixtures; never a production database or Windows GUI claim.
import assert from 'node:assert/strict';
import { existsSync, mkdirSync, writeFileSync, renameSync } from 'node:fs';
import { resolve } from 'node:path';
import { chromium } from '../.tools/browser-check/node_modules/playwright/index.mjs';
const base = process.env.PREVIEW_URL || 'http://127.0.0.1:1420';
assert(['127.0.0.1', 'localhost'].includes(new URL(base).hostname));
const edge =
  process.platform === 'darwin'
    ? '/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge'
    : process.platform === 'win32'
      ? resolve(
          process.env['PROGRAMFILES(X86)'] || 'C:/Program Files (x86)',
          'Microsoft/Edge/Application/msedge.exe',
        )
      : null;
const browser = await chromium.launch({
  headless: true,
  executablePath:
    process.env.BROWSER_EXECUTABLE ||
    (edge && existsSync(edge) ? edge : undefined),
});
const evidence = resolve('.tools/activity-evidence', String(Date.now()));
mkdirSync(evidence, { recursive: true });
const errors = [],
  results = [],
  layouts = [];
let page;
async function step(name, run) {
  await run();
  results.push(name);
  console.log('通过：' + name);
}
function track(p) {
  p.setDefaultTimeout(10000);
  p.on('pageerror', (e) => errors.push(e.message));
}
async function settled() {
  await page.waitForFunction(
    () =>
      !!document.querySelector('[data-metric="total"]') &&
      document.querySelector('.activity-content')?.getAttribute('aria-busy') ===
        'false',
  );
}
async function dimensions() {
  return page.evaluate(() => ({
    width: innerWidth,
    height: innerHeight,
    content: document.querySelector('.activity-dashboard').clientWidth,
    overflow: document.documentElement.scrollWidth > innerWidth,
    columns: getComputedStyle(
      document.querySelector('.activity-two-columns'),
    ).gridTemplateColumns.split(' ').length,
    metrics: getComputedStyle(
      document.querySelector('.activity-metrics'),
    ).gridTemplateColumns.split(' ').length,
    rangeTabs: document.querySelectorAll('.activity-ranges button').length,
    yearScroll:
      document.querySelector('.calendar-scroll').scrollWidth >
      document.querySelector('.calendar-scroll').clientWidth,
  }));
}
const goldens = [
  [
    'S1',
    'A',
    '2026-10-03T22:13:00Z',
    '2026-10-03T23:46:00Z',
    5580,
    'process_exit',
  ],
  [
    'S2',
    'B',
    '2026-10-02T20:41:00Z',
    '2026-10-02T22:07:00Z',
    5160,
    'process_exit',
  ],
  [
    'S3',
    'C',
    '2026-10-01T23:30:00Z',
    '2026-10-02T00:30:00Z',
    3600,
    'process_exit',
  ],
  [
    'S4',
    'A',
    '2026-09-28T21:00:00Z',
    '2026-09-28T22:00:00Z',
    7200,
    'process_exit',
  ],
  [
    'S5',
    'C',
    '2026-09-01T18:00:00Z',
    '2026-09-01T20:00:00Z',
    7200,
    'process_exit',
  ],
  [
    'S6',
    'A',
    '2026-10-03T10:00:00Z',
    '2026-10-03T10:01:00Z',
    0,
    'process_exit',
  ],
  [
    'S7',
    'B',
    '2026-10-03T09:00:00Z',
    '2026-10-03T09:01:00Z',
    60,
    'launch_failed',
  ],
  [
    'S8',
    'D',
    '2025-12-31T10:00:00Z',
    '2025-12-31T22:00:00Z',
    43200,
    'process_exit',
  ],
].map(([id, game_id, started_at, ended_at, duration_seconds, end_reason]) => ({
  id,
  game_id,
  game_title: {
    A: '樱之诗',
    B: '魔法使之夜',
    C: 'Summer Pockets',
    D: '旧年作品',
  }[game_id],
  install_id: 'isolated-install',
  started_at,
  ended_at,
  duration_seconds,
  end_reason,
  process_name: 'fixture.exe',
  corrected: id === 'S4',
  correction_reason: id === 'S4' ? '补记' : null,
}));
try {
  const context = await browser.newContext({
    viewport: { width: 1280, height: 800 },
    recordVideo: { dir: evidence, size: { width: 1280, height: 800 } },
  });
  page = await context.newPage();
  track(page);
  await page.goto(`${base}/#/activity`);
  await settled();
  const appearanceSource = await (
    await page.request.get(`${base}/src/composables/usePreviewAppearance.ts`)
  ).text();
  const storeUrl = appearanceSource.match(
    /from ["'](\/src\/preview\/store\.ts[^"']*)["']/,
  )?.[1];
  assert(storeUrl);
  async function appearance(
    theme,
    palette = 'wisteria',
    motion = 'full',
    glass = true,
  ) {
    await page.evaluate(
      async ({ storeUrl, theme, palette, motion, glass }) => {
        const { preview } = await import(storeUrl);
        preview.theme = theme;
        preview.palette = palette;
        preview.motion_preference = motion;
        const { global_glass_enabled } =
          await import('/src/composables/useDesktopMaterial.ts');
        global_glass_enabled.value = glass;
      },
      { storeUrl, theme, palette, motion, glass },
    );
    await page.waitForFunction(
      ({ theme, palette, motion, glass }) =>
        document.documentElement.dataset.theme === theme &&
        document.documentElement.dataset.palette === palette &&
        document.documentElement.dataset.motion ===
          (motion === 'system'
            ? matchMedia('(prefers-reduced-motion: reduce)').matches
              ? 'reduced'
              : 'full'
            : motion) &&
        document.documentElement.dataset.globalGlass === (glass ? 'on' : 'off'),
      { theme, palette, motion, glass },
    );
  }
  await step('完整活动路由、五项范围与明确演示标识', async () => {
    assert.match(
      await page.locator('.activity-context').innerText(),
      /隔离演示数据/,
    );
    assert.equal(await page.locator('.activity-ranges button').count(), 5);
    assert.equal(await page.locator('[data-metric]').count(), 4);
    for (const id of ['week', 'days30', 'month', 'year', 'all']) {
      await page.locator(`[data-range="${id}"]`).click();
      await settled();
      assert.equal(
        await page.locator(`[data-range="${id}"]`).getAttribute('aria-pressed'),
        'true',
      );
      assert.equal(await page.locator('.calendar-cell').count(), 365);
      assert.equal(await page.locator('.calendar-months span').count(), 12);
      assert.equal(await page.locator('[data-date="2026-01-01"]').count(), 1);
      assert.equal(await page.locator('[data-date="2026-12-31"]').count(), 1);
      assert.equal(await page.locator('.calendar-short-story').count(), 0);
    }
  });
  await step('主题与窗口布局、侧栏缩起无横向溢出', async () => {
    await appearance('light');
    await page.screenshot({
      path: resolve(evidence, 'light-1280.png'),
      fullPage: true,
    });
    await page.screenshot({ path: resolve(evidence, 'light-viewport.png') });
    for (const size of [
      { width: 800, height: 600 },
      { width: 1024, height: 768 },
      { width: 1280, height: 800 },
      { width: 1440, height: 900 },
      { width: 1920, height: 1080 },
    ]) {
      await page.setViewportSize(size);
      const d = await dimensions();
      layouts.push(d);
      assert.equal(d.overflow, false, JSON.stringify(d));
      assert.equal(d.rangeTabs, 5);
      if (size.width === 1920) assert.equal(d.yearScroll, false);
      if (size.width === 800) assert.equal(d.yearScroll, true);
      const cell = await page.locator('.calendar-cell').first().boundingBox();
      assert.ok(Math.abs(cell.width - cell.height) < 1, JSON.stringify(cell));
      assert.equal(d.columns, d.content < 760 ? 1 : 2);
      if (size.width === 800 || size.width === 1920)
        await page.screenshot({
          path: resolve(evidence, `layout-${size.width}.png`),
          fullPage: true,
        });
    }
    await page.setViewportSize({ width: 1280, height: 800 });
    const collapse = page.locator(
      '[aria-controls="navigation-rail main-content"]',
    );
    await collapse.click();
    assert.equal((await dimensions()).overflow, false);
    await collapse.click();
    const widthHandle = page.getByRole('separator', {
      name: '侧栏宽度',
      exact: true,
    });
    await widthHandle.press('End');
    await page.waitForFunction(() => {
      const e = document.querySelector('.sidebar-width-handle');
      return (
        e.getAttribute('aria-valuenow') === e.getAttribute('aria-valuemax')
      );
    });
    await page.waitForTimeout(500);
    const widened = await dimensions();
    layouts.push({ ...widened, sidebar: 'maximum width' });
    assert.equal(widened.overflow, false);
    await widthHandle.dblclick();
    const palettes = [
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
    ];
    for (const theme of ['light', 'dark'])
      for (const palette of palettes) {
        await appearance(theme, palette);
        assert.equal((await dimensions()).overflow, false);
        const colors = await page
          .locator('.calendar-legend i')
          .evaluateAll((es) =>
            es.map((e) => getComputedStyle(e).backgroundColor),
          );
        assert.equal(new Set(colors).size, 5);
        if (['black', 'white'].includes(palette))
          await page.screenshot({
            path: resolve(evidence, `${palette}-${theme}.png`),
          });
      }
    await appearance('dark');
    await page.screenshot({
      path: resolve(evidence, 'dark-1280.png'),
      fullPage: true,
    });
    await appearance('light', 'wisteria', 'full', false);
    await page.screenshot({ path: resolve(evidence, 'glass-off.png') });
    await appearance('light');
  });
  await step('范围与日历键盘、日期局部筛选、图表提示与详情返回', async () => {
    await page.locator('[data-range="all"]').focus();
    await page.keyboard.press('Home');
    await settled();
    assert.equal(
      await page.locator('[data-range="week"]').getAttribute('aria-pressed'),
      'true',
    );
    await page.locator('[data-range="week"]').press('ArrowRight');
    await settled();
    const before = await page.locator('[data-metric="total"]').textContent();
    const day = page.locator('.calendar-cell:not(:disabled)').first();
    await day.scrollIntoViewIfNeeded();
    await day.focus();
    await page.keyboard.press('End');
    assert.equal(await page.locator('.calendar-cell[tabindex="0"]').count(), 1);
    await page.keyboard.press('Enter');
    await settled();
    assert.equal(
      await page.locator('[data-metric="total"]').textContent(),
      before,
    );
    await page.getByRole('button', { name: /清除筛选/ }).click();
    await settled();
    const point = page.locator('.trend-point[tabindex="0"]');
    await point.focus();
    await page.keyboard.press('End');
    await page.waitForFunction(
      () =>
        document.querySelectorAll(
          '.activity-tooltip:not(.activity-tip-leave-active)',
        ).length === 1,
    );
    await page.keyboard.press('Escape');
    await page.waitForFunction(
      () => document.querySelectorAll('.activity-tooltip').length === 0,
    );
    await page.locator('[data-range="all"]').click();
    await settled();
    await page.locator('.activity-year-picker select').selectOption('2025');
    await settled();
    const total = await page.locator('[data-metric="total"]').textContent();
    await page.locator('.activity-ranking-row').first().click();
    await page.waitForURL(/games/);
    await page.goBack();
    await settled();
    assert.equal(
      await page.locator('[data-range="all"]').getAttribute('aria-pressed'),
      'true',
    );
    assert.equal(
      await page.locator('.activity-year-picker select').inputValue(),
      '2025',
    );
    assert.equal(
      await page.locator('[data-metric="total"]').textContent(),
      total,
    );
  });
  await step('light 与 reduced 档取消复杂动画、小时展开与分页', async () => {
    await appearance('light', 'wisteria', 'light');
    await appearance('light', 'wisteria', 'reduced');
    assert.equal(
      await page
        .locator('.trend-line')
        .evaluate((e) => getComputedStyle(e).animationName),
      'none',
    );
    assert.equal(
      await page
        .locator('.activity-ranges')
        .evaluate((e) => getComputedStyle(e, '::before').transitionDuration),
      '0s',
    );
    await page.getByRole('button', { name: /查看 24 小时/ }).click();
    assert.equal(await page.locator('.activity-hour-list li').count(), 24);
    await page.locator('.activity-hour-list li').first().focus();
    await page.waitForFunction(() =>
      [...document.querySelectorAll('.activity-tooltip')].some((e) =>
        e.textContent.includes('开始的会话'),
      ),
    );
    await page.keyboard.press('Escape');
    await page.getByRole('button', { name: /收起时段/ }).click();
    await page.getByRole('button', { name: '下一页', exact: true }).click();
    await settled();
    assert.match(
      await page.locator('.journal-pagination').innerText(),
      /2\s*\//,
    );
    await page.screenshot({ path: resolve(evidence, 'reduced.png') });
  });
  await step(
    'system 跟随减弱动态设置，模拟高对比度仍保留日期文字',
    async () => {
      await page.emulateMedia({
        reducedMotion: 'reduce',
        forcedColors: 'active',
      });
      await appearance('light', 'wisteria', 'system');
      assert.equal(
        await page
          .locator('.trend-line')
          .evaluate((e) => getComputedStyle(e).animationName),
        'none',
      );
      assert.equal((await dimensions()).overflow, false);
      const day = page.locator('.calendar-cell:not(:disabled)').first();
      assert.match(await day.getAttribute('aria-label'), /2025|2026/);
      assert.notEqual(
        await day.evaluate((e) => getComputedStyle(e).borderStyle),
        'none',
      );
      await day.focus();
      await page.screenshot({
        path: resolve(evidence, 'system-forced-colors.png'),
      });
      await page.emulateMedia({
        reducedMotion: 'no-preference',
        forcedColors: 'none',
      });
      await appearance('light', 'wisteria', 'system');
      await page.waitForFunction(
        () => document.documentElement.dataset.motion === 'full',
      );
    },
  );
  const previewVideo = page.video();
  await context.close();
  renameSync(
    await previewVideo.path(),
    resolve(evidence, 'preview-interactions.webm'),
  );

  const ipcContext = await browser.newContext({
    viewport: { width: 1280, height: 800 },
    recordVideo: { dir: evidence, size: { width: 1280, height: 800 } },
  });
  page = await ipcContext.newPage();
  track(page);
  const source = await (
    await page.request.get(`${base}/src/composables/useActivity.ts`)
  ).text();
  const vue = source.match(
    /from ["'](\/node_modules\/\.vite\/deps\/vue\.js[^"']*)["']/,
  )?.[1];
  const router = source.match(
    /from ["'](\/node_modules\/\.vite\/deps\/vue-router\.js[^"']*)["']/,
  )?.[1];
  assert(vue && router);
  await page.addInitScript(
    ({ goldens }) => {
      window.isTauri = true;
      window.testSessions = goldens;
      window.testRequests = [];
      window.testFailure = false;
      window.testConflict = false;
      window.testEmpty = false;
      window.testDelays = {};
      window.testUnlistens = 0;
      window.__TAURI_EVENT_PLUGIN_INTERNALS__ = {
        unregisterListener: () => {},
      };
      const date = (s) => s.slice(0, 10),
        add = (d, n) =>
          new Date(Date.parse(d + 'T00:00:00Z') + n * 86400000)
            .toISOString()
            .slice(0, 10);
      window.buildSnapshot = (q) => {
        const rows = window.testEmpty
          ? []
          : window.testSessions.filter((s) => s.end_reason !== 'launch_failed');
        const start = {
          week: '2026-09-27',
          days30: '2026-09-04',
          month: '2026-10-01',
          year: '2026-01-01',
          all: rows.length
            ? [...rows]
                .sort((a, b) => a.started_at.localeCompare(b.started_at))[0]
                .started_at.slice(0, 10)
            : '2026-10-03',
        }[q.range];
        const records = rows.filter(
          (s) =>
            date(s.started_at) >= start && date(s.started_at) < '2026-10-04',
        );
        const days = new Map(),
          games = new Map(),
          hours = Array.from({ length: 24 }, (_, hour) => ({
            hour,
            duration_seconds: 0,
          }));
        for (const s of records) {
          const d = date(s.started_at),
            v = days.get(d) || {
              date: d,
              duration_seconds: 0,
              session_count: 0,
            };
          v.duration_seconds += s.duration_seconds;
          v.session_count++;
          days.set(d, v);
          if (s.duration_seconds > 0) {
            const g = games.get(s.game_id) || {
              game_id: s.game_id,
              title: s.game_title,
              cover_url: '/preview/shore.svg',
              duration_seconds: 0,
            };
            g.duration_seconds += s.duration_seconds;
            games.set(s.game_id, g);
          }
          hours[new Date(s.started_at).getUTCHours()].duration_seconds +=
            s.duration_seconds;
        }
        const year = q.calendar_year || 2026,
          daily = [];
        const calendarDays = new Map();
        for (const s of rows) {
          const d = date(s.started_at);
          if (d < `${year}-01-01` || d >= `${year + 1}-01-01`) continue;
          const v = calendarDays.get(d) || {
            date: d,
            duration_seconds: 0,
            session_count: 0,
          };
          v.duration_seconds += s.duration_seconds;
          v.session_count++;
          calendarDays.set(d, v);
        }
        for (let d = `${year}-01-01`; d < `${year + 1}-01-01`; d = add(d, 1))
          daily.push(
            calendarDays.get(d) || {
              date: d,
              duration_seconds: 0,
              session_count: 0,
            },
          );
        const filtered = records
          .filter(
            (s) => !q.session_date || date(s.started_at) === q.session_date,
          )
          .sort(
            (a, b) =>
              b.started_at.localeCompare(a.started_at) ||
              b.id.localeCompare(a.id),
          );
        return {
          as_of: '2026-10-03T23:59:59Z',
          range: q.range,
          range_start: start,
          range_end: '2026-10-04',
          summary: {
            total_seconds: records.reduce((n, s) => n + s.duration_seconds, 0),
            played_count: games.size,
            completed_count: [...games.keys()].filter((k) =>
              ['A', 'C', 'D'].includes(k),
            ).length,
            active_days: [...days.values()].filter(
              (v) => v.duration_seconds > 0,
            ).length,
            session_count: records.length,
            active_session_count: records.filter((s) => !s.ended_at).length,
          },
          daily,
          trend_granularity: 'day',
          trend: [...days.values()]
            .sort((a, b) => a.date.localeCompare(b.date))
            .map((d) => ({ ...d, end_date: d.date })),
          games: [...games.values()].sort(
            (a, b) => b.duration_seconds - a.duration_seconds,
          ),
          hours,
          calendar_year: year,
          calendar_years: [2026, 2025],
          session_date: q.session_date,
          sessions: {
            page: q.page,
            page_size: q.page_size,
            total: filtered.length,
            items: filtered.slice(
              (q.page - 1) * q.page_size,
              q.page * q.page_size,
            ),
          },
        };
      };
      window.__TAURI_INTERNALS__ = {
        transformCallback: () => 1,
        unregisterCallback: () => {},
        convertFileSrc: (path) => path,
        invoke: async (command, args = {}) => {
          if (command === 'plugin:event|listen') return 1;
          if (command === 'plugin:event|unlisten') {
            window.testUnlistens++;
            return true;
          }
          const r = args.request;
          window.testRequests.push({ command, payload: r?.payload });
          let data;
          if (command === 'get_activity_snapshot') {
            if (window.testFailure) throw Error('isolated failure');
            data = window.buildSnapshot(r.payload);
            if (window.testDelays[r.payload.range])
              await new Promise((resolve) =>
                setTimeout(resolve, window.testDelays[r.payload.range]),
              );
          } else if (command === 'correct_play_session') {
            const s = window.testSessions.find(
              (s) => s.id === r.payload.session_id,
            );
            if (
              window.testConflict ||
              s.duration_seconds !== r.payload.expected_duration_seconds
            )
              return {
                success: false,
                error_code: 'CONFLICT',
                message: '时长已被修改，请刷新记录后重试。',
                request_id: r.request_id,
                data: null,
              };
            s.duration_seconds = r.payload.duration_seconds;
            s.corrected = true;
            s.correction_reason = r.payload.reason;
            data = s;
          } else if (command === 'list_games')
            data = { page: 1, page_size: 100, total: 0, items: [] };
          else if (
            ['list_collections', 'get_recommendations'].includes(command)
          )
            data = [];
          else if (command === 'backend_status')
            data = { last_scan_task_id: null };
          else if (command === 'get_home_summary') data = {};
          else throw Error('unexpected isolated command ' + command);
          return {
            success: true,
            error_code: null,
            message: '隔离测试响应',
            request_id: r.request_id,
            data,
          };
        },
      };
    },
    { goldens },
  );
  await page.route('**/__activity_component_test', (route) =>
    route.fulfill({
      contentType: 'text/html',
      body: `<html><head><meta charset="utf-8"></head><body><div id="test-root"></div><script type="module">
  import {createApp,h} from '${vue}';import {createRouter,createMemoryHistory,RouterView} from '${router}';import Activity from '/src/views/Activity.vue';import {useMotionPolicy} from '/src/composables/useMotionPolicy.ts';import {usePreviewAppearance} from '/src/composables/usePreviewAppearance.ts';import '/src/style.css';import '/src/appearance.css';
  const router=createRouter({history:createMemoryHistory(),routes:[{path:'/games',name:'games',component:{template:'<p>现有游戏入口占位</p>'}},{path:'/activity',name:'activity',component:Activity},{path:'/games/:game_id',name:'game-detail',component:{template:'<p>现有详情路由占位</p>'}}]});window.testRouter=router;await router.push('/activity');await router.isReady();createApp({setup(){useMotionPolicy();usePreviewAppearance();return ()=>h(RouterView)}}).use(router).mount('#test-root');
  </script></body></html>`,
    }),
  );
  await page.goto(`${base}/__activity_component_test`);
  await settled();
  await step(
    '真实组件读取固定 IPC 数据、五范围准确且失败记录排除',
    async () => {
      for (const [id, total, played, completed, days] of [
        ['week', '5h 59m', 3, 2, 4],
        ['days30', '5h 59m', 3, 2, 4],
        ['month', '3h 59m', 3, 2, 3],
        ['year', '7h 59m', 3, 2, 5],
        ['all', '19h 59m', 4, 3, 6],
      ]) {
        await page.locator(`[data-range="${id}"]`).click();
        await settled();
        assert.equal(
          (
            await page.locator('[data-metric="total"]').textContent()
          ).replaceAll('\n', ''),
          total,
        );
        assert.match(
          await page.locator('[data-metric="played"]').innerText(),
          new RegExp('^' + played),
        );
        assert.match(
          await page.locator('[data-metric="completed"]').innerText(),
          new RegExp('^' + completed),
        );
        assert.match(
          await page.locator('[data-metric="days"]').innerText(),
          new RegExp('^' + days),
        );
        assert.equal(await page.locator('[data-session="S7"]').count(), 0);
        assert.equal(await page.locator('.calendar-cell').count(), 365);
        const outside = page.locator('[data-date="2026-09-01"]');
        assert.match(await outside.getAttribute('class'), /heat-2/);
        assert.equal(await outside.isDisabled(), !['year', 'all'].includes(id));
        assert.equal(
          await outside.evaluate((e) => getComputedStyle(e).opacity),
          '1',
        );
        assert.equal(
          await page.locator('[data-date="2026-12-31"]').isDisabled(),
          true,
        );
      }
      await page.locator('.activity-year-picker select').selectOption('2025');
      await settled();
      assert.equal(
        await page.locator('[data-metric="total"]').textContent(),
        '19h 59m',
      );
      await page.locator('[data-date="2025-12-31"]').click();
      await settled();
      assert.equal(await page.locator('.journal-session').count(), 1);
      assert.equal(await page.locator('[data-session="S8"]').count(), 1);
      await page.locator('[data-range="week"]').click();
      await settled();
      await page.locator('[data-date="2026-10-03"]').click();
      await settled();
      assert.equal(await page.locator('.journal-session').count(), 2);
      assert.equal(await page.locator('[data-session="S6"]').count(), 1);
      await page.getByRole('button', { name: /清除筛选/ }).click();
      await settled();
      await page.screenshot({
        path: resolve(evidence, 'fixed-ipc-data.png'),
        fullPage: true,
      });
    },
  );
  await step('迟到请求不覆盖新选择，错误保留快照且可重试', async () => {
    await page.evaluate(() => (window.testDelays = { year: 250, month: 10 }));
    await page.locator('[data-range="year"]').click();
    await page.locator('[data-range="month"]').click();
    await settled();
    await page.waitForTimeout(300);
    assert.equal(
      await page.locator('[data-metric="total"]').textContent(),
      '3h 59m',
    );
    await page.evaluate(() => {
      window.testDelays = {};
      window.testFailure = true;
    });
    await page
      .getByRole('button', { name: '刷新活动记录', exact: true })
      .click();
    await page.locator('.activity-error-banner').waitFor();
    assert.equal(
      await page.locator('[data-metric="total"]').textContent(),
      '3h 59m',
    );
    await page.screenshot({ path: resolve(evidence, 'read-error.png') });
    await page.evaluate(() => (window.testFailure = false));
    await page.getByRole('button', { name: '重试', exact: true }).click();
    await settled();
    assert.equal(await page.locator('.activity-error-banner').count(), 0);
  });
  await step('修正确认、失败输入保留、原值校验及成功后统一刷新', async () => {
    await page.locator('[data-range="week"]').click();
    await settled();
    await page.locator('[data-session="S1"] .journal-edit').click();
    await page.locator('dialog[open]').waitFor();
    assert.equal(
      await page
        .getByRole('button', { name: '保存修正', exact: true })
        .isDisabled(),
      true,
    );
    await page.getByLabel('小时', { exact: true }).fill('0');
    await page.getByLabel('分钟', { exact: true }).fill('30');
    await page.getByLabel('秒', { exact: true }).fill('0');
    await page.getByLabel('修正原因', { exact: true }).fill('隔离回归修正');
    await page.getByRole('checkbox').check();
    await page.evaluate(() => (window.testConflict = true));
    await page.getByRole('button', { name: '保存修正', exact: true }).click();
    await page.locator('.activity-correction [role="alert"]').waitFor();
    assert.equal(
      await page.getByLabel('修正原因', { exact: true }).inputValue(),
      '隔离回归修正',
    );
    assert.equal(
      await page.getByLabel('分钟', { exact: true }).inputValue(),
      '30',
    );
    await page.evaluate(() => (window.testConflict = false));
    await page.getByRole('button', { name: '保存修正', exact: true }).click();
    await page.locator('dialog[open]').waitFor({ state: 'hidden' });
    await settled();
    assert.equal(
      await page.locator('[data-metric="total"]').textContent(),
      '4h 56m',
    );
    const writes = await page.evaluate(() =>
      window.testRequests.filter((r) => r.command === 'correct_play_session'),
    );
    assert.equal(writes.at(-1).payload.expected_duration_seconds, 5580);
    assert.equal(writes.at(-1).payload.duration_seconds, 1800);
    assert.equal(writes.at(-1).payload.confirmed, true);
  });
  await step('运行中仅显示保存的 300 秒，且禁止修正未结束会话', async () => {
    await page.evaluate(() =>
      window.testSessions.push({
        ...window.testSessions[0],
        id: 'active-checkpoint',
        started_at: '2026-10-03T23:50:00Z',
        ended_at: null,
        end_reason: null,
        duration_seconds: 300,
      }),
    );
    await page
      .getByRole('button', { name: '刷新活动记录', exact: true })
      .click();
    await settled();
    const active = page
      .locator('.journal-session')
      .filter({ hasText: '计时中' });
    assert.equal(await active.count(), 1);
    assert.match(await active.innerText(), /5m/);
    assert.equal(await active.getByRole('button', { name: /修正/ }).count(), 0);
    assert.equal(
      await page.locator('[data-metric="total"]').textContent(),
      '5h 1m',
    );
    await page.screenshot({ path: resolve(evidence, 'active-checkpoint.png') });
    await page.evaluate(
      () =>
        (window.testSessions = window.testSessions.filter(
          (s) => s.id !== 'active-checkpoint',
        )),
    );
  });
  await step('全零状态、分页完整查询和离页监听清理', async () => {
    await page.evaluate(() => (window.testEmpty = true));
    await page
      .getByRole('button', { name: '刷新活动记录', exact: true })
      .click();
    await settled();
    assert.equal(
      await page.locator('[data-metric="total"]').textContent(),
      '0h',
    );
    assert.equal(await page.locator('.activity-empty-notice').count(), 1);
    await page.screenshot({ path: resolve(evidence, 'empty.png') });
    await page.evaluate(() => {
      window.testEmpty = false;
      for (let n = 0; n < 28; n++)
        window.testSessions.push({
          ...window.testSessions[0],
          id: 'extra-' + n,
          started_at: '2026-10-03T12:00:00Z',
          ended_at: '2026-10-03T12:01:00Z',
          duration_seconds: 60,
        });
    });
    await page
      .getByRole('button', { name: '刷新活动记录', exact: true })
      .click();
    await settled();
    await page.getByRole('button', { name: '下一页', exact: true }).click();
    await settled();
    assert.match(
      await page.locator('.journal-pagination').innerText(),
      /2\s*\//,
    );
    assert.equal(await page.locator('.journal-session').count(), 13);
    await page.evaluate(() => window.testRouter.push('/games/A'));
    await page.waitForFunction(() => window.testUnlistens === 2);
  });
  const ipcVideo = page.video();
  await ipcContext.close();
  renameSync(await ipcVideo.path(), resolve(evidence, 'ipc-interactions.webm'));
  assert.deepEqual(errors, []);
  writeFileSync(
    resolve(evidence, 'result.json'),
    JSON.stringify(
      {
        passed: results,
        layouts,
        errors,
        environment:
          'macOS Edge headless; preview and isolated IPC; Windows GUI not verified',
      },
      null,
      2,
    ),
  );
  console.log('证据：' + evidence);
} catch (error) {
  writeFileSync(
    resolve(evidence, 'failure.json'),
    JSON.stringify(
      { passed: results, layouts, errors, error: error.message },
      null,
      2,
    ),
  );
  if (page && !page.isClosed())
    await page
      .screenshot({ path: resolve(evidence, 'failure.png') })
      .catch(() => {});
  throw error;
} finally {
  await browser.close();
}
