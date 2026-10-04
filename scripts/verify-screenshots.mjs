// Isolated component test with an in-memory IPC adapter, never a production DB or Windows claim.
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
const evidence = resolve('.tools/screenshot-evidence', String(Date.now()));
mkdirSync(evidence, { recursive: true });
try {
  const page = await browser.newPage({
    viewport: { width: 1200, height: 900 },
  });
  const errors = [];
  let missingThumbnail = false;
  let missingOriginal = false;
  const resourceRequests = [];
  const layoutResults = [];
  // Use the component's resolved imports, including Vite's HMR/cache query.
  // A second store module would leave the fixture's installations invisible.
  const panelSource = await (
    await page.request.get(`${base}/src/views/PreviewDetail.vue`)
  ).text();
  const libraryModule = panelSource.match(
    /from "(\/src\/stores\/library\.ts[^"]*)"/,
  )?.[1];
  const vueModule = panelSource.match(
    /from "(\/node_modules\/\.vite\/deps\/vue\.js[^"]*)"/,
  )?.[1];
  const routerModule = panelSource.match(
    /from "(\/node_modules\/\.vite\/deps\/vue-router\.js[^"]*)"/,
  )?.[1];
  assert(
    libraryModule && vueModule && routerModule,
    'Vite component imports must resolve',
  );
  page.on('pageerror', (e) => errors.push(e.message));
  await page.addInitScript(() => {
    window.isTauri = true;
    window.testScreenshot = {
      id: 'a1234567-89ab-cdef-0123-456789abcdef',
      game_id: 'test-game',
      install_id: 'test-install',
      image_url: 'screenshot://localhost/a1234567-89ab-cdef-0123-456789abcdef',
      thumbnail_url: `screenshot://localhost/thumbnail/a1234567-89ab-cdef-0123-456789abcdef/${'a'.repeat(64)}.png`,
      title: '路线截图',
      captured_at: null,
      is_spoiler: false,
      created_at: '2026-10-01T00:00:00Z',
    };
    window.testRequests = [];
    window.testIpcRequests = [];
    window.testScreenshotCount = 1;
    window.testInstallation = {
      executable_path: 'C:/fixture/game.exe',
      steam_app_id: '45300',
      candidates: [
        {
          path: 'C:/fixture/game.exe',
          product_name: null,
          file_description: null,
          company_name: null,
          fingerprint: 'isolated-fixture',
          title_evidence: [],
        },
      ],
    };
    window.testGame = {
      id: 'test-game',
      title: '截图布局测试',
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
      description: '隔离截图测试',
      metadata: [],
      installations: [
        {
          id: 'test-install',
          game_id: 'test-game',
          absolute_path: 'C:/fixture',
          source: 'manual',
        },
        {
          id: 'test-install-2',
          game_id: 'test-game',
          absolute_path: 'D:/fixture-2',
          source: 'manual',
        },
      ],
    };
    window.__TAURI_INTERNALS__ = {
      convertFileSrc: (path, protocol) => {
        window.testRequests.push({ protocol, path });
        // Preserve the actual Windows runtime's whole-input URI encoding.
        return `http://${protocol}.localhost/${encodeURIComponent(path)}`;
      },
      invoke: async (command, args = {}) => {
        if (command === 'plugin:dialog|message') return '删除';
        const { request } = args;
        window.testIpcRequests.push({ command, payload: request.payload });
        let data;
        if (command === 'list_screenshots')
          data =
            request.payload.include_spoilers ||
            !window.testScreenshot.is_spoiler
              ? Array.from(
                  { length: window.testScreenshotCount },
                  (_, index) => ({
                    ...window.testScreenshot,
                    id: index
                      ? `b1234567-89ab-cdef-0123-${String(index).padStart(12, '0')}`
                      : window.testScreenshot.id,
                  }),
                )
              : [];
        else if (command === 'export_screenshots')
          data = {
            cancelled: false,
            completed: request.payload.screenshot_ids.length,
            failures: [],
          };
        else if (command === 'delete_screenshots') {
          window.testScreenshotCount = 0;
          data = {
            cancelled: false,
            completed: request.payload.screenshot_ids.length,
            failures: [],
          };
        } else if (command === 'update_screenshot') {
          window.testScreenshot = {
            ...window.testScreenshot,
            ...request.payload,
          };
          data = window.testScreenshot;
        } else if (command === 'scan_screenshots') data = 1;
        else if (command === 'get_game') data = window.testGame;
        else if (command === 'get_hikarinagi_rates') data = { wall: null };
        else if (
          command === 'list_external_sources' ||
          command === 'list_save_profiles'
        )
          data = [];
        else if (command === 'get_bangumi_cover_status')
          data = { message: '隔离测试无封面', remote_id: null };
        else if (command === 'get_installation')
          data = {
            ...window.testGame.installations.find(
              (i) => i.id === request.payload.install_id,
            ),
            executable_path: null,
            arguments: [],
            environment: {},
            working_directory: null,
            main_process_name: null,
            track_after_launcher_exit: true,
            idle_timeout_minutes: null,
            steam_app_id: null,
            candidates: [],
            ...window.testInstallation,
          };
        else if (command === 'configure_installation') {
          window.testInstallation = {
            ...window.testInstallation,
            ...request.payload,
          };
          data = {
            ...window.testGame.installations.find(
              (i) => i.id === request.payload.install_id,
            ),
            ...window.testInstallation,
          };
        } else throw Error(`Unexpected test command ${command}`);
        return {
          success: true,
          error_code: null,
          message: '隔离测试响应',
          request_id: request.request_id,
          data,
        };
      },
    };
  });
  await page.route('**/__screenshot_component_test', (route) =>
    route.fulfill({
      contentType: 'text/html',
      body: `<html><head><meta charset="utf-8"></head><body><div id="test-root"></div><script type="module">
    import { createApp } from '${vueModule}';
    import { createRouter, createMemoryHistory, RouterView } from '${routerModule}';
    import Detail from '/src/views/PreviewDetail.vue';
    import { local, preview, displayGame } from '${libraryModule}';
    import '/src/style.css'; import '/src/appearance.css';
    local.records['test-game']=window.testGame;
    preview.games=[displayGame(window.testGame,window.testGame)];
    const router=createRouter({history:createMemoryHistory(),routes:[{path:'/games/:game_id',component:Detail}]});
    await router.push('/games/test-game');
    await router.isReady();
    createApp(RouterView).use(router).mount('#test-root');
    </script></body></html>`,
    }),
  );
  await page.route('http://screenshot.localhost/**', (route) => {
    const path = new URL(route.request().url()).pathname;
    resourceRequests.push(path);
    const id = 'a1234567-89ab-cdef-0123-456789abcdef';
    const thumbnail = `/thumbnail/${id}/${'a'.repeat(64)}.png`;
    if (path !== thumbnail && path !== `/${id}`)
      return route.fulfill({ status: 400, body: 'Invalid screenshot route' });
    if (
      (path === thumbnail && missingThumbnail) ||
      (path === `/${id}` && missingOriginal)
    )
      return route.fulfill({ status: 404, body: 'Missing fixture image' });
    return route.fulfill({
      contentType: 'image/png',
      body: Buffer.from(
        'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+jE1sAAAAASUVORK5CYII=',
        'base64',
      ),
    });
  });
  async function showGallery() {
    await page.getByRole('tab', { name: '截图', exact: true }).click();
    await page.locator('.screenshot-grid').waitFor();
    await page.locator('.screenshot-grid').scrollIntoViewIfNeeded();
  }
  await page.goto(`${base}/__screenshot_component_test`);
  assert.equal(await page.locator('.detail-copy .eyebrow').count(), 0);
  await page.getByRole('tab', { name: '启动', exact: true }).click();
  await page.getByLabel('候选入口').waitFor();
  assert.equal(await page.getByLabel(/Steam\s*AppID/i).count(), 0);
  assert.equal(
    await page.locator('.binding-form option[value="steam"]').count(),
    0,
  );
  assert.equal(
    await page.getByLabel('完整 EXE / BAT 路径', { exact: true }).count(),
    0,
  );
  await page.getByLabel('候选入口').selectOption('__custom__');
  await page
    .getByLabel('完整 EXE / BAT 路径', { exact: true })
    .fill('C:/fixture/custom.exe');
  await page.getByRole('button', { name: '保存启动配置', exact: true }).click();
  await page.waitForFunction(
    () => window.testInstallation.executable_path === 'C:/fixture/custom.exe',
  );
  await showGallery();
  await page.getByRole('tab', { name: '启动', exact: true }).click();
  await page.locator('#panel-launch .launch-workspace').waitFor();
  await page.getByLabel('完整 EXE / BAT 路径', { exact: true }).waitFor();
  assert.equal(
    await page.getByLabel('完整 EXE / BAT 路径', { exact: true }).inputValue(),
    'C:/fixture/custom.exe',
  );
  await page.getByLabel('候选入口').selectOption('C:/fixture/game.exe');
  await page
    .getByLabel('完整 EXE / BAT 路径', { exact: true })
    .waitFor({ state: 'hidden' });
  assert.equal(
    await page.getByLabel('完整 EXE / BAT 路径', { exact: true }).count(),
    0,
  );
  await page.getByRole('button', { name: '保存启动配置', exact: true }).click();
  await page.waitForFunction(
    () => window.testInstallation.executable_path === 'C:/fixture/game.exe',
  );
  const entryUpdates = await page.evaluate(() =>
    window.testIpcRequests.filter(
      (r) => r.command === 'configure_installation',
    ),
  );
  assert.equal(entryUpdates.length, 2);
  assert(entryUpdates.every((r) => r.payload.steam_app_id === '45300'));
  assert.equal(
    await page.evaluate(() =>
      window.testIpcRequests.some((r) => r.command === 'launch_game'),
    ),
    false,
  );
  await page.getByRole('tab', { name: '存档', exact: true }).click();
  await page.locator('.detail-panel--saves').waitFor();
  for (const width of [800, 1000, 1280]) {
    await page.setViewportSize({ width, height: 600 });
    assert.equal(
      await page
        .locator('.detail-panel--saves')
        .evaluate((n) => getComputedStyle(n).backgroundColor),
      'rgba(0, 0, 0, 0)',
    );
    assert(
      await page.evaluate(
        () => document.documentElement.scrollWidth <= window.innerWidth,
      ),
    );
  }
  await page.setViewportSize({ width: 1200, height: 900 });

  await showGallery();
  const previewLoaded = () =>
    page.waitForFunction(() => {
      const img = document.querySelector('.screenshot-card img');
      return img?.complete && img.naturalWidth > 0;
    });
  await previewLoaded();
  assert(resourceRequests[0].startsWith('/thumbnail/'));
  assert(!resourceRequests[0].includes('%2F'));
  await page.screenshot({ path: resolve(evidence, 'gallery.png') });
  assert.equal(
    await page.getByRole('button', { name: '批量选择', exact: true }).count(),
    1,
  );
  assert.equal(
    await page.locator('.screenshots-panel').getByText('C:/fixture').count(),
    0,
  );
  await page.evaluate(() => {
    window.testScreenshotCount = 8;
  });
  await page.getByRole('tab', { name: '资料', exact: true }).click();
  await page.locator('.screenshot-scan-panel').waitFor();
  await showGallery();
  assert.equal(await page.locator('.screenshot-card').count(), 8);
  for (const width of [420, 900, 1280, 1800]) {
    await page.setViewportSize({ width, height: 1000 });
    await page.evaluate(
      () =>
        new Promise((resolve) =>
          requestAnimationFrame(() => requestAnimationFrame(resolve)),
        ),
    );
    const layout = await page.locator('.screenshot-grid').evaluate((grid) => ({
      columns: getComputedStyle(grid).gridTemplateColumns.split(' ').length,
      gridWidth: grid.getBoundingClientRect().width,
      panelWidth: grid.closest('.detail-panel').getBoundingClientRect().width,
      overflows: grid.scrollWidth > grid.clientWidth + 1,
    }));
    assert(
      Math.abs(layout.gridWidth - layout.panelWidth) < 2,
      'Gallery must fill detail panel',
    );
    assert(!layout.overflows, 'Gallery must not overflow horizontally');
    if (width === 420) assert.equal(layout.columns, 1);
    if (width === 900) assert(layout.columns >= 3);
    if (width === 1280) assert(layout.columns >= 4);
    if (width === 1800) assert(layout.columns >= 5);
    layoutResults.push({ width, ...layout });
  }
  await page.screenshot({ path: resolve(evidence, 'gallery-wide.png') });
  await page.setViewportSize({ width: 1200, height: 900 });
  missingThumbnail = true;
  await page.reload();
  await showGallery();
  await previewLoaded();
  assert.equal(
    new URL(await page.locator('.screenshot-card img').getAttribute('src'))
      .pathname,
    '/a1234567-89ab-cdef-0123-456789abcdef',
  );
  missingOriginal = true;
  const beforeFailure = resourceRequests.length;
  await page.reload();
  await showGallery();
  await page.getByText('预览无法读取，请重新扫描').waitFor();
  assert.equal(await page.locator('.screenshot-card img').count(), 0);
  assert.equal(resourceRequests.length - beforeFailure, 2);
  missingOriginal = false;
  missingThumbnail = false;
  await page.getByRole('tab', { name: '资料', exact: true }).click();
  await page.locator('.screenshot-scan-panel').waitFor();
  assert.equal(await page.locator('.screenshot-scan-sources li').count(), 2);
  await page
    .getByRole('button', { name: '扫描截图：D:/fixture-2', exact: true })
    .click();
  assert.equal(
    await page.evaluate(
      () =>
        window.testIpcRequests
          .filter((r) => r.command === 'scan_screenshots')
          .at(-1).payload.install_id,
    ),
    'test-install-2',
  );
  await page.screenshot({ path: resolve(evidence, 'scan-sources.png') });
  await showGallery();
  await previewLoaded();
  assert(
    new URL(
      await page.locator('.screenshot-card img').getAttribute('src'),
    ).pathname.startsWith('/thumbnail/'),
  );
  await page.locator('.screenshot-card').click();
  const dialog = page.getByRole('dialog');
  await dialog.waitFor();
  await page.waitForFunction(() => {
    const dialog = document.querySelector('dialog');
    return dialog?.open && dialog.contains(document.activeElement);
  });
  assert.equal(await page.getByLabel('截图标题').count(), 0);
  assert.equal(await page.getByLabel('显示剧透').count(), 0);
  assert.equal(await page.getByLabel('标记为剧透').count(), 0);
  await page.getByRole('button', { name: '保存', exact: true }).click();
  assert.equal(
    await page.evaluate(
      () =>
        window.testIpcRequests.find((r) => r.command === 'export_screenshots')
          .payload.batch,
    ),
    false,
  );
  await page.getByRole('button', { name: '放大', exact: true }).click();
  assert.equal(
    await page.getByRole('button', { name: '恢复比例' }).textContent(),
    '125%',
  );
  await page.getByRole('button', { name: '关闭图片' }).click();
  await page.evaluate(() => (window.testScreenshot.is_spoiler = true));
  await page.getByRole('button', { name: '批量选择' }).click();
  assert(
    await page.getByRole('button', { name: '保存', exact: true }).isDisabled(),
  );
  assert(
    await page.getByRole('button', { name: '删除', exact: true }).isDisabled(),
  );
  await page.locator('.screenshot-card').click();
  await page.getByRole('button', { name: '保存', exact: true }).click();
  assert.equal(
    await page.evaluate(
      () =>
        window.testIpcRequests
          .filter((r) => r.command === 'export_screenshots')
          .at(-1).payload.batch,
    ),
    true,
  );
  await page.screenshot({ path: resolve(evidence, 'selection.png') });
  await page.getByRole('button', { name: '删除', exact: true }).click();
  await page.waitForFunction(
    () => document.querySelectorAll('.screenshot-card').length === 0,
  );
  assert.equal(
    await page.evaluate(() =>
      window.testIpcRequests.some(
        (r) => r.command === 'delete_screenshots' && r.payload.confirmed,
      ),
    ),
    true,
  );
  assert.deepEqual(errors, []);
  const requests = await page.evaluate(() => window.testRequests);
  assert(
    requests.some(
      (item) => item.protocol === 'screenshot' && item.path === 'thumbnail',
    ),
  );
  writeFileSync(
    resolve(evidence, 'result.json'),
    JSON.stringify(
      {
        platform: process.platform,
        adapter: 'isolated-memory-ipc',
        passed: [
          '详情冗余眉题移除',
          'SteamAppID 控件隐藏且保存入口时保留旧值',
          '自定义路径展开、再次打开和切回候选；仅保存不启动',
          '存档外层背景透明与 800/1000/1280 宽度无横向溢出',
          '截图标题与剧透编辑控件移除',
          '查看器 dialog 初始焦点、缩放和关闭',
          '单张和批量原图保存请求',
          '未选择时批量保存删除禁用；确认后删除选中对象',
          '原图/缩略图协议转换',
          '模拟 Windows 编码后缩略图实际加载',
          '缩略图缺失回退已登记原图',
          '原图也失败后停止重试并提示',
          '重新扫描恢复缩略图预览',
          '真实详情页截图网格随窗口在一列至多列间调整',
          '截图页无扫描路径或按钮',
          '资料按指定安装版本扫描并切回最新索引',
        ],
        layoutResults,
        errors,
        windows_tested: false,
      },
      null,
      2,
    ),
  );
  console.log(`截图组件回归通过：${evidence}`);
} finally {
  await browser.close();
}
