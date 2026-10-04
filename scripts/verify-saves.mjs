// Real detail component, isolated in-memory IPC. Never reads a production database or real saves.
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
const evidence = resolve('.tools/save-evidence', String(Date.now()));
mkdirSync(evidence, { recursive: true });
let page;
try {
  page = await browser.newPage({
    viewport: { width: 1440, height: 1100 },
  });
  const errors = [];
  page.on('pageerror', (error) => errors.push(error.message));
  const source = await (
    await page.request.get(`${base}/src/views/PreviewDetail.vue`)
  ).text();
  const libraryModule = source.match(
    /from "(\/src\/stores\/library\.ts[^"]*)"/,
  )?.[1];
  const vueModule = source.match(
    /from "(\/node_modules\/\.vite\/deps\/vue\.js[^"]*)"/,
  )?.[1];
  const routerModule = source.match(
    /from "(\/node_modules\/\.vite\/deps\/vue-router\.js[^"]*)"/,
  )?.[1];
  assert(libraryModule && vueModule && routerModule);
  await page.addInitScript(() => {
    window.isTauri = true;
    window.requests = [];
    window.failBackup = false;
    window.unavailable = false;
    window.onlyExtraFile = false;
    window.failProfileDelete = false;
    window.blockProfileDelete = false;
    window.profiles = [
      {
        id: 'profile-1',
        game_id: 'test-game',
        install_id: 'install-1',
        source_path: 'C:/fixture/故事/Save',
        backup_before_launch: false,
        backup_after_exit: true,
        retention_count: 10,
        source_available: true,
        last_error: null,
      },
    ];
    window.snapshots = Array.from({ length: 26 }, (_, n) => ({
      id: `snapshot-${n}`,
      game_id: 'test-game',
      save_profile_id: 'profile-1',
      label:
        n === 0
          ? '共通线结束，夏日的约定'
          : n === 1
            ? '恢复前的安全备份'
            : n === 2
              ? '第一章 · 海风吹过的时候'
              : `故事片刻 ${n + 1}`,
      note: n === 0 ? '下一次，从与你重逢的地方开始。' : null,
      created_at: new Date(Date.UTC(2026, 9, 3, 10 - n)).toISOString(),
      size_bytes: Math.round(1024 * 1024 * 2.4),
      file_count: 12,
      sha256: 'a'.repeat(64),
      creation_reason:
        n === 1
          ? 'safety_before_restore'
          : n % 3 === 0
            ? 'manual'
            : 'after_exit',
      archive_path: 'save-backups/fixture.zip',
    }));
    window.testGame = {
      id: 'test-game',
      title: '夏日的故事',
      title_ja: null,
      developer: null,
      publisher: null,
      release_date: null,
      source_rating: null,
      source_tags: [],
      tags: [],
      added_at: '2026-10-01T00:00:00Z',
      last_played_at: null,
      total_playtime_seconds: 0,
      favorite: false,
      hidden: false,
      status: 'not_started',
      cover_url: null,
      user_rating: null,
      metadata_status: 'local_only',
      description: '隔离存档界面测试',
      metadata: [],
      installations: [
        {
          id: 'install-1',
          game_id: 'test-game',
          absolute_path: 'C:/fixture/故事',
          source: 'manual',
        },
        {
          id: 'install-2',
          game_id: 'test-game',
          absolute_path: 'D:/fixture/另一个安装版本',
          source: 'manual',
        },
      ],
    };
    window.__TAURI_INTERNALS__ = {
      invoke: async (command, args = {}) => {
        if (command === 'plugin:dialog|open')
          return 'D:/fixture/另一个安装版本/Save';
        const request = args.request;
        if (!request) return 1;
        const payload = request.payload;
        window.requests.push({ command, payload });
        let data;
        if (command === 'get_game') data = window.testGame;
        else if (command === 'get_hikarinagi_rates') data = { wall: null };
        else if (command === 'get_bangumi_cover_status')
          data = { message: '隔离测试', remote_id: null };
        else if (command === 'list_save_profiles')
          data = window.profiles.map((p) => ({
            ...p,
            source_available: !window.unavailable,
          }));
        else if (command === 'list_save_snapshots') data = window.snapshots;
        else if (command === 'configure_save_profile') {
          const id = payload.id || 'profile-2';
          const p = {
            ...payload,
            id,
            game_id: 'test-game',
            source_available: true,
            last_error: null,
          };
          window.profiles = [
            ...window.profiles.filter((item) => item.id !== id),
            p,
          ];
          data = p;
        } else if (command === 'delete_save_profile') {
          if (window.blockProfileDelete)
            await new Promise((done) => (window.finishProfileDelete = done));
          if (window.failProfileDelete)
            return {
              success: false,
              error_code: 'DATABASE_ERROR',
              message: '隔离测试：配置删除失败，原记录已保留',
              request_id: request.request_id,
              data: null,
            };
          window.profiles = window.profiles.filter(
            (p) => p.id !== payload.profile_id,
          );
          window.snapshots = window.snapshots.filter(
            (s) => s.save_profile_id !== payload.profile_id,
          );
          data = true;
        } else if (command === 'detect_save_paths')
          data = ['D:/fixture/另一个安装版本/Save'];
        else if (command === 'create_save_snapshot') {
          if (window.failBackup)
            return {
              success: false,
              error_code: 'INTERNAL_ERROR',
              message: '隔离测试：备份失败，请重试',
              request_id: request.request_id,
              data: null,
            };
          const s = {
            ...window.snapshots[0],
            id: 'new-snapshot',
            save_profile_id: payload.profile_id,
            label: payload.label,
            note: payload.note,
            creation_reason: 'manual',
            created_at: '2026-10-04T00:00:00Z',
          };
          window.snapshots.unshift(s);
          data = s;
        } else if (command === 'preview_save_restore')
          data = {
            preview_id: 'preview-1',
            confirmation_token: 'isolated-token',
            snapshot_id: payload.snapshot_id,
            source_path: 'C:/fixture/故事/Save',
            expires_at: '2026-10-04T00:00:00Z',
            added_files: window.onlyExtraFile ? 0 : 1,
            modified_files: window.onlyExtraFile ? 0 : 2,
            deleted_files: 1,
            preserved_files: 0,
            changes: window.onlyExtraFile
              ? [{ path: 'slot05.sav', change: 'deleted' }]
              : [
                  { path: 'slot01.sav', change: 'added' },
                  { path: 'system.sav', change: 'modified' },
                  { path: 'slot05.sav', change: 'deleted' },
                ],
            truncated: false,
          };
        else if (command === 'restore_save_snapshot')
          data = {
            safety_backup_id: 'safety-backup',
            added_files: 1,
            modified_files: 2,
            deleted_files: 1,
          };
        else if (command === 'delete_save_snapshot') {
          window.snapshots = window.snapshots.filter(
            (s) => s.id !== payload.snapshot_id,
          );
          data = true;
        } else throw Error(`Unexpected isolated command: ${command}`);
        return {
          success: true,
          error_code: null,
          message: '隔离测试响应',
          request_id: request.request_id,
          data: structuredClone(data),
        };
      },
    };
  });
  await page.route('**/__save_detail_test', (route) =>
    route.fulfill({
      contentType: 'text/html',
      body: `<html data-theme="light" data-motion="full"><head><meta charset="utf-8"></head><body><div id="test-root"></div><script type="module">
    import { createApp, h } from '${vueModule}';
    import { createRouter, createMemoryHistory, RouterView } from '${routerModule}';
    import Detail from '/src/views/PreviewDetail.vue';
    import Feedback from '/src/components/preview/PreviewFeedback.vue';
    import { local, preview, displayGame } from '${libraryModule}';
    import '/src/style.css'; import '/src/appearance.css';
    local.records['test-game']=window.testGame; preview.games=[displayGame(window.testGame,window.testGame)];
    const router=createRouter({history:createMemoryHistory(),routes:[{path:'/games/:game_id',component:Detail}]});
    await router.push('/games/test-game'); await router.isReady();
    createApp({render:()=>h('div',[h(RouterView),h(Feedback)])}).use(router).mount('#test-root');
    </script></body></html>`,
    }),
  );
  await page.goto(`${base}/__save_detail_test`);
  await page.getByRole('tab', { name: '存档', exact: true }).click();
  await page.locator('.save-snapshot').first().waitFor();
  await page.locator('.save-panel').scrollIntoViewIfNeeded();
  const layouts = [];
  await page
    .getByRole('button', { name: '备份当前存档', exact: true })
    .waitFor();
  for (const width of [1440, 1100, 800, 420]) {
    await page.setViewportSize({ width, height: 1100 });
    await page.locator('.save-panel').scrollIntoViewIfNeeded();
    const layout = await page.evaluate(() => {
      const panel = document.querySelector('.save-panel');
      const columns = getComputedStyle(
        document.querySelector('.save-workspace'),
      ).gridTemplateColumns;
      return {
        width: innerWidth,
        panelWidth: panel.getBoundingClientRect().width,
        overflow: document.documentElement.scrollWidth > innerWidth,
        columns,
      };
    });
    assert.equal(layout.overflow, false, JSON.stringify(layout));
    layouts.push(layout);
    if ([1440, 800].includes(width))
      await page
        .locator('.save-panel')
        .screenshot({ path: `${evidence}/saves-${width}-light.png` });
  }
  await page.setViewportSize({ width: 1440, height: 1100 });
  await page
    .locator('.save-panel')
    .evaluate((el) => el.scrollIntoView({ block: 'start' }));
  await page.screenshot({ path: `${evidence}/preview-light.png` });
  await page.evaluate(() => (document.documentElement.dataset.theme = 'dark'));
  await page
    .locator('.save-panel')
    .screenshot({ path: `${evidence}/saves-dark.png` });
  await page.evaluate(() => (document.documentElement.dataset.theme = 'light'));
  assert.equal(await page.locator('.save-snapshot').count(), 20);
  await page.getByRole('button', { name: '显示更多快照' }).click();
  assert.equal(await page.locator('.save-snapshot').count(), 26);
  await page.getByRole('button', { name: '安全', exact: true }).click();
  await page.waitForFunction(
    () => document.querySelectorAll('.save-snapshot').length === 1,
  );
  assert.match(
    await page.locator('.save-snapshot').innerText(),
    /恢复前的安全备份/,
  );
  await page.getByRole('button', { name: '全部', exact: true }).click();
  await page.getByRole('searchbox', { name: '搜索快照' }).fill('夏日');
  await page.waitForFunction(
    () => document.querySelectorAll('.save-snapshot').length === 1,
  );
  await page.evaluate(() => (window.onlyExtraFile = true));
  await page.getByRole('button', { name: '预览恢复', exact: true }).click();
  await page.getByRole('region', { name: '恢复差异预览' }).waitFor();
  if (
    (await page
      .getByRole('button', { name: /存档设置/ })
      .getAttribute('aria-expanded')) === 'false'
  )
    await page.getByRole('button', { name: /存档设置/ }).click();
  assert(
    await page
      .getByRole('button', { name: '删除存档配置', exact: true })
      .isDisabled(),
  );
  assert.match(
    await page.getByRole('region', { name: '恢复差异预览' }).innerText(),
    /删除文件\s*1/,
  );
  assert.equal(
    await page.getByText('当前文件已与快照一致，无需恢复。').count(),
    0,
  );
  await page
    .getByRole('region', { name: '恢复差异预览' })
    .screenshot({ path: `${evidence}/restore-preview.png` });
  await page.getByRole('button', { name: '确认恢复…', exact: true }).click();
  await page.getByRole('dialog').waitFor();
  assert.match(
    await page.getByRole('dialog').innerText(),
    /新增 0 个、修改 0 个、删除 1 个文件/,
  );
  assert.match(await page.getByRole('dialog').innerText(), /先备份当前存档/);
  await page.getByRole('button', { name: '取消', exact: true }).click();
  assert.equal(
    await page.evaluate(
      () =>
        window.requests.filter((r) => r.command === 'restore_save_snapshot')
          .length,
    ),
    0,
  );
  await page.getByRole('button', { name: '确认恢复…', exact: true }).click();
  await page
    .getByRole('button', { name: '创建安全备份并恢复', exact: true })
    .click();
  await page.waitForFunction(() =>
    window.requests.some((r) => r.command === 'restore_save_snapshot'),
  );
  assert.deepEqual(
    await page.evaluate(
      () =>
        window.requests.find((r) => r.command === 'restore_save_snapshot')
          .payload,
    ),
    {
      snapshot_id: 'snapshot-0',
      preview_id: 'preview-1',
      confirmation_token: 'isolated-token',
    },
  );
  await page
    .getByRole('region', { name: '恢复差异预览' })
    .waitFor({ state: 'hidden' });
  await page.getByRole('button', { name: /移除快照：共通线/ }).click();
  await page.getByRole('button', { name: '取消', exact: true }).click();
  assert.equal(
    await page.evaluate(
      () =>
        window.requests.filter((r) => r.command === 'delete_save_snapshot')
          .length,
    ),
    0,
  );
  await page.getByRole('button', { name: /移除快照：共通线/ }).click();
  await page.getByRole('button', { name: '确认移除快照', exact: true }).click();
  await page.waitForFunction(() =>
    window.requests.some(
      (r) =>
        r.command === 'delete_save_snapshot' && r.payload.confirmed === true,
    ),
  );
  await page.getByRole('searchbox', { name: '搜索快照' }).fill('');
  if (
    (await page
      .getByRole('button', { name: /存档设置/ })
      .getAttribute('aria-expanded')) === 'false'
  )
    await page.getByRole('button', { name: /存档设置/ }).click();
  const retentionInput = page.getByRole('spinbutton');
  await retentionInput.fill('0');
  assert.equal(
    await page
      .getByRole('button', { name: '保存存档配置', exact: true })
      .isDisabled(),
    true,
  );
  await retentionInput.fill('10');
  await page.getByRole('switch', { name: '启动前备份' }).focus();
  await page.keyboard.press('Space');
  assert.equal(
    await page.getByRole('switch', { name: '启动前备份' }).isChecked(),
    true,
  );
  assert.equal(
    await page.evaluate(
      () =>
        window.requests.filter((r) => r.command === 'configure_save_profile')
          .length,
    ),
    0,
  );
  await page.getByRole('button', { name: '保存存档配置', exact: true }).click();
  await page.waitForFunction(() => window.profiles[0].backup_before_launch);
  await page.getByRole('button', { name: /存档设置/ }).click();
  assert.equal(
    await page
      .getByRole('button', { name: '选择目录', exact: true })
      .isDisabled(),
    true,
  );
  await page
    .getByRole('combobox', { name: /^安装版本/ })
    .selectOption('install-2');
  await page.locator('.save-onboarding').waitFor();
  await page
    .locator('.save-workspace')
    .screenshot({ path: `${evidence}/first-configuration.png` });
  assert.equal(await page.locator('.save-snapshot').count(), 0);
  await page
    .getByRole('button', { name: '识别 Save/save', exact: true })
    .click();
  await page.getByRole('button', { name: '保存存档配置', exact: true }).click();
  await page.locator('.save-history-empty').waitFor();
  await page.getByLabel('快照名称', { exact: false }).fill('第二安装的章节');
  await page.evaluate(() => (window.failBackup = true));
  await page.getByRole('button', { name: '备份当前存档', exact: true }).click();
  await page.getByRole('alert').filter({ hasText: '备份失败' }).waitFor();
  assert.equal(
    await page.getByLabel('快照名称', { exact: false }).inputValue(),
    '第二安装的章节',
  );
  await page.evaluate(() => (window.failBackup = false));
  await page.getByRole('button', { name: '备份当前存档', exact: true }).click();
  await page.locator('.save-snapshot').waitFor();
  assert.equal(
    await page.evaluate(
      () =>
        window.requests
          .filter((r) => r.command === 'create_save_snapshot')
          .at(-1).payload.profile_id,
    ),
    'profile-2',
  );
  await page.evaluate(() => (window.unavailable = true));
  await page.getByRole('button', { name: '刷新记录', exact: true }).click();
  await page
    .getByRole('alert')
    .filter({ hasText: '此存档目录不可访问' })
    .waitFor();
  assert.equal(
    await page
      .getByRole('button', { name: '备份当前存档', exact: true })
      .isDisabled(),
    true,
  );
  assert.equal(
    await page
      .getByRole('button', { name: '预览恢复', exact: true })
      .isDisabled(),
    true,
  );
  const deleteProfile = page.getByRole('button', {
    name: '删除存档配置',
    exact: true,
  });
  if (
    (await page
      .getByRole('button', { name: /存档设置/ })
      .getAttribute('aria-expanded')) === 'false'
  )
    await page.getByRole('button', { name: /存档设置/ }).click();
  const profileDeleteCalls = () =>
    page.evaluate(
      () =>
        window.requests.filter((r) => r.command === 'delete_save_profile')
          .length,
    );
  await deleteProfile.click();
  const dialog = page.getByRole('dialog');
  await dialog.waitFor();
  assert.match(await dialog.innerText(), /另一个安装版本\/Save/);
  assert.match(await dialog.innerText(), /1 份历史快照/);
  assert.match(await dialog.innerText(), /原游戏存档不修改/);
  await dialog.getByRole('button', { name: '取消', exact: true }).click();
  assert.equal(await profileDeleteCalls(), 0);
  await deleteProfile.click();
  await page.keyboard.press('Escape');
  await dialog.waitFor({ state: 'hidden' });
  assert.equal(await profileDeleteCalls(), 0);
  await page.evaluate(() => (window.failProfileDelete = true));
  await deleteProfile.click();
  await dialog
    .getByRole('button', { name: '确认删除存档配置', exact: true })
    .click();
  await page.getByRole('alert').filter({ hasText: '配置删除失败' }).waitFor();
  assert.equal(await page.locator('.save-snapshot').count(), 1);
  assert.equal(await page.evaluate(() => window.profiles.length), 2);
  await page.setViewportSize({ width: 420, height: 1100 });
  await page.evaluate(() => {
    window.failProfileDelete = false;
    window.blockProfileDelete = true;
  });
  await deleteProfile.click();
  await page.waitForFunction(() => {
    const scrim = document.querySelector('.modal-scrim');
    return (
      scrim &&
      getComputedStyle(scrim).opacity === '1' &&
      !scrim.classList.contains('modal-enter-active')
    );
  });
  const dialogBounds = await dialog.boundingBox();
  assert(
    dialogBounds.x >= 0 &&
      dialogBounds.x + dialogBounds.width <= page.viewportSize().width,
  );
  await dialog.screenshot({
    path: `${evidence}/profile-delete-confirmation.png`,
  });
  await dialog
    .getByRole('button', { name: '确认删除存档配置', exact: true })
    .click();
  await page.waitForFunction(
    () => typeof window.finishProfileDelete === 'function',
  );
  assert(await deleteProfile.isDisabled());
  assert(
    await page
      .getByRole('button', { name: '刷新记录', exact: true })
      .isDisabled(),
  );
  await page.evaluate(() => window.finishProfileDelete());
  await page.locator('.save-onboarding').waitFor();
  assert.equal(await page.locator('.save-snapshot').count(), 0);
  assert.equal(await page.evaluate(() => window.profiles.length), 1);
  assert.deepEqual(
    await page.evaluate(
      () =>
        window.requests
          .filter((r) => r.command === 'delete_save_profile')
          .at(-1).payload,
    ),
    { profile_id: 'profile-2', confirmed: true },
  );
  await page
    .getByRole('combobox', { name: /^安装版本/ })
    .selectOption('install-1');
  await page
    .getByRole('button', { name: '删除存档配置', exact: true })
    .waitFor();
  assert.equal(await page.evaluate(() => window.profiles[0].id), 'profile-1');
  await page.evaluate(
    () => (document.documentElement.dataset.motion = 'reduced'),
  );
  assert.equal(
    await page
      .locator('.save-chevron')
      .evaluate((e) => getComputedStyle(e).transitionDuration),
    '0s',
  );
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await page.evaluate(() => (document.documentElement.dataset.motion = 'full'));
  assert.equal(
    await page
      .locator('.save-overview')
      .evaluate((e) => getComputedStyle(e).animationName),
    'none',
  );
  const readonly = await browser.newPage();
  await readonly.route('**/__save_readonly_test', (route) =>
    route.fulfill({
      contentType: 'text/html',
      body: `<html data-theme="light"><body><div id="test-root"></div><script type="module">
      import {createApp} from '${vueModule}';
      import Saves from '/src/components/SavePanel.vue';
      import '/src/style.css';
      createApp(Saves,{gameId:'browser-only'}).mount('#test-root');
    </script></body></html>`,
    }),
  );
  await readonly.goto(`${base}/__save_readonly_test`);
  await readonly
    .getByText('浏览器预览不读取或备份真实存档。', { exact: false })
    .waitFor();
  assert.equal(
    await readonly.getByRole('button', { name: '备份当前存档' }).count(),
    0,
  );
  await readonly.close();
  assert.deepEqual(errors, []);
  writeFileSync(
    `${evidence}/result.json`,
    JSON.stringify(
      {
        passed: true,
        layouts,
        errors,
        verification:
          'macOS Chromium; isolated IPC; no real files or Windows claim',
      },
      null,
      2,
    ),
  );
  console.log(JSON.stringify({ passed: true, evidence, layouts }, null, 2));
} catch (error) {
  if (page) {
    await page.screenshot({ path: `${evidence}/failure.png` });
    console.log(await page.locator('.save-panel').innerText());
    console.log(
      await page.evaluate(() => ({
        requests: window.requests.slice(-12),
        profiles: window.profiles,
        snapshots: window.snapshots.slice(0, 2),
      })),
    );
  }
  throw error;
} finally {
  await browser.close();
}
