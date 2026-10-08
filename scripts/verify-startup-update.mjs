// Browser UI regression with an isolated update transport, never real downloads.
import assert from 'node:assert/strict';
import { mkdir, writeFile } from 'node:fs/promises';
import { chromium } from '../.tools/browser-check/node_modules/playwright/index.mjs';

const base = process.env.PREVIEW_URL || 'http://127.0.0.1:1420';
assert(['127.0.0.1', 'localhost'].includes(new URL(base).hostname));
const folder = '.tools/startup-update-evidence';
await mkdir(folder, { recursive: true });
const browser = await chromium.launch({
  headless: true,
  executablePath:
    process.env.BROWSER_EXECUTABLE ||
    '/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge',
});
const passed = [];
try {
  const page = await browser.newPage({
    viewport: { width: 1280, height: 800 },
  });
  const errors = [];
  page.on('pageerror', (error) => errors.push(error.message));
  await page.route('https://**/*', (route) => route.abort());
  // Enable only the real startup timer's desktop gate, keeping unrelated native
  // services out of this browser fixture.
  await page.route('**/src/App.vue*', async (route) => {
    if (new URL(route.request().url()).searchParams.has('type')) {
      await route.continue();
      return;
    }
    const response = await route.fetch();
    const body = await response.text();
    const gate = 'if (desktop) startupUpdateTimer = setTimeout';
    assert(body.includes(gate));
    await route.fulfill({
      response,
      body: body.replace(gate, 'startupUpdateTimer = setTimeout'),
    });
  });
  // Only this store sees a desktop transport. Library data remains browser demo
  // data; native installation, signature checks and OS composition are separate.
  await page.route('**/src/stores/updates.ts*', async (route) => {
    const response = await route.fetch();
    const body = await response.text();
    const importLine =
      /import \{ api, desktop, errorText \} from "[^"]+library\.ts[^"]*";/;
    assert(importLine.test(body));
    await route.fulfill({
      response,
      body:
        body.replace(
          importLine,
          'import { errorText } from "/src/stores/library.ts"; const desktop = true; const api = (...args) => window.__updateAPI(...args);',
        ) +
        '\nwindow.__updateStore = { updates, checkStartupUpdate, checkAppUpdate };',
    });
  });
  await page.addInitScript(() => {
    window.__updateCalls = [];
    window.__updateAPI = async (command) => {
      window.__updateCalls.push(command);
      if (command === 'check_app_update')
        return new Promise((resolve, reject) => {
          window.__finishUpdateCheck = resolve;
          window.__failUpdateCheck = reject;
        });
      if (command === 'download_app_update')
        return new Promise((resolve) => {
          window.__finishUpdateDownload = resolve;
        });
      if (command === 'get_app_update_status') {
        const { updates } = window.__updateStore;
        return { ...updates.status };
      }
    };
  });
  await page.goto(`${base}/#/games`);
  const search = page.locator('[data-library-search]');
  await search.waitFor();
  const panel = page.locator('.startup-update-panel');
  await search.fill('ATRI');
  assert.equal(await panel.count(), 0);
  await page.waitForFunction(() => window.__updateCalls.length === 1);
  await page.evaluate(() => {
    window.__failUpdateCheck(new Error('GitHub 无法连接'));
  });
  await page.waitForFunction(
    () => window.__updateStore.updates.status.phase === 'error',
  );
  assert.equal(await panel.count(), 0);
  assert.equal(
    await page.getByText('GitHub 无法连接', { exact: true }).count(),
    0,
  );
  await search.fill('');
  passed.push('后台检查挂起及 GitHub 失败不弹窗，搜索保持可用');

  await search.focus();
  const before = await page.locator('.gallery-stage').boundingBox();
  await page.evaluate(async () => {
    const { updates, checkAppUpdate } = window.__updateStore;
    window.__retryCheck = checkAppUpdate();
    window.__finishUpdateCheck({
      ...updates.status,
      phase: 'available',
      version: '0.3.1',
      signed: true,
      architecture: 'x64',
      installation: 'installer',
      notes:
        '更轻盈的界面，更从容的体验。\n\n- 改善游戏库与账户同步\n- 优化布局与动画\n- 修复若干已知问题',
    });
    await window.__retryCheck;
  });
  await panel.waitFor();
  await page.waitForTimeout(500);
  assert(await search.evaluate((n) => n === document.activeElement));
  assert.equal(await panel.evaluate((n) => n.matches(':modal')), false);
  assert.deepEqual(await page.locator('.gallery-stage').boundingBox(), before);
  await search.click();
  await search.fill('ATRI');
  await search.fill('');
  assert(await panel.isVisible());
  assert.deepEqual(await page.evaluate(() => window.__updateCalls), [
    'check_app_update',
    'check_app_update',
  ]);
  passed.push('发现新版显示非模态弹窗；不抢焦点、不改变页面布局、不自动下载');

  for (const theme of ['light', 'dark']) {
    await page.evaluate(async (theme) => {
      const { preview } = await import('/src/preview/store.ts');
      preview.theme = theme;
    }, theme);
    await page.waitForTimeout(200);
    for (const material of ['browser', 'windows-acrylic', 'macos-vibrancy']) {
      await page.evaluate((material) => {
        document.documentElement.dataset.windowMaterial = material;
        document.documentElement.dataset.globalGlass = 'on';
      }, material);
      assert.equal(
        await panel.evaluate((n) => getComputedStyle(n).backdropFilter),
        'blur(18px)',
      );
    }
    await panel.screenshot({ path: `${folder}/${theme}.png` });
  }
  for (const [width, height] of [
    [360, 640],
    [500, 500],
    [800, 600],
    [1280, 800],
  ]) {
    await page.setViewportSize({ width, height });
    await panel.locator('summary').click();
    await page.waitForTimeout(200);
    const bounds = await panel.boundingBox();
    assert(
      bounds.x >= 0 &&
        bounds.y >= 0 &&
        bounds.x + bounds.width <= width &&
        bounds.y + bounds.height <= height,
    );
    assert(await panel.evaluate((n) => n.scrollWidth <= n.clientWidth));
    await panel.screenshot({ path: `${folder}/${width}.png` });
    await panel.locator('summary').click();
  }
  passed.push('深浅主题、原生材质样式及 360/500/800/1280 布局通过');

  await panel.getByRole('button', { name: '下载更新', exact: true }).click();
  await panel.getByRole('progressbar').waitFor();
  await search.fill('ATRI');
  await panel.getByRole('button', { name: '收起，继续使用' }).click();
  await panel.waitFor({ state: 'hidden' });
  await page.evaluate(async () => {
    const { updates } = window.__updateStore;
    window.__finishUpdateDownload({ ...updates.status, phase: 'ready' });
  });
  await page.waitForFunction(async () => {
    const { updates } = window.__updateStore;
    return updates.status.phase === 'ready' && !updates.busy;
  });
  assert.equal(await panel.count(), 0);
  assert(
    !(await page.evaluate(() =>
      window.__updateCalls.includes('install_app_update'),
    )),
  );
  passed.push('下载期间继续搜索；关闭提示不取消下载，完成后不重开、不自动安装');

  await page.evaluate(async () => {
    const { updates } = window.__updateStore;
    updates.notice = true;
  });
  await panel.getByRole('button', { name: '前往安装' }).click();
  await page.locator('#settings-updates').waitFor();
  assert(page.url().includes('section=updates'));
  await panel.waitFor({ state: 'hidden' });
  assert.equal(await panel.count(), 0);
  passed.push('准备完成后进入设置更新章节，保留安装确认流程');

  await page.goto(`${base}/#/games`);
  await search.waitFor();
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await page.evaluate(async () => {
    const { updates } = window.__updateStore;
    window.__updateCalls = [];
    Object.assign(updates.status, {
      phase: 'available',
      version: '0.3.1',
      signed: false,
      installation: 'portable',
      download_url:
        'https://github.com/doudou0611/NekoBox/releases/download/0.3.1/NekoBox_0.3.1_x64-portable.zip',
    });
    updates.notice = true;
  });
  await panel.waitFor();
  assert.equal(
    await panel.evaluate((n) => getComputedStyle(n).transitionDuration),
    '0s',
  );
  await panel.getByRole('button', { name: '下载便携包' }).click();
  assert.deepEqual(await page.evaluate(() => window.__updateCalls), [
    'open_app_update_release',
  ]);
  await page.evaluate(() => {
    window.__updateStore.updates.status.download_url = null;
  });
  assert(
    await panel
      .getByRole('button', { name: '查看更新', exact: true })
      .isVisible(),
  );
  await panel.getByRole('button', { name: '关闭更新提示' }).focus();
  await page.keyboard.press('Escape');
  await panel.waitFor({ state: 'hidden' });
  passed.push('减少动态效果、便携版手动下载、键盘关闭通过');
  assert.deepEqual(errors, []);
  await writeFile(
    `${folder}/result.json`,
    JSON.stringify(
      {
        passed,
        errors,
        native_windows_verified: false,
        scope: 'browser UI + mocked update transport',
      },
      null,
      2,
    ),
  );
  console.log(
    JSON.stringify(
      { passed: true, scenarios: passed.length, evidence: folder },
      null,
      2,
    ),
  );
} finally {
  await browser.close();
}
