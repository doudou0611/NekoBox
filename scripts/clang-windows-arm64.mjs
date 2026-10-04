#!/usr/bin/env node
// ring selects the GNU clang driver on Windows ARM64, even when cargo-xwin
// supplies clang-cl flags. Translate its system include option without changing
// any source, ABI, optimization level or crypto implementation.
import { spawnSync } from 'node:child_process';

const compiler = process.env.NEKOBOX_ARM64_CLANG;
if (!compiler) {
  console.error('缺少 ARM64 Clang 路径，请通过 Windows 构建脚本运行。');
  process.exit(2);
}
const args = process.argv
  .slice(2)
  .map((arg) => (arg === '/imsvc' ? '-isystem' : arg));
const result = spawnSync(compiler, args, { stdio: 'inherit' });
if (result.error) console.error(result.error.message);
process.exit(result.status ?? 1);
