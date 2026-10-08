// Isolated Vue/IPC workflow regression. No production DB or remote API writes.
import assert from 'node:assert/strict';
import { mkdirSync } from 'node:fs';
import { resolve } from 'node:path';
const evidence = resolve('.tools/single-import-evidence');
mkdirSync(evidence, { recursive: true });
import { chromium } from '../.tools/browser-check/node_modules/playwright/index.mjs';
const base = process.env.PREVIEW_URL || 'http://127.0.0.1:1420';
assert(['127.0.0.1', 'localhost'].includes(new URL(base).hostname));
const browser = await chromium.launch({
  headless: true,
  executablePath:
    process.env.BROWSER_EXECUTABLE ||
    '/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge',
});
let checks = 0;
try {
  for (const scenario of [
    'manual',
    'manual-disabled',
    'manual-fallback',
    'automatic',
    'cancel',
    'settings',
    'duplicate-mixed',
    'duplicate-all',
    'duplicate-partial',
    'single',
    'single-double-click',
    'single-failure',
    'single-cancel',
  ]) {
    const page = await browser.newPage({
      viewport: { width: 1280, height: 1100 },
    });
    const errors = [];
    page.on('pageerror', (e) => errors.push(e.message));
    const source = await (
      await page.request.get(`${base}/src/components/LocalImport.vue`)
    ).text();
    const modulePath = (path) =>
      source.match(new RegExp(`from "(${path}[^"]*)"`))?.[1];
    const vue = modulePath('/node_modules/\\.vite/deps/vue\\.js');
    const library = modulePath('/src/stores/library\\.ts');
    const operations = modulePath('/src/stores/operations\\.ts');
    assert(vue && library && operations);
    await page.addInitScript(
      ({ scenario }) => {
        window.isTauri = true;
        window.requests = [];
        window.pending = [];
        window.hikariSettings = {
          enabled: true,
          method: 'client_credentials',
          client_id: '',
          has_client_secret: false,
          has_access_token: false,
          can_search: true,
        };
        window.__TAURI_INTERNALS__ = {
          transformCallback: () => 1,
          unregisterCallback: () => {},
          convertFileSrc: (path) => `http://asset.localhost/${path}`,
          invoke: async (command, args = {}) => {
            if (command === 'plugin:event|listen') return 1;
            if (command === 'plugin:event|unlisten') return;
            if (command === 'plugin:dialog|open') {
              window.dialogOptions = args.options;
              return ['C:/fixture'];
            }
            const request = args.request;
            window.requests.push({ command, payload: request.payload });
            const success = (data) => ({
              success: true,
              error_code: null,
              message: '隔离测试',
              request_id: request.request_id,
              data,
            });
            const failure = () => ({
              success: false,
              error_code: 'NETWORK_UNAVAILABLE',
              message: 'Bangumi detail fixture failure',
              request_id: request.request_id,
              data: null,
            });
            let data;
            const sources = {
              sources: [
                { provider: 'bangumi', enabled: true },
                { provider: 'vndb', enabled: true },
                { provider: 'hikarinagi', enabled: false },
              ],
            };
            if (command === 'get_metadata_sources') return success(sources);
            if (command === 'save_metadata_sources')
              return success(request.payload);
            if (command === 'begin_import_batch')
              return success({ batch_id: 'fixture-batch', sources });
            if (
              command === 'cancel_import_batch' ||
              command === 'cancel_metadata_search'
            )
              return success(true);
            if (command === 'get_vndb_settings')
              data = { has_api_token: false };
            else if (command === 'save_vndb_settings')
              data = { has_api_token: true };
            else if (command === 'test_vndb_connection')
              data = { username: 'fixture-vndb', permissions: ['listread'] };
            else if (command === 'get_hikarinagi_settings')
              data = window.hikariSettings;
            else if (command === 'save_hikarinagi_settings') {
              const p = request.payload;
              window.hikariSettings = {
                enabled: p.enabled,
                method: p.method,
                client_id: p.client_id,
                has_client_secret: !!p.client_secret,
                has_access_token: !!p.access_token,
                can_search: p.enabled && !!p.client_id && !!p.client_secret,
              };
              data = window.hikariSettings;
            } else if (command === 'test_hikarinagi_connection') data = true;
            else if (command === 'preview_import') {
              data = {
                items: Array.from(
                  {
                    length: scenario.startsWith('duplicate')
                      ? 3
                      : scenario === 'automatic'
                        ? 2
                        : 1,
                  },
                  (_, index) => ({
                    directory: `C:/fixture/game${index}`,
                    folder_name: `中文作品${index}`,
                    search_name: `中文作品${index}`,
                    executables: scenario.startsWith('single')
                      ? ['game.exe', 'engine/game.exe'].map((name) => ({
                          path: `C:/fixture/game${index}/${name}`,
                          product_name: null,
                          file_description: null,
                          company_name: null,
                          fingerprint: 'fixture',
                          title_evidence: [],
                        }))
                      : [],
                    selected_executable: null,
                    existing_game_id:
                      scenario.startsWith('duplicate') &&
                      (scenario === 'duplicate-all' || index === 0)
                        ? `existing-${index}`
                        : null,
                    duplicate_reason:
                      scenario.startsWith('duplicate') &&
                      (scenario === 'duplicate-all' || index === 0)
                        ? '路径已在游戏库中'
                        : null,
                  }),
                ),
                issue_count: 0,
                issues: [],
                scanned_directories: 2,
                skipped_directories: 0,
              };
            } else if (command === 'search_metadata') {
              const provider = request.payload.providers[0];
              if (scenario === 'single-cancel')
                await new Promise((resolve) => {
                  window.finishSearch = resolve;
                });
              data = [
                {
                  provider,
                  remote_id: provider === 'vndb' ? 'v123' : '123',
                  title: request.payload.query,
                  subtitle: '原名',
                  cover_url: scenario.startsWith('single')
                    ? '/preview/garden.svg'
                    : 'https://example.invalid/fixture.png',
                  has_chinese_description: true,
                  confidence: 1,
                  matched_fields: ['title'],
                  explanation: '',
                  fetched_at: '',
                  cached: false,
                },
              ];
            } else if (command === 'prepare_import_metadata') {
              if (scenario === 'single-failure' && !window.failedOnce) {
                window.failedOnce = true;
                return failure();
              }
              if (
                (scenario === 'automatic' || scenario === 'manual-fallback') &&
                request.payload.provider === 'bangumi'
              )
                return failure();
              return new Promise((resolve) => {
                window.pending.push(() =>
                  resolve(
                    success({
                      preparation_id: request.payload.directory,
                      title: '中文作品',
                      subtitle: null,
                      cover_path: 'covers/fixture.png',
                      provider: request.payload.provider,
                      remote_id: request.payload.remote_id,
                      translation_message: null,
                    }),
                  ),
                );
              });
            } else if (command === 'discard_import_metadata') data = 1;
            else if (command === 'import_prepared_game') {
              if (
                scenario === 'duplicate-partial' &&
                request.payload.directory.endsWith('game2') &&
                !window.failedImportOnce
              ) {
                window.failedImportOnce = true;
                return failure();
              }
              data = { id: request.payload.directory };
            } else if (command === 'backend_status')
              data = {
                data_directory: 'C:/fixture',
                schema_version: 1,
                portable: true,
                last_scan_task_id: null,
              };
            else if (command === 'list_games')
              data = { items: [], total: 0, page: 1, page_size: 100 };
            else if (
              command === 'list_collections' ||
              command === 'get_recommendations'
            )
              data = [];
            else if (command === 'get_home_summary') data = {};
            else throw Error(`Unexpected isolated command ${command}`);
            return success(data);
          },
        };
      },
      { scenario },
    );
    await page.route('https://example.invalid/**', (route) => route.abort());
    await page.route('**/__import_workflow_test', (route) =>
      route.fulfill({
        contentType: 'text/html',
        body: `<html><head><meta charset="utf-8"></head><body><div id="test-root"></div><script type="module">
      import { createApp, h, nextTick } from '${vue}';
      import LocalImport from '/src/components/LocalImport.vue';
      import OperationCenter from '/src/components/OperationCenter.vue';
      import SourceSettings from '/src/components/MetadataSettings.vue';
      import { local } from '${library}';
      import { operations } from '${operations}';
      import '/src/style.css'; import '/src/appearance.css';
      window.testLocal=local; window.testOperations=operations;
      createApp({render:()=>h('main',{},${scenario === 'settings' ? '[h(SourceSettings)]' : '[h(OperationCenter),h(LocalImport)]'})}).mount('#test-root');
      await nextTick(); ${scenario === 'settings' ? '' : 'local.import_open=true;'}
    </script></body></html>`,
      }),
    );
    await page.goto(`${base}/__import_workflow_test`);
    if (scenario === 'settings') {
      await page.getByLabel('VNDB API Token').fill('fixture-private-token');
      await page.getByRole('button', { name: '保存并测试 VNDB' }).click();
      await page
        .getByRole('status')
        .filter({ hasText: 'VNDB 授权有效' })
        .waitFor();
      assert.equal(await page.getByLabel('VNDB API Token').inputValue(), '');
      assert.equal(
        await page.getByLabel('Hikarinagi Client Secret').count(),
        0,
      );
      assert.deepEqual(
        await page.evaluate(() =>
          window.requests
            .map((r) => r.command)
            .filter((c) => !c.includes('metadata_sources')),
        ),
        ['get_vndb_settings', 'save_vndb_settings', 'test_vndb_connection'],
      );
      checks += 3;
    } else if (scenario.startsWith('single')) {
      await page.getByRole('button', { name: /导入单个游戏/ }).click();
      const single = page.getByRole('dialog', {
        name: '导入单个游戏',
        exact: true,
      });
      await single.waitFor();
      assert.equal(await page.locator('.review-card:visible').count(), 0);
      assert.equal(await page.locator('dialog[open]').count(), 1);
      assert.equal(
        await single.getByLabel('单个游戏刮削源').inputValue(),
        'bangumi',
      );
      const picker = await page.evaluate(() => window.dialogOptions);
      assert.equal(picker.directory, true);
      assert.equal(picker.multiple, false);
      const preview = await page.evaluate(
        () =>
          window.requests.find((r) => r.command === 'preview_import').payload,
      );
      assert.equal(preview.single_directory, true);
      assert(!preview.single_executable);
      if (scenario === 'single-cancel') {
        await page.waitForFunction(
          () => typeof window.finishSearch === 'function',
        );
        await single.getByRole('button', { name: '取消', exact: true }).click();
        await single.waitFor({ state: 'hidden' });
        await page.evaluate(() => window.finishSearch());
        await page.waitForTimeout(100);
        const requests = await page.evaluate(() => window.requests);
        assert(requests.some((r) => r.command === 'cancel_import_batch'));
        assert(requests.some((r) => r.command === 'cancel_metadata_search'));
        assert(!requests.some((r) => r.command === 'import_prepared_game'));
        assert.equal(await page.locator('dialog[open]').count(), 0);
      } else {
        await single.locator('.single-candidate').first().waitFor();
        await single.evaluate(async (element) => {
          await Promise.all(
            element.getAnimations().map((animation) => animation.finished),
          );
        });
        const box = await single.boundingBox();
        assert(Math.abs(box.x + box.width / 2 - 640) < 2);
        assert(Math.abs(box.y + box.height / 2 - 550) < 2);
        await single
          .locator('.single-candidate img')
          .evaluate((img) => img.decode());
        await single.screenshot({ path: resolve(evidence, `${scenario}.png`) });
        assert.equal(
          await single.locator('.single-candidate .preview-cover').count(),
          1,
        );
        await single.getByLabel('单个游戏刮削源').selectOption('vndb');
        await single.locator('.single-candidate').first().waitFor();
        await single.locator('.single-candidate').first().click();
        assert.equal(
          await single
            .locator('.single-candidate')
            .first()
            .getAttribute('aria-pressed'),
          'true',
        );
        assert.equal(
          await page.evaluate(
            () =>
              window.requests.filter(
                (request) => request.command === 'prepare_import_metadata',
              ).length,
          ),
          0,
          'single click must only select the candidate',
        );
        if (scenario === 'single-double-click')
          await single.locator('.single-candidate').first().dblclick();
        else
          await single.getByRole('button', { name: '确认刮削并导入' }).click();
        await single
          .getByRole('alert')
          .filter({ hasText: '启动入口' })
          .waitFor();
        assert.equal(
          await page.evaluate(
            () =>
              window.requests.filter(
                (request) => request.command === 'prepare_import_metadata',
              ).length,
          ),
          0,
          'missing executable must block scraping',
        );
        await single
          .locator('.single-launch select')
          .selectOption('C:/fixture/game0/game.exe');
        if (scenario === 'single-double-click')
          await single.locator('.single-candidate').first().dblclick();
        else
          await single.getByRole('button', { name: '确认刮削并导入' }).click();
        if (scenario === 'single-failure') {
          await single
            .getByRole('alert')
            .filter({ hasText: 'fixture failure' })
            .waitFor();
          assert.equal(
            await single
              .locator('.single-candidate[aria-pressed="true"]')
              .count(),
            1,
          );
          await single.getByRole('button', { name: '确认刮削并导入' }).click();
        }
        await page.waitForFunction(() => window.pending.length === 1);
        if (scenario === 'single-double-click') {
          // Repeated events while the first request is pending must not import twice.
          await single
            .locator('.single-candidate')
            .first()
            .dispatchEvent('dblclick');
          assert.equal(await page.evaluate(() => window.pending.length), 1);
        }
        await page.evaluate(() => window.pending.shift()());
        await single.waitFor({ state: 'hidden' });
        const requests = await page.evaluate(() => window.requests);
        const preparation = requests.filter(
          (r) => r.command === 'prepare_import_metadata',
        );
        if (scenario === 'single-double-click')
          assert.equal(preparation.length, 1);
        assert(
          preparation.every(
            (r) =>
              r.payload.provider === 'vndb' &&
              r.payload.single_source &&
              r.payload.manual,
          ),
        );
        assert(
          requests
            .filter((r) => r.command === 'search_metadata')
            .every((r) => r.payload.providers.length === 1),
        );
        assert.equal(
          requests.filter((r) => r.command === 'import_prepared_game').length,
          1,
        );
        assert(!requests.some((r) => r.command === 'save_metadata_sources'));
      }
      assert.deepEqual(errors, []);
      checks += 10;
    } else {
      await page.getByRole('button', { name: /导入游戏目录/ }).click();
      await page.locator('.review-card').first().waitFor();
      if (scenario.startsWith('duplicate')) {
        const cards = page.locator('.review-card');
        const stats = () =>
          Promise.all(
            [
              '.review-recognized-count',
              '.review-pending-count',
              '.review-scraped-count',
            ].map((selector) => page.locator(selector).innerText()),
          );
        const all = scenario === 'duplicate-all';
        assert.deepEqual(
          await stats(),
          all ? ['0', '0', '0'] : ['2', '2', '0'],
        );
        assert.equal(
          await page.locator('.review-imported-count').innerText(),
          all ? '已导入（3）' : '已导入（1）',
        );
        const imported = cards.filter({
          has: page.locator('.review-status[data-status="imported"]'),
        });
        assert.equal(await imported.count(), all ? 3 : 1);
        assert.equal(
          await imported.first().locator('.review-status').innerText(),
          '已导入',
        );
        assert(
          await imported.first().locator('input[type="checkbox"]').isDisabled(),
        );
        assert(
          await imported
            .first()
            .locator('.review-search-name input')
            .isDisabled(),
        );
        assert(
          await imported
            .first()
            .getByRole('button', { name: '手动匹配' })
            .isDisabled(),
        );
        const green = await imported.first().evaluate((el) => ({
          border: getComputedStyle(el).borderColor,
          success: getComputedStyle(document.documentElement)
            .getPropertyValue('--success')
            .trim(),
        }));
        const expected = await page.evaluate((color) => {
          const e = document.createElement('span');
          e.style.color = color;
          document.body.append(e);
          const c = getComputedStyle(e).color;
          e.remove();
          return c;
        }, green.success);
        await page.waitForTimeout(300);
        assert.equal(
          await imported
            .first()
            .evaluate((el) => getComputedStyle(el).borderColor),
          expected,
        );
        if (all) {
          for (const name of ['开始刮削', '后台运行', '确认导入'])
            assert(
              await page
                .getByRole('button', { name, exact: true })
                .isDisabled(),
            );
          assert(
            !(await page.evaluate(() => window.requests)).some((r) =>
              [
                'search_metadata',
                'prepare_import_metadata',
                'import_prepared_game',
                'begin_import_batch',
              ].includes(r.command),
            ),
          );
        } else {
          await page
            .getByRole('button', { name: '开始刮削', exact: true })
            .click();
          for (let i = 0; i < 2; i++) {
            await page.waitForFunction(() => window.pending.length === 1);
            await page.evaluate(() => window.pending.shift()());
          }
          await page.waitForFunction(
            () => window.testOperations[0].status === 'completed',
          );
          assert.deepEqual(await stats(), ['2', '0', '2']);
          const calls = await page.evaluate(() => window.requests);
          assert.deepEqual(
            calls
              .filter((r) => r.command === 'prepare_import_metadata')
              .map((r) => r.payload.directory),
            ['C:/fixture/game1', 'C:/fixture/game2'],
          );
          assert.deepEqual(
            calls
              .filter((r) => r.command === 'search_metadata')
              .map((r) => r.payload.query),
            ['中文作品1', '中文作品2'],
          );
          assert.equal(
            await page.evaluate(() => window.testOperations[0].total),
            2,
          );
          await page.waitForTimeout(300);
          assert.equal(
            await imported
              .first()
              .evaluate((el) => getComputedStyle(el).borderColor),
            await cards
              .nth(1)
              .evaluate((el) => getComputedStyle(el).borderColor),
          );
          if (scenario === 'duplicate-mixed')
            await page
              .locator('.local-import[open]')
              .screenshot({ path: resolve(evidence, 'duplicate-mixed.png') });
          await page
            .getByRole('button', { name: '确认导入', exact: true })
            .click();
          if (scenario === 'duplicate-partial') {
            await page.waitForFunction(() =>
              window.testOperations.some(
                (o) => o.kind === 'import' && o.status === 'failed',
              ),
            );
            assert.equal(
              await page.locator('.review-imported-count').innerText(),
              '已导入（2）',
            );
            assert.deepEqual(await stats(), ['1', '1', '0']);
            assert.equal(
              await page
                .locator('.review-status[data-status="imported"]')
                .count(),
              2,
            );
            await page
              .getByRole('button', { name: '确认导入', exact: true })
              .click();
          }
          await page.waitForFunction(() => !window.testLocal.import_open);
          const commits = await page.evaluate(() =>
            window.requests
              .filter((r) => r.command === 'import_prepared_game')
              .map((r) => r.payload.directory),
          );
          assert.deepEqual(
            commits,
            scenario === 'duplicate-partial'
              ? ['C:/fixture/game1', 'C:/fixture/game2', 'C:/fixture/game2']
              : ['C:/fixture/game1', 'C:/fixture/game2'],
          );
        }
        assert.deepEqual(errors, []);
        checks += 15;
        await page.close();
        continue;
      }
      assert.equal(await page.locator('.review-imported-count').count(), 0);
      if (scenario.startsWith('manual')) {
        await page.getByRole('button', { name: '手动匹配' }).click();
        assert.equal(
          await page.locator('option[value="hikarinagi"]').isDisabled(),
          false,
        );
        assert.equal(await page.locator('dialog[open]').count(), 2);
        assert.equal(
          await page
            .locator(
              '.local-import:not(.manual-match-dialog) .manual-match-panel',
            )
            .count(),
          0,
        );
        await page
          .locator('select')
          .filter({ has: page.locator('option[value="bangumi"]') })
          .selectOption(
            scenario === 'manual-disabled' ? 'hikarinagi' : 'bangumi',
          );
        await page.getByRole('button', { name: '搜索', exact: true }).click();
        await page.getByRole('button', { name: '使用', exact: true }).click();
        await page.getByText('已匹配 · 待刮削', { exact: true }).waitFor();
        assert.equal(
          await page.evaluate(
            () =>
              window.requests.filter(
                (r) => r.command === 'prepare_import_metadata',
              ).length,
          ),
          0,
        );
        await page
          .getByRole('button', { name: '确认导入', exact: true })
          .click();
        await page
          .getByRole('alert')
          .filter({ hasText: '先点击“开始刮削”' })
          .waitFor();
        assert.equal(
          await page.evaluate(
            () =>
              window.requests.filter(
                (r) => r.command === 'import_prepared_game',
              ).length,
          ),
          0,
        );
        checks += 5;
      }
      await page.getByRole('button', { name: '开始刮削', exact: true }).click();
      await page.waitForFunction(() => window.pending.length === 1);
      assert.equal(await page.locator('.import-error').count(), 0);
      const buttons = await page
        .locator('.dialog-actions-right button')
        .evaluateAll((nodes) =>
          nodes.map((n) => ({
            text: n.textContent.trim(),
            class: n.className,
            left: n.getBoundingClientRect().left,
          })),
        );
      assert.equal(buttons[0].text, '后台运行');
      assert.equal(buttons[1].text, '开始刮削');
      assert.equal(buttons[0].class, buttons[1].class);
      if (scenario === 'cancel') {
        assert(
          await page
            .getByRole('button', { name: '取消', exact: true })
            .isEnabled(),
        );
        await page.getByRole('button', { name: '取消', exact: true }).click();
        await page.waitForFunction(
          () => window.testOperations[0].status === 'cancelled',
        );
        assert.equal(await page.locator('dialog[open]').count(), 0);
        await page.evaluate(() => window.pending.shift()());
        await page.waitForTimeout(200);
        assert.equal(
          await page.evaluate(() => window.testOperations[0].status),
          'cancelled',
        );
        assert.equal(
          await page.evaluate(
            () =>
              window.requests.filter(
                (r) => r.command === 'import_prepared_game',
              ).length,
          ),
          0,
        );
        assert(
          await page.evaluate(() =>
            window.requests.some((r) => r.command === 'cancel_import_batch'),
          ),
        );
        assert.deepEqual(errors, []);
        checks += 7;
        await page.close();
        continue;
      }
      assert(await page.getByRole('button', { name: '后台运行' }).isEnabled());
      await page.getByRole('button', { name: '后台运行' }).click();
      assert.equal(await page.locator('dialog[open]').count(), 0);
      await page.getByRole('button', { name: '打开后台任务' }).click();
      await page.getByRole('progressbar').waitFor();
      assert.equal(
        await page.getByRole('progressbar').getAttribute('aria-valuenow'),
        '0',
      );
      assert.equal(await page.locator('.operation-popover li').count(), 1);
      await page.getByRole('button', { name: '查看刮削结果' }).click();
      await page.locator('dialog[open]').waitFor();
      assert(await page.getByRole('button', { name: '开始刮削' }).isDisabled());
      await page.evaluate(() => window.pending.shift()());
      if (scenario === 'automatic') {
        await page.waitForFunction(
          () =>
            window.pending.length === 1 &&
            window.testOperations[0].progress === 1,
        );
        await page.evaluate(() => window.pending.shift()());
      }
      await page.waitForFunction(
        () => window.testOperations[0].status === 'completed',
      );
      const calls = await page.evaluate(() => window.requests);
      const providers = calls
        .filter((r) => r.command === 'prepare_import_metadata')
        .map((r) => r.payload.provider);
      assert.deepEqual(
        providers,
        scenario === 'manual-disabled'
          ? ['hikarinagi']
          : scenario === 'manual'
            ? ['bangumi']
            : scenario === 'manual-fallback'
              ? ['bangumi', 'vndb']
              : ['bangumi', 'vndb', 'bangumi', 'vndb'],
      );
      if (scenario !== 'manual-disabled')
        assert(
          !calls.some((r) => r.payload?.providers?.includes('hikarinagi')),
        );
      if (scenario.startsWith('manual'))
        assert(
          calls.find((r) => r.command === 'prepare_import_metadata').payload
            .manual === true,
        );
      const count = providers.length;
      await page.getByRole('button', { name: '开始刮削', exact: true }).click();
      assert.equal(
        await page.evaluate(
          () =>
            window.requests.filter(
              (r) => r.command === 'prepare_import_metadata',
            ).length,
        ),
        count,
      );
      assert.equal(await page.evaluate(() => window.testOperations.length), 1);
      checks += 9;
    }
    assert.deepEqual(errors, []);
    await page.close();
  }
  console.log(
    `导入刮削浏览器回归：${checks} 项通过（隔离 IPC；非 Windows 真机验收）。`,
  );
} finally {
  await browser.close();
}
