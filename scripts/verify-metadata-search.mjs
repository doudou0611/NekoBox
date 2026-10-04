// Real MetadataMatch component, isolated IPC. Never touches a production DB.
import assert from 'node:assert/strict';
import { mkdirSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { chromium } from '../.tools/browser-check/node_modules/playwright/index.mjs';
const base = process.env.PREVIEW_URL || 'http://127.0.0.1:1420';
assert(['localhost', '127.0.0.1'].includes(new URL(base).hostname));
const browser = await chromium.launch({
  headless: true,
  executablePath:
    process.env.BROWSER_EXECUTABLE ||
    '/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge',
});
const evidence = resolve('.tools/search-cancel-evidence', String(Date.now()));
mkdirSync(evidence, { recursive: true });
const errors = [],
  passed = [];
const page = await browser.newPage({ viewport: { width: 1280, height: 800 } });
page.setDefaultTimeout(10000);
page.on('pageerror', (e) => errors.push(e.message));
try {
  const source = await (
    await page.request.get(`${base}/src/components/MetadataMatch.vue`)
  ).text();
  const vue = source.match(
    /from ["'](\/node_modules\/\.vite\/deps\/vue\.js[^"']*)["']/,
  )?.[1];
  assert(vue);
  await page.addInitScript(() => {
    window.isTauri = true;
    window.searchCalls = [];
    window.cancelCalls = [];
    window.pendingSearches = [];
    window.__TAURI_INTERNALS__ = {
      transformCallback: () => 1,
      unregisterCallback: () => {},
      convertFileSrc: (path) => path,
      invoke: async (command, args = {}) => {
        const request = args.request;
        const ok = (data) => ({
          success: true,
          error_code: null,
          message: '隔离测试',
          data,
          request_id: request.request_id,
        });
        if (command === 'cancel_metadata_search') {
          window.cancelCalls.push(request.payload.search_request_id);
          return ok(true);
        }
        if (command === 'search_metadata') {
          window.searchCalls.push(request);
          return new Promise((resolve) =>
            window.pendingSearches.push({
              id: request.request_id,
              finish: (title, failed = false) =>
                resolve(
                  failed
                    ? {
                        success: false,
                        error_code: 'NETWORK_UNAVAILABLE',
                        message: title,
                        data: null,
                        request_id: request.request_id,
                      }
                    : ok([
                        {
                          provider: request.payload.providers[0],
                          remote_id: '123',
                          title,
                          subtitle: null,
                          cover_url: null,
                          confidence: 1,
                          matched_fields: ['title'],
                          explanation: '',
                          fetched_at: '',
                          cached: false,
                        },
                      ]),
                ),
            }),
          );
        }
        if (command === 'confirm_metadata_match')
          throw Error('unexpected binding');
        if (command === 'get_home_summary') return ok({ total_games: 3 });
        throw Error('unexpected command ' + command);
      },
    };
  });
  await page.route('**/__metadata_search_test', (route) =>
    route.fulfill({
      contentType: 'text/html',
      body: `<!doctype html><html><head><meta charset="utf-8"></head><body><div id="app"></div><script type="module">
    import { createApp, h, ref, nextTick } from '${vue}';
    import MetadataMatch from '/src/components/MetadataMatch.vue';
    import { api } from '/src/stores/library.ts';
    import '/src/style.css'; import '/src/appearance.css';
    const show = ref(true), game = ref('first'), localResult = ref('');
    window.testLeave = () => { show.value = false; };
    window.testReturn = async () => { show.value = true; await nextTick(); };
    window.testGame = (id) => { game.value = id; };
    window.testLocal = async () => { localResult.value = String((await api('get_home_summary', {})).total_games); };
    createApp({ render: () => h('main', {style: 'padding:40px'}, [
      show.value ? h(MetadataMatch, { gameId: game.value, title: '离页测试作品' }) : h('p', '其他页面'),
      h('output', localResult.value),
    ]) }).mount('#app');
  </script></body></html>`,
    }),
  );
  await page.goto(`${base}/__metadata_search_test`);
  async function open() {
    await page
      .getByRole('button', { name: '搜索资料与封面', exact: true })
      .click();
    await page.locator('dialog[open]').waitFor();
  }
  async function choose(name, count) {
    await page.getByRole('button', { name, exact: true }).click();
    await page.waitForFunction(
      (count) => window.searchCalls.length === count,
      count,
    );
  }
  await open();
  await choose('Bangumi', 1);
  await choose('Hikarinagi', 2);
  await choose('VNDB', 3);
  await page.waitForFunction(() => window.cancelCalls.length === 2);
  assert.deepEqual(
    await page.evaluate(() => window.cancelCalls),
    await page.evaluate(() =>
      window.searchCalls.slice(0, 2).map((r) => r.request_id),
    ),
  );
  passed.push('换源取消前一请求，三个来源使用同一取消通道');
  // Close/reopen in the same JS turn, before the native close event arrives.
  await page.evaluate(() => {
    document.querySelector('[aria-label="关闭资料搜索"]').click();
    [...document.querySelectorAll('button')]
      .find((e) => e.textContent.trim() === '搜索资料与封面')
      .click();
  });
  await page.locator('dialog[open]').waitFor();
  await choose('Bangumi', 4);
  await page.waitForFunction(() => window.cancelCalls.length === 3);
  await page.evaluate(() => {
    window.pendingSearches[0].finish('旧结果不得出现');
    window.pendingSearches[1].finish('旧失败不得出现', true);
    window.pendingSearches[2].finish('另一个旧结果');
  });
  await page.waitForTimeout(100);
  assert.equal(
    await page.getByText('正在搜索 bangumi…', { exact: true }).count(),
    1,
  );
  assert.equal(await page.locator('.metadata-candidates li').count(), 0);
  assert.equal(await page.locator('.metadata-error').count(), 0);
  await page.evaluate(() => window.pendingSearches[3].finish('当前候选'));
  await page.getByText('当前候选', { exact: true }).waitFor();
  passed.push('关闭重开不被旧 close 事件干扰，迟到结果与错误丢弃');
  await choose('Hikarinagi', 5);
  await page.keyboard.press('Escape');
  await page.locator('dialog[open]').waitFor({ state: 'hidden' });
  await page.waitForFunction(() => window.cancelCalls.length === 4);
  passed.push('Escape 取消进行中的搜索');
  await open();
  await choose('VNDB', 6);
  await page.evaluate(() => window.testLeave());
  await page.getByText('其他页面', { exact: true }).waitFor();
  await page.waitForFunction(() => window.cancelCalls.length === 5);
  assert.equal(await page.locator('dialog').count(), 0);
  await page.evaluate(() => window.testLocal());
  await page.locator('output').filter({ hasText: '3' }).waitFor();
  await page.evaluate(() => {
    window.pendingSearches[4].finish('离页后的旧失败', true);
    window.pendingSearches[5].finish('离页后的候选');
  });
  await page.evaluate(() => window.testReturn());
  await open();
  assert.equal(await page.locator('.metadata-candidates li').count(), 0);
  assert.equal(await page.locator('.metadata-error').count(), 0);
  assert.equal(await page.locator('[role="status"]').count(), 0);
  passed.push('搜索未完成离页发送取消，清除弹窗且其他页面请求可完成');
  await choose('Bangumi', 7);
  await page.evaluate(() => window.testGame('second'));
  await page.locator('dialog[open]').waitFor({ state: 'hidden' });
  await page.waitForFunction(() => window.cancelCalls.length === 6);
  assert.equal(
    await page.evaluate(
      () => window.searchCalls.filter((r) => r.payload.manual !== true).length,
    ),
    0,
  );
  assert.deepEqual(errors, []);
  passed.push('切换作品取消旧搜索，未触发确认绑定');
  await open();
  await page.screenshot({ path: resolve(evidence, 'returned-search.png') });
  writeFileSync(
    resolve(evidence, 'result.json'),
    JSON.stringify(
      {
        passed,
        errors,
        environment: 'macOS Edge, isolated IPC; not native Windows acceptance',
      },
      null,
      2,
    ),
  );
  console.log(`详情搜索回归：${passed.length} 组通过。证据：${evidence}`);
} catch (error) {
  writeFileSync(
    resolve(evidence, 'failure.json'),
    JSON.stringify({ passed, errors, error: error.message }, null, 2),
  );
  await page.screenshot({ path: resolve(evidence, 'failure.png') });
  throw error;
} finally {
  await browser.close();
}
