// Browser interaction with isolated IPC; never reads the user's database.
import assert from 'node:assert/strict';
import { chromium } from '../.tools/browser-check/node_modules/playwright/index.mjs';
const base = process.env.PREVIEW_URL || 'http://127.0.0.1:1420';
assert(['127.0.0.1', 'localhost'].includes(new URL(base).hostname));
const browser = await chromium.launch({
  headless: true,
  executablePath:
    process.env.BROWSER_EXECUTABLE ||
    '/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge',
});
try {
  for (const width of [1280, 520]) {
    const page = await browser.newPage({ viewport: { width, height: 900 } });
    const errors = [];
    page.on('pageerror', (e) => errors.push(e.message));
    const source = await (
      await page.request.get(`${base}/src/components/MetadataMatch.vue`)
    ).text();
    const vue = source.match(/from "([^"]*\/vue\.js[^"]*)"/)?.[1];
    assert(vue);
    await page.addInitScript(() => {
      window.isTauri = true;
      window.requests = [];
      window.pendingBangumi = null;
      window.__TAURI_INTERNALS__ = {
        convertFileSrc: (path) => path,
        invoke: async (command, { request }) => {
          window.requests.push({ command, payload: request.payload });
          const success = (data) => ({
            success: true,
            error_code: null,
            message: '隔离测试',
            request_id: request.request_id,
            data,
          });
          if (command === 'search_metadata') {
            const provider = request.payload.providers[0];
            const candidates = [
              {
                provider,
                remote_id: '123',
                title: `${provider} 作品`,
                subtitle: '原名',
                cover_url: 'https://fixture.invalid/cover.png',
                has_chinese_description: false,
                confidence: 1,
                matched_fields: ['title'],
                explanation: '',
                fetched_at: '',
                cached: false,
              },
            ];
            if (provider === 'bangumi')
              return new Promise((resolve) => {
                window.pendingBangumi = () => resolve(success(candidates));
              });
            return success(candidates);
          }
          if (command === 'confirm_metadata_match')
            return success({
              ...request.payload,
              matched_at: '',
              translation_message: null,
              supplementation_message: null,
              cover_message: '封面已缓存。',
            });
          if (command === 'get_game')
            return success({ id: 'fixture-game', title: '已更新作品' });
          throw Error(`Unexpected command ${command}`);
        },
      };
    });
    await page.route('https://fixture.invalid/cover.png', (route) =>
      route.fulfill({
        contentType: 'image/png',
        body: Buffer.from(
          'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+aO9sAAAAASUVORK5CYII=',
          'base64',
        ),
      }),
    );
    await page.route('**/__metadata_match_test', (route) =>
      route.fulfill({
        contentType: 'text/html',
        body: `<html><head><meta charset="utf-8"></head><body><div id="root"></div><script type="module">
        import {createApp,h} from '${vue}';
        import MetadataMatch from '/src/components/MetadataMatch.vue';
        import {metadataSources} from '/src/stores/metadataSources.ts';
        import '/src/style.css'; import '/src/appearance.css';
        metadataSources.sources.forEach(s=>s.enabled=s.provider==='vndb');
        window.testSources=metadataSources;
        createApp({render:()=>h(MetadataMatch,{gameId:'fixture-game',title:'作品',metadataStatus:'synced'})}).mount('#root');
      </script></body></html>`,
      }),
    );
    await page.goto(`${base}/__metadata_match_test`);
    await page.getByRole('button', { name: '搜索资料与封面' }).click();
    const dialog = page.getByRole('dialog', { name: '选择资料来源与作品' });
    await dialog.waitFor();
    assert.equal(await page.evaluate(() => window.requests.length), 0);
    assert.equal(await page.getByRole('button', { name: /解绑/ }).count(), 0);
    assert.equal(
      await dialog
        .getByRole('button', { name: 'Hikarinagi', exact: true })
        .isEnabled(),
      true,
    );
    await dialog.getByRole('button', { name: 'Bangumi', exact: true }).click();
    await page.waitForFunction(() => !!window.pendingBangumi);
    await dialog
      .getByRole('button', { name: 'Hikarinagi', exact: true })
      .click();
    await dialog.getByText('hikarinagi 作品', { exact: true }).waitFor();
    await dialog
      .getByRole('img', { name: 'hikarinagi 作品 · 作品封面' })
      .waitFor();
    await page.evaluate(() => window.pendingBangumi());
    assert.equal(
      await dialog.getByText('bangumi 作品', { exact: true }).count(),
      0,
    );
    assert.equal(
      await dialog.evaluate((node) => node.scrollWidth <= node.clientWidth),
      true,
    );
    await dialog.getByRole('button', { name: '确认绑定' }).click();
    await page.waitForFunction(() => !document.querySelector('dialog').open);
    const requests = await page.evaluate(() => window.requests);
    assert.deepEqual(
      requests
        .filter((r) => r.command === 'search_metadata')
        .map((r) => r.payload.providers),
      [['bangumi'], ['hikarinagi']],
    );
    assert(
      requests
        .filter((r) => r.command === 'search_metadata')
        .every((r) => r.payload.manual === true),
    );
    assert.deepEqual(
      requests.find((r) => r.command === 'confirm_metadata_match').payload,
      {
        game_id: 'fixture-game',
        title_hint: 'hikarinagi 作品',
        provider: 'hikarinagi',
        remote_id: '123',
        manual: true,
      },
    );
    assert.deepEqual(
      await page.evaluate(() =>
        window.testSources.sources
          .filter((s) => s.enabled)
          .map((s) => s.provider),
      ),
      ['vndb'],
    );
    assert.deepEqual(errors, []);
    await page.close();
  }
  console.log(
    '资料搜索弹窗回归：2 种窗口宽度通过；来源独立选择、迟到响应、封面及手动绑定有效（隔离 IPC，非原生真机验收）。',
  );
} finally {
  await browser.close();
}
