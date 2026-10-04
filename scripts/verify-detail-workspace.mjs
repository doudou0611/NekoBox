// Real Vue/App interactions, with IPC fixtures exclusively in browser memory.
import assert from 'node:assert/strict';
import { mkdirSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { chromium } from '../.tools/browser-check/node_modules/playwright/index.mjs';
const base = process.env.PREVIEW_URL || 'http://127.0.0.1:1421';
assert(['127.0.0.1', 'localhost'].includes(new URL(base).hostname));
const evidence = resolve(
  '.tools/detail-workspace-evidence',
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
    viewport: { width: 1280, height: 900 },
  });
  const errors = [];
  page.on('pageerror', (e) => errors.push(e.message));
  const source = await (await page.request.get(`${base}/src/App.vue`)).text();
  const vue = source.match(
    /from "(\/node_modules\/\.vite\/deps\/vue\.js[^"]*)"/,
  )?.[1];
  const library = source.match(/from "(\/src\/stores\/library\.ts[^"]*)"/)?.[1];
  assert(vue && library);
  await page.addInitScript(() => {
    window.isTauri = true;
    const installation = {
      id: 'workspace-install',
      game_id: 'workspace-game',
      absolute_path: 'C:\\Fixture\\星海 游戏',
      executable_path: 'C:\\Fixture\\星海 游戏\\launcher.exe',
      source: 'local',
      steam_app_id: null,
      path_valid: true,
      arguments: [],
      working_directory: null,
      environment: {},
      candidates: [
        {
          path: 'C:\\Fixture\\星海 游戏\\launcher.exe',
          product_name: '故事启动器',
          file_description: null,
          company_name: null,
          fingerprint: 'fixture',
          title_evidence: [],
        },
      ],
      main_process_name: null,
      track_after_launcher_exit: true,
      idle_timeout_minutes: null,
      use_locale_emulator: null,
      use_magpie: null,
    };
    const summary = {
      id: 'workspace-game',
      title: '星海的回声',
      title_zh: null,
      title_ja: null,
      title_en: null,
      cover_url: null,
      developer: 'Fixture Studio',
      publisher: null,
      release_date: '2025-01-01',
      source_rating: 8.6,
      source_tags: [],
      status: 'not_started',
      favorite: false,
      hidden: false,
      user_rating: null,
      added_at: '2026-10-01T00:00:00Z',
      total_playtime_seconds: 0,
      last_played_at: null,
      metadata_status: 'synced',
      installations: [installation],
      tags: [],
    };
    window.fixture = {
      summary,
      installation,
      description: '刮削自动填入的中文简介。',
      metadata_locked: false,
      metadata: [
        {
          field: 'description',
          value: '刮削自动填入的中文简介。',
          provider: 'hikarinagi',
          remote_id: '7788',
          fetched_at: '2026-10-04T00:00:00Z',
          cached: false,
          manually_edited: false,
        },
      ],
      commands: [],
      failSave: false,
      failProcess: false,
      active: false,
    };
    window.__TAURI_INTERNALS__ = {
      metadata: {
        currentWindow: { label: 'main' },
        currentWebview: { label: 'main' },
      },
      transformCallback: () => 1,
      unregisterCallback: () => {},
      convertFileSrc: (path) => path,
      invoke: async (command, args = {}) => {
        if (!args.request) return 1;
        const f = window.fixture,
          q = args.request.payload;
        f.commands.push({ command, payload: structuredClone(q) });
        let data = {};
        const detail = () => ({
          ...f.summary,
          description: f.description,
          metadata: f.metadata,
          metadata_locked: f.metadata_locked,
        });
        const failure = (message) => ({
          success: false,
          error_code: 'CONFLICT',
          message,
          request_id: args.request.request_id,
          data: null,
        });
        if (command === 'get_game') data = detail();
        else if (command === 'list_games')
          data = { items: [f.summary], total: 1, page: 1, page_size: 100 };
        else if (command === 'backend_status')
          data = {
            data_directory: 'isolated-memory-only',
            schema_version: 1,
            portable: true,
            last_scan_task_id: null,
          };
        else if (command === 'get_app_settings') data = f.settings;
        else if (command === 'save_app_settings') data = f.settings = q;
        else if (
          [
            'list_collections',
            'get_recommendations',
            'list_external_sources',
          ].includes(command)
        )
          data = [];
        else if (command === 'get_hikarinagi_rates') data = { wall: null };
        else if (['bangumi_account', 'hikarinagi_account'].includes(command))
          data = { status: 'signed_out', profile: null, message: '' };
        else if (command === 'get_metadata_refresh') data = { status: 'idle' };
        else if (command === 'get_application_backup_status')
          data = { running: false };
        else if (command === 'get_installation') data = f.installation;
        else if (command === 'update_game_metadata') {
          if (f.failSave)
            return failure('隔离测试：后台资料已变化，请重新载入后再保存。');
          for (const [key, value] of Object.entries(q.changes)) {
            if (key === 'description') f.description = value;
            else if (key !== 'cover_path') f.summary[key] = value;
          }
          data = detail();
        } else if (command === 'set_metadata_lock') {
          f.metadata_locked = q.locked;
          data = detail();
        } else if (command === 'configure_installation') {
          Object.assign(f.installation, q);
          data = f.installation;
        } else if (command === 'list_game_processes')
          data = {
            active_session: f.active,
            items: [
              {
                pid: 4312,
                created_at_ticks: '134035612345678901',
                name: 'story.exe',
                path: 'C:\\Fixture\\星海 游戏\\story.exe',
              },
            ],
          };
        else if (command === 'select_game_process') {
          if (f.failProcess) return failure('游戏进程已经退出，请刷新列表。');
          data = {
            pid: 4312,
            created_at_ticks: '134035612345678901',
            name: 'story.exe',
            path: 'C:\\Fixture\\星海 游戏\\story.exe',
          };
        } else if (command === 'scan_screenshots') data = 3;
        return {
          success: true,
          error_code: null,
          message: '隔离内存响应',
          request_id: args.request.request_id,
          data: structuredClone(data),
        };
      },
    };
  });
  await page.route('**/__workspace_test', (route) =>
    route.fulfill({
      contentType: 'text/html',
      body: `<html><head><meta charset="utf-8"></head><body><div id="test-root"></div><script type="module">
  import {createApp} from '${vue}'; import App from '/src/App.vue'; import {createAppRouter} from '/src/router/index.ts'; import {createMemoryHistory} from '/node_modules/.vite/deps/vue-router.js'; import {DEFAULT_SETTINGS} from '/src/types/settings.ts'; import {loadDetail,local,preview} from '${library}';
  import '/src/style.css'; import '/src/appearance.css'; import '/src/settings.css'; import '/src/sidebar-tools.css'; import '/src/detail-workspace.css';
  window.fixture.settings={...DEFAULT_SETTINGS,ui_refresh_seconds:15};window.detailTest={loadDetail,local,preview};
  const router=createAppRouter(createMemoryHistory());await router.push('/games/workspace-game?panel=information');await router.isReady();createApp(App).use(router).mount('#test-root');
  </script></body></html>`,
    }),
  );
  await page.goto(`${base}/__workspace_test`);
  await page.locator('.information-workspace').waitFor();
  const tabs = await page.getByRole('tab').allTextContents();
  assert.equal(tabs[tabs.indexOf('资料') + 1], '启动');
  assert.deepEqual(
    await page
      .locator('[data-info-section]')
      .evaluateAll((els) => els.map((e) => e.dataset.infoSection)),
    ['name', 'cover', 'binding', 'metadata', 'sources', 'lock', 'screenshots'],
  );
  const title = page.getByRole('textbox', { name: '作品名称' });
  const developer = page.getByRole('textbox', { name: '开发商' });
  const description = page.getByRole('textbox', { name: '简介编辑' });
  assert.equal(await developer.inputValue(), 'Fixture Studio');
  assert.equal(await description.inputValue(), '刮削自动填入的中文简介。');
  assert(
    (await page.locator('.workspace-source').innerText()).includes('7788'),
  );
  await description.fill('我的未保存简介');
  await page.evaluate(async () => {
    window.fixture.summary.developer = '新刮削开发商';
    window.fixture.description = '新的自动简介';
    await window.detailTest.loadDetail('workspace-game');
  });
  assert.equal(await developer.inputValue(), '新刮削开发商');
  assert.equal(await description.inputValue(), '我的未保存简介');
  await title.fill('我设定的作品名称');
  await page.evaluate(() => (window.fixture.failSave = true));
  await page.getByRole('button', { name: '保存资料', exact: true }).click();
  await page.getByRole('alert').filter({ hasText: '后台资料已变化' }).waitFor();
  assert.equal(await description.inputValue(), '我的未保存简介');
  await page.evaluate(() => (window.fixture.failSave = false));
  await page.getByRole('button', { name: '保存资料', exact: true }).click();
  await page.waitForFunction(
    () => window.detailTest.preview.games[0].title === '我设定的作品名称',
  );
  const edit = await page.evaluate(
    () =>
      window.fixture.commands
        .filter((c) => c.command === 'update_game_metadata')
        .at(-1).payload,
  );
  assert.deepEqual(Object.keys(edit.changes).sort(), ['description', 'title']);
  assert.equal(edit.expected.description, '刮削自动填入的中文简介。');
  assert.equal(
    await page.locator('[data-detail-heading]').innerText(),
    '我设定的作品名称',
  );
  await page.getByRole('checkbox', { name: '锁定元数据' }).check();
  await page.waitForFunction(() => window.fixture.metadata_locked);
  assert(await title.isDisabled());
  assert(
    await page.getByRole('button', { name: '重新刮削（手动）' }).isDisabled(),
  );
  assert(
    await page
      .getByRole('button', { name: '保存资料', exact: true })
      .isDisabled(),
  );
  assert(await page.getByRole('button', { name: /扫描截图：/ }).isEnabled());
  await page.getByRole('checkbox', { name: '锁定元数据' }).uncheck();
  await page.waitForFunction(() => !window.fixture.metadata_locked);
  assert(await title.isEnabled());
  await page.getByRole('button', { name: /扫描截图：/ }).click();
  assert.equal(
    await page.evaluate(
      () =>
        window.fixture.commands.filter((c) => c.command === 'scan_screenshots')
          .length,
    ),
    1,
  );
  await page.getByRole('button', { name: '重新刮削（手动）' }).click();
  const manualDialog = page.getByRole('dialog', { name: '选择资料来源与作品' });
  await manualDialog.waitFor();
  const manualBounds = await manualDialog.boundingBox();
  assert(Math.abs(manualBounds.x + manualBounds.width / 2 - 640) < 2);
  await page.keyboard.press('Escape');
  await manualDialog.waitFor({ state: 'hidden' });
  await page.locator('[data-info-section="metadata"]').scrollIntoViewIfNeeded();
  await page.screenshot({ path: resolve(evidence, 'information-light.png') });
  await page.getByRole('tab', { name: '启动', exact: true }).click();
  await page.locator('.launch-workspace').waitFor();
  await page.waitForFunction(
    () => !document.querySelector('.launch-workspace fieldset').disabled,
  );
  assert.deepEqual(
    await page
      .locator('[data-launch-section]')
      .evaluateAll((els) => els.map((e) => e.dataset.launchSection)),
    [
      'installation',
      'entry',
      'arguments',
      'work',
      'environment',
      'process',
      'tools',
    ],
  );
  assert.equal(
    await page.locator('[data-info-section="screenshots"]').count(),
    0,
  );
  await page.getByRole('button', { name: '添加参数', exact: true }).click();
  await page
    .getByRole('textbox', { name: '启动参数 1', exact: true })
    .fill('C:\\中文 目录\\save');
  await page.getByRole('button', { name: '添加环境变量', exact: true }).click();
  await page.getByRole('textbox', { name: '环境变量名称 1' }).fill('GAME_PATH');
  await page
    .getByRole('textbox', { name: '环境变量值 1' })
    .fill(' value with spaces ');
  await page
    .locator('.workspace-tool-grid .preference-choice')
    .first()
    .getByRole('radio', { name: '关闭' })
    .check();
  await page.getByRole('button', { name: '保存启动配置', exact: true }).click();
  await page.waitForFunction(() =>
    window.fixture.commands.some((c) => c.command === 'configure_installation'),
  );
  const launch = await page.evaluate(
    () =>
      window.fixture.commands.find(
        (c) => c.command === 'configure_installation',
      ).payload,
  );
  assert.deepEqual(launch.arguments, ['C:\\中文 目录\\save']);
  assert.deepEqual(launch.environment, { GAME_PATH: ' value with spaces ' });
  assert.equal(launch.use_locale_emulator, false);
  assert.equal(launch.use_magpie, null);
  assert.equal(launch.main_process_name, null);
  await page.getByRole('button', { name: '选择运行中进程' }).click();
  await page.getByRole('dialog', { name: '选择 Windows 主进程' }).waitFor();
  await page.getByRole('button', { name: /story.exe.*PID 4312/ }).click();
  assert.equal(
    await page.getByRole('textbox', { name: /主游戏进程名/ }).inputValue(),
    'story.exe',
  );
  assert.equal(
    await page.evaluate(
      () =>
        window.fixture.commands.filter(
          (c) => c.command === 'select_game_process',
        ).length,
    ),
    0,
  );
  await page.evaluate(() => {
    window.fixture.active = true;
    window.fixture.failProcess = true;
  });
  await page.getByRole('button', { name: '选择运行中进程' }).click();
  await page.getByRole('button', { name: /story.exe.*PID 4312/ }).click();
  await page
    .getByRole('alert')
    .filter({ hasText: '游戏进程已经退出' })
    .waitFor();
  assert(
    await page.getByRole('dialog', { name: '选择 Windows 主进程' }).isVisible(),
  );
  await page.evaluate(() => (window.fixture.failProcess = false));
  await page.getByRole('button', { name: /story.exe.*PID 4312/ }).click();
  await page
    .getByRole('status')
    .filter({ hasText: '当前会话已切换' })
    .waitFor();
  const process = await page.evaluate(
    () =>
      window.fixture.commands
        .filter((c) => c.command === 'select_game_process')
        .at(-1).payload,
  );
  assert.equal(process.created_at_ticks, '134035612345678901');
  await page.getByRole('button', { name: '保存启动配置', exact: true }).click();
  await page.locator('[data-launch-section="tools"]').scrollIntoViewIfNeeded();
  await page.screenshot({ path: resolve(evidence, 'launch-light.png') });
  const sizes = [];
  for (const width of [1920, 1280, 800, 500]) {
    await page.setViewportSize({ width, height: 900 });
    await page.getByRole('button', { name: '选择运行中进程' }).click();
    const bounds = await page
      .getByRole('dialog', { name: '选择 Windows 主进程' })
      .boundingBox();
    assert(bounds.x >= 0 && bounds.x + bounds.width <= width + 1);
    assert(
      Math.abs(bounds.x + bounds.width / 2 - width / 2) < 2,
      'Process dialog must be centered',
    );
    await page.keyboard.press('Escape');
    await page.getByRole('tab', { name: '资料', exact: true }).click();
    await page.locator('.information-workspace').waitFor();
    const result = await page.evaluate(() => ({
      overflow: document.documentElement.scrollWidth - innerWidth,
      inputs: [
        ...document.querySelectorAll(
          '.information-workspace .workspace-field input',
        ),
      ].map((e) => ({ w: e.getBoundingClientRect().width })),
    }));
    assert(
      result.overflow <= 1,
      `Width ${width}: horizontal overflow ${result.overflow}`,
    );
    assert(result.inputs.every((i) => i.w > 40));
    sizes.push({ width, ...result });
    if (width === 500)
      await page.screenshot({ path: resolve(evidence, 'information-500.png') });
    await page.getByRole('tab', { name: '启动', exact: true }).click();
    await page.locator('.launch-workspace').waitFor();
    await page.waitForFunction(
      () => !document.querySelector('.launch-workspace fieldset').disabled,
    );
  }
  await page.setViewportSize({ width: 1280, height: 900 });
  await page.evaluate(() => {
    window.detailTest.preview.theme = 'dark';
    document.documentElement.dataset.motion = 'reduced';
  });
  await page.getByRole('button', { name: '选择运行中进程' }).click();
  await page.screenshot({ path: resolve(evidence, 'process-dark.png') });
  assert.equal(
    await page
      .locator('.workspace-process-picker')
      .evaluate((e) => getComputedStyle(e).animationName),
    'none',
  );
  await page.keyboard.press('Escape');
  await page.getByRole('tab', { name: '资料', exact: true }).click();
  await page.locator('.information-workspace').waitFor();
  assert.equal(
    await page
      .locator('.workspace-card')
      .first()
      .evaluate((e) => getComputedStyle(e).animationName),
    'none',
  );
  await page.locator('[data-info-section="sources"]').scrollIntoViewIfNeeded();
  await page.screenshot({ path: resolve(evidence, 'sources-dark.png') });
  const themes = [];
  for (const theme of ['light', 'dark']) {
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
        ({ theme, palette }) => {
          window.detailTest.preview.theme = theme;
          window.detailTest.preview.palette = palette;
        },
        { theme, palette },
      );
      const colors = await page
        .locator('.information-workspace input')
        .first()
        .evaluate((e) => ({
          text: getComputedStyle(e).color,
          background: getComputedStyle(e).backgroundColor,
          accent: getComputedStyle(document.documentElement)
            .getPropertyValue('--accent')
            .trim(),
        }));
      assert(colors.text !== colors.background && colors.accent);
      themes.push({ theme, palette, ...colors });
    }
  }
  assert.deepEqual(errors, []);
  const result = {
    passed: true,
    checks: [
      'ordered tabs/cards',
      'automatic metadata + dirty draft merge',
      'optimistic payload + failed save preserves text',
      'linked detail title',
      'lock fields and manual scraping, scan independent',
      'literal arguments/environment and tool inheritance',
      'real picker IPC, inactive vs active, failed selection',
      'responsive light/dark + reduced motion',
    ],
    sizes,
    themes,
    errors,
    evidence,
  };
  writeFileSync(
    resolve(evidence, 'result.json'),
    JSON.stringify(result, null, 2),
  );
  console.log(JSON.stringify(result, null, 2));
} finally {
  await browser.close();
}
