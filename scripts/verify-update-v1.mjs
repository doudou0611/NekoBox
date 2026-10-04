// Isolated Chromium evidence; never a Windows runtime acceptance result.
import assert from 'node:assert/strict';
import { mkdir, writeFile } from 'node:fs/promises';
import { chromium } from '../.tools/browser-check/node_modules/playwright/index.mjs';
const base = process.env.PREVIEW_URL || 'http://127.0.0.1:1420';
assert(['127.0.0.1', 'localhost'].includes(new URL(base).hostname));
const browser = await chromium.launch({
  headless: true,
  executablePath:
    process.env.BROWSER_EXECUTABLE ||
    '/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge',
});
const evidence = '.tools/update-v1-evidence';
await mkdir(evidence, { recursive: true });
let checks = 0;
try {
  const page = await browser.newPage({
    viewport: { width: 1280, height: 900 },
  });
  const errors = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto(`${base}/#/settings`);
  await page.getByRole('button', { name: '黑色配色', exact: true }).waitFor();
  assert.equal(await page.locator('.preference-palette').count(), 10);
  checks++;
  assert.equal(
    await page
      .getByRole('heading', { name: '开发连通性', exact: true })
      .count(),
    0,
  );
  assert.equal(
    await page.getByRole('heading', { name: '推荐偏好', exact: true }).count(),
    0,
  );
  checks += 2;
  for (const name of ['黑色', '白色'])
    for (const mode of ['深色', '浅色']) {
      await page
        .getByRole('button', { name: `${name}配色`, exact: true })
        .click();
      await page
        .getByRole('radio', { name: `主题：${mode}展厅`, exact: true })
        .click();
      const state = await page.evaluate(() => ({
        palette: document.documentElement.dataset.palette,
        theme: document.documentElement.dataset.theme,
        ink: getComputedStyle(document.documentElement)
          .getPropertyValue('--accent-ink')
          .trim(),
        text: getComputedStyle(document.documentElement)
          .getPropertyValue('--text')
          .trim(),
        accent: getComputedStyle(document.documentElement)
          .getPropertyValue('--accent')
          .trim(),
      }));
      assert.equal(state.palette, name === '黑色' ? 'black' : 'white');
      assert.equal(state.ink, state.text);
      assert.equal(state.accent, name === '黑色' ? '#000000' : '#ffffff');
      checks += 3;
    }
  await page.reload();
  await page.getByRole('button', { name: '白色配色', exact: true }).waitFor();
  assert.equal(
    await page
      .getByRole('button', { name: '白色配色', exact: true })
      .getAttribute('aria-pressed'),
    'true',
  );
  checks++;
  const rows = page.locator('.metadata-source-order li');
  await rows.first().waitFor();
  const handle = rows.nth(0).locator('.drag-handle');
  await handle.scrollIntoViewIfNeeded();
  const start = await handle.boundingBox();
  const target = await rows.nth(2).boundingBox();
  assert(start && target);
  await page.mouse.move(start.x + start.width / 2, start.y + start.height / 2);
  await page.mouse.down();
  await page.mouse.move(
    target.x + target.width / 2,
    target.y + target.height / 2,
    { steps: 12 },
  );
  await page.mouse.up();
  await page.waitForFunction(() =>
    document
      .querySelector('.metadata-source-order li')
      .textContent.includes('Bangumi'),
  );
  assert.match(await rows.first().textContent(), /Bangumi/);
  assert.match(await rows.nth(2).textContent(), /Hikarinagi/);
  checks += 2;
  await rows.nth(0).getByRole('switch').click();
  await rows.nth(1).getByRole('switch').click();
  await rows.nth(2).getByRole('switch').click();
  assert.equal(
    await rows.nth(2).getByRole('switch').getAttribute('aria-checked'),
    'true',
  );
  await page.getByRole('alert').filter({ hasText: '至少启用一个' }).waitFor();
  checks += 2;
  await page.reload();
  await rows.first().waitFor();
  assert.match(await rows.first().textContent(), /Bangumi/);
  assert.equal(
    await rows.nth(2).getByRole('switch').getAttribute('aria-checked'),
    'true',
  );
  checks += 2;
  await page.screenshot({
    path: `${evidence}/settings-white.png`,
    fullPage: true,
  });
  // The former header blur contract is replaced by the sidebar tool dock.
  assert.equal(await page.locator('.app-topbar').count(), 0);
  assert.equal(await page.locator('.sidebar-dock .sidebar-tool').count(), 4);
  checks += 2;
  assert.deepEqual(errors, []);
  checks++;
  await writeFile(
    `${evidence}/result.json`,
    JSON.stringify(
      {
        checks,
        environment:
          'macOS Chromium; native material selectors simulated; Windows pending',
      },
      null,
      2,
    ),
  );
  console.log(
    `UpdateV1 外观与来源配置浏览器回归：${checks} 项通过；证据 ${evidence}`,
  );
} finally {
  await browser.close();
}
