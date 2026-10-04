import { existsSync } from 'node:fs';
import { resolve, delimiter } from 'node:path';
import { spawnSync } from 'node:child_process';
const root = resolve(import.meta.dirname, '..');
const cargoHome = resolve(root, '.tools/cargo');
const local =
  process.platform !== 'win32' && existsSync(resolve(cargoHome, 'bin/cargo'));
const env = local
  ? {
      ...process.env,
      CARGO_HOME: cargoHome,
      RUSTUP_HOME: resolve(root, '.tools/rustup'),
      PATH: resolve(cargoHome, 'bin') + delimiter + process.env.PATH,
    }
  : process.env;
const result = spawnSync('cargo', process.argv.slice(2), {
  cwd: root,
  env,
  stdio: 'inherit',
});
if (result.error)
  console.error('Cargo 不可用，请按 README 安装 Rust：', result.error.message);
process.exit(result.status ?? 1);
