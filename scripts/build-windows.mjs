import {
  existsSync,
  mkdirSync,
  symlinkSync,
  lstatSync,
  readlinkSync,
  unlinkSync,
} from 'node:fs';
import { delimiter, resolve } from 'node:path';
import { homedir } from 'node:os';
import { spawnSync } from 'node:child_process';

const root = resolve(import.meta.dirname, '..');
const cargoHome = resolve(root, '.tools/cargo');
const localCargo =
  process.platform !== 'win32' && existsSync(resolve(cargoHome, 'bin/cargo'));
const targets = {
  x64: 'x86_64-pc-windows-msvc',
  arm64: 'aarch64-pc-windows-msvc',
};
const requested = process.argv[2] ?? 'all';
const selectedTargets =
  requested === 'all'
    ? [targets.x64, targets.arm64]
    : targets[requested]
      ? [targets[requested]]
      : [];

if (selectedTargets.length === 0) {
  console.error('用法：node scripts/build-windows.mjs [x64|arm64|all]');
  process.exit(2);
}

const env = localCargo
  ? {
      ...process.env,
      CARGO_HOME: cargoHome,
      RUSTUP_HOME: resolve(root, '.tools/rustup'),
      XWIN_CACHE_DIR: resolve(root, '.tools/xwin'),
      PATH: resolve(cargoHome, 'bin') + delimiter + process.env.PATH,
    }
  : { ...process.env };
const defaultKey = resolve(homedir(), '.config/NekoBox/updater.key');
if (!env.TAURI_SIGNING_PRIVATE_KEY && existsSync(defaultKey)) {
  env.TAURI_SIGNING_PRIVATE_KEY = defaultKey;
  env.TAURI_SIGNING_PRIVATE_KEY_PASSWORD ??= '';
}
if (!env.TAURI_SIGNING_PRIVATE_KEY) {
  console.error(
    '缺少更新签名私钥，请按 README 配置 TAURI_SIGNING_PRIVATE_KEY。',
  );
  process.exit(1);
}
const runner =
  localCargo && existsSync(resolve(cargoHome, 'bin/cargo-xwin'))
    ? ['--runner', 'cargo-xwin']
    : [];

const licenses = spawnSync(process.execPath, ['scripts/release-licenses.mjs'], {
  cwd: root,
  env,
  stdio: 'inherit',
});
if (licenses.error || licenses.status !== 0) {
  console.error('发行授权文件生成失败。', licenses.error?.message ?? '');
  process.exit(licenses.status || 1);
}

for (const target of selectedTargets) {
  let buildEnv = env;
  if (
    runner.length &&
    target === targets.arm64 &&
    process.platform === 'darwin'
  ) {
    const shimDirectory = resolve(root, '.tools/windows-arm64-clang');
    mkdirSync(shimDirectory, { recursive: true });
    const shim = resolve(shimDirectory, 'clang');
    const source = resolve(root, 'scripts/clang-windows-arm64.mjs');
    try {
      if (lstatSync(shim).isSymbolicLink() && readlinkSync(shim) !== source)
        unlinkSync(shim);
    } catch (cause) {
      if (cause.code !== 'ENOENT') throw cause;
    }
    if (!existsSync(shim)) symlinkSync(source, shim);
    const llvm = ['/opt/homebrew/opt/llvm/bin', '/usr/local/opt/llvm/bin'].find(
      (path) => existsSync(resolve(path, 'clang-cl')),
    );
    buildEnv = {
      ...env,
      NEKOBOX_ARM64_CLANG: llvm ? resolve(llvm, 'clang') : '/usr/bin/clang',
      PATH: [shimDirectory, ...(llvm ? [llvm] : []), env.PATH].join(delimiter),
    };
  }
  console.log(`\n开始构建 Windows ${target}...`);
  const result = spawnSync(
    'pnpm',
    ['exec', 'tauri', 'build', ...runner, '--target', target],
    {
      cwd: root,
      env: buildEnv,
      shell: process.platform === 'win32',
      stdio: 'inherit',
    },
  );
  if (result.error) {
    console.error(`Windows ${target} 构建启动失败：${result.error.message}`);
    process.exit(1);
  }
  if (result.status !== 0) {
    console.error(`Windows ${target} 构建失败，退出码：${result.status}`);
    process.exit(result.status ?? 1);
  }
  const arch = target === targets.x64 ? 'x64' : 'arm64';
  const packaged = spawnSync(
    process.execPath,
    ['scripts/package-windows.mjs', arch],
    { cwd: root, stdio: 'inherit' },
  );
  if (packaged.error || packaged.status !== 0) {
    console.error(`Windows ${arch} 发行包整理失败。`);
    process.exit(packaged.status || 1);
  }
}

if (requested === 'all') {
  const manifest = spawnSync(process.execPath, ['scripts/release-update.mjs'], {
    cwd: root,
    env,
    stdio: 'inherit',
  });
  if (manifest.error || manifest.status !== 0)
    process.exit(manifest.status || 1);
}

console.log(
  `\nWindows ${selectedTargets
    .map((target) => (target === targets.x64 ? 'x64' : 'ARM64'))
    .join(' 与 ')} 构建完成。`,
);
