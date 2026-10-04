import { spawnSync } from 'node:child_process';
let failed = false;
for (const [command, args] of [
  ['node', ['--version']],
  ['pnpm', ['--version']],
  ['node', ['scripts/rust.mjs', '--version']],
]) {
  const result = spawnSync(command, args, {
    stdio: 'inherit',
    shell: process.platform === 'win32',
  });
  if (result.error || result.status !== 0) {
    console.error(command + ' 检查失败');
    failed = true;
  }
}
console.log('平台：' + process.platform + ' / ' + process.arch);
console.log(
  'Windows C++ Build Tools / SDK / WebView2 仍需按 README 在 Windows 验证。',
);
process.exit(failed ? 1 : 0);
