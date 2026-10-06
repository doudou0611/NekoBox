// Isolated UI fixtures: no real credentials, database access, or account writes.
import assert from 'node:assert/strict';
import { mkdir, writeFile } from 'node:fs/promises';
import { chromium } from '../.tools/browser-check/node_modules/playwright/index.mjs';

const base = process.env.PREVIEW_URL || 'http://127.0.0.1:1420';
assert(['127.0.0.1', 'localhost'].includes(new URL(base).hostname));
const evidence = '.tools/account-cards-evidence';
await mkdir(evidence, { recursive: true });
const browser = await chromium.launch({
  headless: true,
  executablePath:
    process.env.BROWSER_EXECUTABLE ||
    '/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge',
});
const results = [];
try {
  const page = await browser.newPage({
    viewport: { width: 1280, height: 1000 },
  });
  const errors = [];
  page.on('pageerror', (error) => errors.push(error.message));
  const source = await (
    await page.request.get(`${base}/src/components/AccountDialog.vue`)
  ).text();
  const vue = source.match(/from "([^"]*\/vue\.js[^"]*)"/)?.[1];
  assert(vue);
  const storePath = (name) =>
    source.match(new RegExp(`from "(/src/stores/${name}\\.ts[^"]*)"`))?.[1];
  const bangumiStore = storePath('bangumi');
  const hikariStore = storePath('hikarinagiAccount');
  const fieldStore = storePath('hikariField');
  assert(bangumiStore && hikariStore && fieldStore);
  await page.addInitScript(() => {
    window.isTauri = true;
    window.requests = [];
    window.__TAURI_INTERNALS__ = {
      invoke: async (command, args) => {
        if (command.startsWith('plugin:event|')) return 1;
        const request = args.request;
        window.requests.push({ command, payload: request.payload });
        const signedOut = {
          status: 'signed_out',
          profile: null,
          message: '尚未登录。',
        };
        let data;
        if (['hikarinagi_account', 'hikarifield_account'].includes(command))
          data = signedOut;
        else if (command === 'get_hikarifield_settings')
          data = { root: null, uuid: '' };
        else if (command === 'login_bangumi')
          data = {
            status: 'authenticated',
            profile: {
              id: 14,
              username: 'fixture-bgm',
              nickname: '测试 Bangumi',
              avatar_url: null,
            },
            message: '已登录 Bangumi。',
          };
        else if (command === 'login_hikarifield') {
          await new Promise((resolve) => {
            window.rejectFieldLogin = resolve;
          });
          throw new Error('隔离测试：账户验证失败，请重试。');
        } else throw new Error(`Unexpected card fixture command: ${command}`);
        return {
          success: true,
          request_id: request.request_id,
          error_code: null,
          message: 'fixture',
          data,
        };
      },
    };
  });
  await page.route('**/__account_cards_test', (route) =>
    route.fulfill({
      contentType: 'text/html',
      body: `<html data-theme="light" data-motion="full"><head><meta charset="utf-8"></head><body><main id="fixture"></main><script type="module">
    import {createApp,h} from '${vue}';
    import AccountDialog from '/src/components/AccountDialog.vue';
    import {bangumi} from '${bangumiStore}';
    import {hikariAccount} from '${hikariStore}';
    import {hikariField} from '${fieldStore}';
    import '/src/style.css'; import '/src/appearance.css';
    window.fixtures = {bangumi,hikarinagi:hikariAccount,hikarifield:hikariField};
    window.setAccountState = (state) => {
      for (const [provider,store] of Object.entries(window.fixtures)) {
        store.error=''; store.busy=false;
        if (provider==='hikarifield') store.sync_message=state==='authenticated'?'已同步 12 部已拥有的游戏，新增 12 部。':'';
        store.account={status:state,message:state==='authenticated'?'账户已连接，记录将随故事一起珍藏。':'尚未登录。',profile:state==='signed_out'?null:provider==='hikarifield'?{name:'测试 HIKARI FIELD 用户',email:'fixture@example.invalid'}:{id:14,username:'fixture-user',nickname:provider==='bangumi'?'测试 Bangumi 用户':'测试 Hikarinagi 用户',avatar_url:null}};
      }
    };
    createApp({render:()=>h(AccountDialog)}).mount('#fixture');
    </script></body></html>`,
    }),
  );
  await page.route('https://**/*', (route) => route.abort());
  await page.goto(`${base}/__account_cards_test`);
  const dialog = page.locator('dialog.account-dialog[open]');
  await dialog.waitFor();
  const settle = async (provider) => {
    await page.locator(`#account-panel-${provider} .provider-intro`).waitFor();
    await page.waitForFunction(() => {
      const card = document.querySelector('.account-card');
      const stage = document.querySelector('.account-card-stage');
      return (
        card &&
        stage &&
        !document.querySelector(
          '.account-card-enter-active,.account-card-leave-active',
        ) &&
        Math.abs(
          card.getBoundingClientRect().height -
            stage.getBoundingClientRect().height,
        ) < 2
      );
    });
  };
  const providers = {
    bangumi: 'Bangumi',
    hikarinagi: 'Hikarinagi',
    hikarifield: 'HIKARI FIELD',
  };
  for (const state of ['signed_out', 'authenticated']) {
    await page.evaluate((state) => window.setAccountState(state), state);
    for (const theme of ['light', 'dark']) {
      await page.evaluate(
        (theme) => (document.documentElement.dataset.theme = theme),
        theme,
      );
      for (const width of [1280, 800, 390]) {
        await page.setViewportSize({ width, height: 1000 });
        const sharedStyles = [];
        for (const [provider, label] of Object.entries(providers)) {
          await page.getByRole('tab', { name: label, exact: true }).click();
          await settle(provider);
          await page.waitForFunction(() => {
            const logo = document.querySelector('.provider-logo');
            return logo?.complete && logo.naturalWidth > 0;
          });
          const logo = await page
            .locator('.provider-logo')
            .evaluate((node) => ({
              src: new URL(node.src).pathname,
              origin: new URL(node.src).origin,
              alt: node.alt,
            }));
          assert.equal(
            logo.src,
            `/brand/providers/${provider}.${provider === 'hikarifield' ? 'svg' : 'png'}`,
          );
          assert.equal(logo.origin, new URL(base).origin);
          assert.equal(logo.alt, `${label} 官方图标`);
          const layout = await dialog.evaluate((node) => {
            const header = node.querySelector('.provider-intro');
            const emblem = node.querySelector('.provider-emblem');
            const profile = node.querySelector('.provider-profile');
            const title = header.querySelector('h3');
            const style = getComputedStyle(header);
            const card = node
              .querySelector('.account-card')
              .getBoundingClientRect();
            const stage = node
              .querySelector('.account-card-stage')
              .getBoundingClientRect();
            return {
              overflow: node.scrollWidth > node.clientWidth + 1,
              outerOverflow:
                document.documentElement.scrollWidth > window.innerWidth,
              heightDifference: Math.abs(card.height - stage.height),
              headerGap: style.gap,
              emblemWidth: emblem.getBoundingClientRect().width,
              titleSize: getComputedStyle(title).fontSize,
              profileRadius: profile
                ? getComputedStyle(profile).borderRadius
                : null,
              profileAvatarWidth: profile
                ? profile
                    .querySelector('.provider-avatar')
                    .getBoundingClientRect().width
                : null,
            };
          });
          assert(
            !layout.overflow && !layout.outerOverflow,
            `${state} ${theme} ${width} ${provider} overflow`,
          );
          assert(layout.heightDifference < 2);
          sharedStyles.push([
            layout.headerGap,
            layout.emblemWidth,
            layout.titleSize,
            layout.profileRadius,
            layout.profileAvatarWidth,
          ]);
          results.push({ state, theme, width, provider, ...layout });
          if (width === 1280 || (width === 390 && theme === 'light'))
            await dialog.screenshot({
              path: `${evidence}/${state}-${theme}-${width}-${provider}.png`,
            });
        }
        assert.deepEqual(sharedStyles[0], sharedStyles[1]);
        assert.deepEqual(sharedStyles[1], sharedStyles[2]);
      }
    }
  }
  // The dialog interpolates between different card heights instead of jumping in one frame.
  const startingHeight = await page
    .locator('.account-card-stage')
    .evaluate((node) => node.getBoundingClientRect().height);
  await page.getByRole('tab', { name: 'Bangumi', exact: true }).click();
  const heightSamples = await page.evaluate(
    () =>
      new Promise((resolve) => {
        const samples = [];
        const start = performance.now();
        function sample() {
          samples.push(
            document
              .querySelector('.account-card-stage')
              .getBoundingClientRect().height,
          );
          if (performance.now() - start > 750) resolve(samples);
          else requestAnimationFrame(sample);
        }
        requestAnimationFrame(sample);
      }),
  );
  await settle('bangumi');
  const endingHeight = heightSamples.at(-1);
  assert(Math.abs(endingHeight - startingHeight) > 10);
  assert(
    heightSamples.some(
      (height) =>
        height > Math.min(startingHeight, endingHeight) + 2 &&
        height < Math.max(startingHeight, endingHeight) - 2,
    ),
  );
  // Downloads retain the logout lock and readable spacing around their explanation.
  await page.getByRole('tab', { name: 'HIKARI FIELD', exact: true }).click();
  await settle('hikarifield');
  await page.evaluate(
    () => (window.fixtures.hikarifield.tasks = [{ status: 'running' }]),
  );
  assert(
    await page
      .getByRole('button', { name: '退出登录', exact: true })
      .isDisabled(),
  );
  await page
    .getByText('下载完成或取消后可退出账户。', { exact: true })
    .waitFor();
  await page.evaluate(() => (window.fixtures.hikarifield.tasks = []));
  // Expired authorization still exposes reauthorization without falsely showing a connected check.
  await page.evaluate(() => window.setAccountState('expired'));
  await page.getByRole('tab', { name: 'Bangumi', exact: true }).click();
  await settle('bangumi');
  assert.equal(await page.getByLabel('需要重新授权').count(), 1);
  assert.equal(
    await page.getByLabel('Access Token', { exact: true }).count(),
    1,
  );
  // Token submission clears the sensitive input and renders the shared authenticated profile.
  await page.evaluate(() => window.setAccountState('signed_out'));
  await page
    .getByLabel('Access Token', { exact: true })
    .fill('isolated-fixture-token');
  await page
    .getByRole('button', { name: '登录并保存授权', exact: true })
    .click();
  await page.getByText('测试 Bangumi', { exact: true }).waitFor();
  assert.equal(
    await page.getByLabel('Access Token', { exact: true }).count(),
    0,
  );
  assert.equal(
    await page.evaluate(
      () =>
        window.requests.find((r) => r.command === 'login_bangumi').payload
          .access_token,
    ),
    'isolated-fixture-token',
  );
  // HIKARI FIELD preserves password visibility controls and clears the input before awaiting login.
  await page.getByRole('tab', { name: 'HIKARI FIELD', exact: true }).click();
  await settle('hikarifield');
  await page
    .getByLabel('邮箱', { exact: true })
    .fill('fixture@example.invalid');
  await page
    .getByLabel('密码', { exact: true })
    .fill('isolated-fixture-password');
  await page.getByRole('button', { name: '显示密码', exact: true }).click();
  assert.equal(
    await page.getByLabel('密码', { exact: true }).getAttribute('type'),
    'text',
  );
  await page
    .getByRole('button', { name: '登录并导入已购游戏', exact: true })
    .click();
  await page.waitForFunction(() => window.rejectFieldLogin);
  assert.equal(await page.getByLabel('密码', { exact: true }).inputValue(), '');
  assert.equal(
    await page.getByLabel('密码', { exact: true }).getAttribute('type'),
    'password',
  );
  await page.evaluate(() => window.rejectFieldLogin());
  await page.getByRole('alert').waitFor();
  await settle('hikarifield');
  const feedbackGap = await page.evaluate(
    () =>
      document.querySelector('.provider-feedback').getBoundingClientRect().top -
      document.querySelector('.provider-form').getBoundingClientRect().bottom,
  );
  assert(Math.abs(feedbackGap - 20) < 0.1);
  // Rapid selections settle on the final provider, keep one panel, and retain tab focus.
  await page.evaluate(async () => {
    for (const provider of [
      'bangumi',
      'hikarifield',
      'hikarinagi',
      'bangumi',
      'hikarifield',
    ]) {
      document.getElementById(`account-tab-${provider}`).click();
      await new Promise(requestAnimationFrame);
    }
    document.getElementById('account-tab-hikarifield').focus();
  });
  await settle('hikarifield');
  assert.equal(await page.getByRole('tabpanel').count(), 1);
  assert.equal(
    await page.evaluate(() => document.activeElement.id),
    'account-tab-hikarifield',
  );
  // Both application and system reduced-motion settings remove height/slide/open animations.
  for (const mode of ['light', 'reduced', 'system-reduced']) {
    await page.emulateMedia({
      reducedMotion: mode === 'system-reduced' ? 'reduce' : 'no-preference',
    });
    await page.evaluate(
      (mode) =>
        (document.documentElement.dataset.motion =
          mode === 'system-reduced' ? 'full' : mode),
      mode,
    );
    await page.getByRole('tab', { name: 'Bangumi', exact: true }).focus();
    await page.keyboard.press('Home');
    await page.keyboard.press('ArrowRight');
    await settle('hikarinagi');
    if (mode !== 'light') {
      const motion = await page.evaluate(() => ({
        height: getComputedStyle(document.querySelector('.account-card-stage'))
          .transitionDuration,
        slide: getComputedStyle(
          document.querySelector('.account-tab-indicator'),
        ).transitionDuration,
        open: getComputedStyle(document.querySelector('.account-dialog'))
          .animationName,
      }));
      assert.deepEqual(motion, { height: '0s', slide: '0s', open: 'none' });
    }
    for (let i = 0; i < 10; i++) {
      await page.keyboard.press('Tab');
      assert(
        await dialog.evaluate((node) => node.contains(document.activeElement)),
      );
    }
  }
  assert.deepEqual(errors, []);
  await writeFile(
    `${evidence}/checks.json`,
    JSON.stringify(
      {
        layouts: results,
        errors,
        expired: true,
        tokenLogin: true,
        passwordCleared: true,
        feedbackGap,
        rapidSwitch: true,
        reducedMotion: true,
        heightInterpolates: true,
        downloadLogoutLock: true,
        officialLogosLoadOffline: true,
      },
      null,
      2,
    ),
  );
  console.log(
    `账户卡片验证：${results.length} 组布局、授权过期、隔离登录、密码清空、快速切换及动画模式通过。截图：${evidence}`,
  );
} finally {
  await browser.close();
}
