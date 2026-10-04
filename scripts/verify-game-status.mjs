// Real Vue/App interactions, with IPC fixtures exclusively in browser memory.
import assert from 'node:assert/strict';
import { mkdirSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { chromium } from '../.tools/browser-check/node_modules/playwright/index.mjs';
const base = process.env.PREVIEW_URL || 'http://127.0.0.1:1420';
assert(['127.0.0.1', 'localhost'].includes(new URL(base).hostname));
const evidence = resolve('.tools/game-status-evidence', String(Date.now()));
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
      delaySave: false,
      releaseSave: null,
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
        else if (command === 'update_game') {
          if (f.failSave) return failure('隔离测试：状态保存失败');
          if (f.delaySave)
            await new Promise((resolve) => {
              f.releaseSave = resolve;
            });
          Object.assign(f.summary, q);
          data = detail();
        } else if (command === 'update_game_metadata') {
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
  const router=createAppRouter(createMemoryHistory());window.detailTest.router=router;await router.push('/games/workspace-game');await router.isReady();createApp(App).use(router).mount('#test-root');
  </script></body></html>`,
    }),
  );
  await page.goto(`${base}/__workspace_test`);
  const trigger = page.getByRole('combobox', { name: '游玩状态', exact: true });
  await trigger.waitFor();
  const labels = ['通关', '在玩', '搁置', '弃坑', '未开始'];
  const values = ['completed', 'playing', 'paused', 'dropped', 'not_started'];
  const expectStatus = async (value, label) => {
    await page.waitForFunction(
      (v) => window.detailTest.preview.games[0]?.status === v,
      value,
    );
    assert.equal(await trigger.innerText(), label);
    const rail = page.locator('.group-game .game-status');
    assert((await rail.count()) > 0);
    assert((await rail.allTextContents()).every((text) => text === label));
  };
  for (let i = 0; i < labels.length; i++) {
    await trigger.click();
    assert.deepEqual(await page.getByRole('option').allTextContents(), [
      '通关故事已读完',
      '在玩继续这段旅程',
      '搁置暂时放下，留待以后',
      '弃坑在这里告一段落',
      '未开始等待故事开始',
    ]);
    await page.getByRole('option', { name: labels[i], exact: true }).click();
    await expectStatus(values[i], labels[i]);
    await page.getByRole('listbox').waitFor({ state: 'hidden' });
  }
  console.log('PASS: all five labels and sidebar sync');
  // Esc closes the menu and returns focus without leaving the detail page.
  await trigger.click();
  await page.keyboard.press('Escape');
  await page.getByRole('listbox').waitFor({ state: 'hidden' });
  assert.equal(
    await page.evaluate(() => window.detailTest.router.currentRoute.value.name),
    'game-detail',
  );
  assert(await trigger.evaluate((el) => el === document.activeElement));
  await trigger.focus();
  await page.keyboard.press('ArrowDown');
  await page.getByRole('listbox').waitFor();
  await page.waitForFunction(
    () => document.activeElement?.getAttribute('role') === 'option',
  );
  await page.keyboard.press('Home');
  await page.waitForFunction(
    () => document.activeElement?.dataset.status === 'completed',
  );
  await page.keyboard.press('ArrowDown');
  await page.waitForFunction(
    () => document.activeElement?.dataset.status === 'playing',
  );
  await page.keyboard.press('Enter');
  await expectStatus('playing', '在玩');
  console.log('PASS: keyboard');
  // Failures cannot optimistically alter either label.
  await page.evaluate(() => (window.fixture.failSave = true));
  await trigger.click();
  await page.getByRole('option', { name: '弃坑', exact: true }).click();
  await page.getByRole('status').filter({ hasText: '状态保存失败' }).waitFor();
  await expectStatus('playing', '在玩');
  assert(await trigger.isEnabled());
  await page.evaluate(() => {
    window.fixture.failSave = false;
    window.fixture.delaySave = true;
  });
  await trigger.click();
  await page.getByRole('option', { name: '搁置', exact: true }).click();
  await page.waitForFunction(() => !!window.fixture.releaseSave);
  assert(await trigger.isDisabled());
  assert.equal(await trigger.getAttribute('aria-busy'), 'true');
  await expectStatus('playing', '在玩');
  await page.evaluate(() => {
    window.fixture.releaseSave();
    window.fixture.delaySave = false;
  });
  await expectStatus('paused', '搁置');
  assert(await trigger.isEnabled());
  // A backend-only pending state remains visible, without inventing a sixth choice.
  await page.evaluate(async () => {
    window.fixture.summary.status = 'pending_confirmation';
    await window.detailTest.loadDetail('workspace-game');
  });
  await expectStatus('pending_confirmation', '待确认入口');
  await trigger.click();
  assert.equal(await page.getByRole('option').count(), 5);
  await page.getByRole('option', { name: '搁置', exact: true }).click();
  await expectStatus('paused', '搁置');
  for (const theme of ['light', 'dark']) {
    await page.evaluate((theme) => {
      window.detailTest.preview.theme = theme;
    }, theme);
    await page.waitForFunction(
      (theme) => document.documentElement.dataset.theme === theme,
      theme,
    );
    for (const width of [1280, 800, 500]) {
      await page.setViewportSize({ width, height: 900 });
      await trigger.click();
      await page.getByRole('listbox').waitFor();
      await page.waitForFunction(
        () =>
          !document
            .querySelector('.game-status-menu')
            ?.getAnimations()
            .some((a) => a.playState === 'running'),
      );
      const bounds = await page.getByRole('listbox').boundingBox();
      assert(bounds.x >= 0 && bounds.x + bounds.width <= width + 1);
      assert(bounds.y >= 0 && bounds.y + bounds.height <= 901);
      assert(
        await page.evaluate(
          () => document.documentElement.scrollWidth <= innerWidth,
        ),
      );
      const check = await page.getByRole('listbox').evaluate((el) => ({
        radius: getComputedStyle(el).borderRadius,
        surface: getComputedStyle(el).backgroundColor,
      }));
      assert.equal(check.radius, '18px');
      assert.notEqual(check.surface, 'rgba(0, 0, 0, 0)');
      await page.screenshot({
        path: resolve(evidence, `${theme}-${width}.png`),
      });
      await page.keyboard.press('Escape');
      await page.getByRole('listbox').waitFor({ state: 'hidden' });
    }
  }
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await trigger.click();
  assert.equal(
    await page
      .getByRole('listbox')
      .evaluate((el) => getComputedStyle(el).animationName),
    'none',
  );
  await page.keyboard.press('Escape');
  await page.emulateMedia({ reducedMotion: 'no-preference' });
  await page.evaluate(
    () => (document.documentElement.dataset.motion = 'reduced'),
  );
  await trigger.click();
  assert.equal(
    await page
      .getByRole('listbox')
      .evaluate((el) => getComputedStyle(el).animationName),
    'none',
  );
  await page.keyboard.press('Escape');
  // Fixed groups share status data with the detail select, including return navigation.
  await page.setViewportSize({ width: 1280, height: 900 });
  await page.evaluate(async () => {
    document.documentElement.dataset.motion = 'full';
    await window.detailTest.router.push('/games');
  });
  const groupBar = page.locator('.gallery-tabs');
  await groupBar.waitFor();
  assert.deepEqual(await groupBar.locator('button').allTextContents(), [
    '全部作品1',
    '我的收藏',
    '在玩',
    '通关',
    '搁置',
    '弃坑',
  ]);
  const expectCards = async (count) => {
    await page.waitForFunction(
      (count) =>
        document.querySelectorAll('[data-preview-card]').length === count,
      count,
    );
  };
  await groupBar.getByRole('button', { name: '搁置', exact: true }).click();
  await expectCards(1);
  await page.locator('[data-preview-open="gallery:workspace-game"]').click();
  await trigger.waitFor();
  await trigger.click();
  await page.getByRole('option', { name: '弃坑', exact: true }).click();
  await expectStatus('dropped', '弃坑');
  await page.getByRole('button', { name: '返回原展廊位置' }).click();
  await groupBar.waitFor();
  await expectCards(0);
  assert.equal(
    await groupBar
      .getByRole('button', { name: '搁置', exact: true })
      .getAttribute('aria-pressed'),
    'true',
  );
  await groupBar.getByRole('button', { name: '弃坑', exact: true }).click();
  await expectCards(1);
  await page.locator('[data-preview-open="gallery:workspace-game"]').click();
  await trigger.click();
  await page.getByRole('option', { name: '搁置', exact: true }).click();
  await expectStatus('paused', '搁置');
  await page.getByRole('button', { name: '返回原展廊位置' }).click();
  await groupBar.waitFor();
  await expectCards(0);
  assert.equal(
    await groupBar
      .getByRole('button', { name: '弃坑', exact: true })
      .getAttribute('aria-pressed'),
    'true',
  );
  await groupBar.getByRole('button', { name: '搁置', exact: true }).click();
  await expectCards(1);
  for (const [value, label] of [
    ['playing', '在玩'],
    ['completed', '通关'],
  ]) {
    await page.locator('[data-preview-open="gallery:workspace-game"]').click();
    await trigger.click();
    await page.getByRole('option', { name: label, exact: true }).click();
    await expectStatus(value, label);
    await page.getByRole('button', { name: '返回原展廊位置' }).click();
    await groupBar.waitFor();
    await expectCards(0);
    await groupBar.getByRole('button', { name: label, exact: true }).click();
    await expectCards(1);
  }
  await page.waitForFunction(
    () =>
      !document.querySelector(
        '[data-detail-stage], .page-enter-active, .page-leave-active, [data-shared-overlay]',
      ),
  );
  for (const theme of ['light', 'dark']) {
    await page.evaluate(
      (theme) => (window.detailTest.preview.theme = theme),
      theme,
    );
    for (const width of [1280, 500]) {
      await page.setViewportSize({ width, height: 900 });
      await groupBar.getByRole('button', { name: '弃坑', exact: true }).click();
      await expectCards(0);
      await page.waitForFunction(
        () =>
          document
            .querySelector('.gallery-tabs button.active')
            ?.textContent.trim() === '弃坑',
      );
      await page.waitForFunction(
        () =>
          !document
            .querySelector('.gallery-stage')
            ?.getAnimations({ subtree: true })
            .some((a) => a.playState === 'running'),
      );
      assert(
        await page.evaluate(
          () => document.documentElement.scrollWidth <= innerWidth,
        ),
      );
      await page.waitForFunction(
        () =>
          !document
            .querySelector('.gallery-tabs')
            ?.getAnimations({ subtree: true })
            .some((a) => a.playState === 'running'),
      );
      const bounds = await groupBar
        .getByRole('button', { name: '弃坑', exact: true })
        .boundingBox();
      assert(bounds.x >= 0 && bounds.x + bounds.width <= width + 1);
      await page.screenshot({
        path: resolve(evidence, `fixed-groups-${theme}-${width}.png`),
      });
    }
  }
  console.log(
    'PASS: four fixed groups, membership changes, retained return filter and responsive themes',
  );
  assert(!errors.length, errors.join('\n'));
  const commands = await page.evaluate(() => window.fixture.commands);
  assert(!commands.some((c) => c.command === 'launch_game'));
  const writes = commands.filter((c) => c.command === 'update_game');
  assert(
    values.every((value) => writes.some((c) => c.payload.status === value)),
  );
  writeFileSync(
    resolve(evidence, 'result.json'),
    JSON.stringify(
      {
        passed: true,
        themes: 2,
        widths: [1280, 800, 500],
        checks: [
          'five status options and sidebar sync',
          'keyboard and focus',
          'Esc preserves detail',
          'failed save retains status',
          'busy submission guard',
          'pending fallback',
          'viewport collision',
          'reduced motion',
          'four fixed groups match detail status and retain return filters',
        ],
        commands: writes,
        errors,
      },
      null,
      2,
    ),
  );
  console.log(
    'PASS: game status select and sidebar synchronization; ' + evidence,
  );
} catch (error) {
  const p = browser.contexts()[0]?.pages()[0];
  if (p) {
    await p.screenshot({ path: resolve(evidence, 'failure.png') });
    console.log(
      await p.evaluate(() => ({
        active: document.activeElement?.outerHTML,
        status: window.fixture?.summary.status,
        commands: window.fixture?.commands.slice(-6),
      })),
    );
  }
  throw error;
} finally {
  await browser.close();
}
