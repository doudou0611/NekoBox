// Isolated UI regression with an in-memory IPC fixture; no user key or database is accessed.
import assert from 'node:assert/strict';
import { mkdirSync } from 'node:fs';
import { resolve } from 'node:path';
import { chromium } from '../.tools/browser-check/node_modules/playwright/index.mjs';
const base = process.env.PREVIEW_URL || 'http://127.0.0.1:1420';
assert(['127.0.0.1', 'localhost'].includes(new URL(base).hostname));
const evidence = resolve('.tools/translation-evidence');
mkdirSync(evidence, { recursive: true });
const browser = await chromium.launch({
  headless: true,
  executablePath:
    process.env.BROWSER_EXECUTABLE ||
    '/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge',
});
try {
  const page = await browser.newPage({
    viewport: { width: 1280, height: 950 },
  });
  const errors = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.addInitScript(() => {
    window.isTauri = true;
    window.translationFixture = {
      enabled: false,
      endpoint: '',
      model: '',
      has_api_key: false,
    };
    window.translationRequests = [];
    window.translationTestFails = false;
    window.__TAURI_INTERNALS__ = {
      invoke: async (command, { request }) => {
        window.translationRequests.push({ command, payload: request.payload });
        let data;
        if (command === 'get_translation_settings')
          data = window.translationFixture;
        else if (command === 'save_translation_settings') {
          const p = request.payload;
          data = window.translationFixture = {
            enabled: p.enabled,
            endpoint: p.endpoint,
            model: p.model,
            has_api_key: p.clear_api_key
              ? false
              : Boolean(p.api_key) || window.translationFixture.has_api_key,
          };
        } else if (command === 'test_translation') {
          if (window.translationTestFails)
            return {
              success: false,
              error_code: 'PERMISSION_DENIED',
              request_id: request.request_id,
              message: '翻译 API 密钥或权限无效；原文已保留。',
              data: null,
            };
          data = { translated_text: '关于友谊与夏日旅程的故事。' };
        } else throw Error(`Unexpected test command ${command}`);
        return {
          success: true,
          error_code: null,
          request_id: request.request_id,
          message: 'fixture',
          data,
        };
      },
    };
  });
  await page.route('**/__translation_component_test', (route) =>
    route.fulfill({
      contentType: 'text/html',
      body: `<html data-theme="dark"><head><meta charset="utf-8"></head><body><main id="test-root"></main><script type="module">
  import {createApp} from '/node_modules/.vite/deps/vue.js';
  import Panel from '/src/components/TranslationSettings.vue';
  import '/src/style.css'; import '/src/appearance.css';
  window.mountTranslation = () => { window.translationApp = createApp(Panel); window.translationApp.mount('#test-root'); }; window.mountTranslation();
  </script></body></html>`,
    }),
  );
  await page.goto(`${base}/__translation_component_test`);
  const toggle = page.getByRole('checkbox', {
    name: '翻译外文资料',
    exact: true,
  });
  await page
    .getByRole('button', { name: '保存翻译设置', exact: true })
    .waitFor();
  assert.equal(await toggle.isChecked(), false);
  assert.equal(
    await page.getByRole('button', { name: '保存并测试翻译' }).isDisabled(),
    true,
  );
  await toggle.check();
  await page
    .getByLabel('API 地址', { exact: true })
    .fill('https://fixture.example/v1');
  await page.getByLabel('模型名称', { exact: true }).fill('fixture-model');
  const key = page.getByLabel('API Key', { exact: true });
  assert.equal(await key.getAttribute('type'), 'password');
  await key.fill('fixture-secret');
  await page.getByRole('button', { name: '保存并测试翻译' }).click();
  await page.getByText('测试译文：关于友谊与夏日旅程的故事。').waitFor();
  assert.equal(await key.inputValue(), '');
  assert.equal(await key.getAttribute('placeholder'), '已保存密钥，留空保留');
  assert(!(await page.locator('body').innerText()).includes('fixture-secret'));
  await page.evaluate(() => {
    window.translationApp.unmount();
    window.mountTranslation();
  });
  await page.getByPlaceholder('已保存密钥，留空保留').waitFor();
  assert.equal(await toggle.isChecked(), true);
  assert.equal(
    await page.getByLabel('模型名称', { exact: true }).inputValue(),
    'fixture-model',
  );
  await page.getByRole('checkbox', { name: '清除已保存的密钥' }).check();
  await page.getByRole('button', { name: '保存翻译设置', exact: true }).click();
  await page.getByPlaceholder('输入密钥；本地无鉴权服务可留空').waitFor();
  await page.evaluate(() => {
    window.translationTestFails = true;
  });
  await page.getByRole('button', { name: '保存并测试翻译' }).click();
  await page
    .getByRole('alert')
    .getByText('翻译 API 密钥或权限无效；原文已保留。')
    .waitFor();
  await page.screenshot({
    path: resolve(evidence, 'desktop-fixture-dark.png'),
    fullPage: true,
  });
  await page.evaluate(() => {
    document.documentElement.dataset.theme = 'light';
  });
  await page.screenshot({
    path: resolve(evidence, 'desktop-fixture-light.png'),
    fullPage: true,
  });
  const requests = await page.evaluate(() => window.translationRequests);
  assert.deepEqual(
    requests.filter((r) => r.command === 'save_translation_settings')[0]
      .payload,
    {
      enabled: true,
      endpoint: 'https://fixture.example/v1',
      model: 'fixture-model',
      api_key: 'fixture-secret',
      clear_api_key: false,
    },
  );
  assert(
    requests
      .filter((r) => r.command === 'save_translation_settings')
      .slice(1)
      .every((r) => r.payload.api_key === null),
  );
  const preview = await browser.newPage({
    viewport: { width: 1280, height: 950 },
  });
  await preview.goto(`${base}/#/settings`);
  await preview
    .getByText('浏览器预览无法保存翻译配置或请求真实 API，请在桌面软件中配置。')
    .waitFor();
  assert.equal(
    await preview
      .getByRole('checkbox', { name: '翻译外文资料', exact: true })
      .isDisabled(),
    true,
  );
  await preview.screenshot({
    path: resolve(evidence, 'browser-settings.png'),
    fullPage: true,
  });
  assert.deepEqual(errors, []);
  console.log(
    'Translation settings: default off, save/test, masked key, remount, clear key, failure feedback, dark/light and browser isolation passed.',
  );
} finally {
  await browser.close();
}
