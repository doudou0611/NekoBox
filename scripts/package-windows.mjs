import {
  copyFileSync,
  cpSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  rmSync,
  writeFileSync,
} from 'node:fs';
import { resolve, join } from 'node:path';
import { writeReleaseChecksums } from './release-update.mjs';
import { spawnSync } from 'node:child_process';

const root = resolve(import.meta.dirname, '..');
const arch = process.argv[2];
const targets = {
  x64: { triple: 'x86_64-pc-windows-msvc', machine: 0x8664 },
  arm64: { triple: 'aarch64-pc-windows-msvc', machine: 0xaa64 },
};
if (!targets[arch])
  throw Error('用法：node scripts/package-windows.mjs [x64|arm64]');
const config = JSON.parse(
  readFileSync(resolve(root, 'src-tauri/tauri.conf.json')),
);
const { version, productName } = config;
const binaryDirectory = resolve(
  root,
  'src-tauri/target',
  targets[arch].triple,
  'release',
);
const exe = readFileSync(join(binaryDirectory, `${productName}.exe`));
const pe = exe.readUInt32LE(0x3c);
if (
  exe.toString('ascii', pe, pe + 4) !== 'PE\0\0' ||
  exe.readUInt16LE(pe + 4) !== targets[arch].machine ||
  exe.readUInt16LE(pe + 24) !== 0x20b
)
  throw Error(`Windows ${arch} 程序架构不匹配。`);
const installerDirectory = join(binaryDirectory, 'bundle/nsis');
const installers = readdirSync(installerDirectory).filter(
  (name) => name.includes(`_${version}_`) && name.endsWith('-setup.exe'),
);
if (installers.length !== 1) throw Error(`无法唯一确定 ${arch} 安装包。`);
const destination = resolve(root, '.tools/releases', `v${version}`);
mkdirSync(destination, { recursive: true });
const prefix = `${productName}_${version}_${arch}`;
const installer = join(destination, `${prefix}-setup.exe`);
copyFileSync(join(installerDirectory, installers[0]), installer);
// Tauri signs the original bytes; standardizing the filename does not alter the signature.
copyFileSync(
  join(installerDirectory, installers[0] + '.sig'),
  installer + '.sig',
);
// A rebuilt artifact invalidates any earlier manifest until both architectures are finalized.
rmSync(join(destination, 'latest.json'), { force: true });
const temporary = mkdtempSync(join(destination, '.staging-'));
const portable = join(temporary, prefix);
mkdirSync(portable);
copyFileSync(
  join(binaryDirectory, `${productName}.exe`),
  join(portable, `${productName}.exe`),
);
for (const name of readdirSync(binaryDirectory).filter((name) =>
  name.endsWith('.dll'),
))
  copyFileSync(join(binaryDirectory, name), join(portable, name));
for (const name of ['LICENSE', 'THIRD_PARTY_NOTICES.txt'])
  copyFileSync(join(root, name), join(portable, name));
cpSync(resolve(root, 'licenses'), join(portable, 'licenses'), {
  recursive: true,
});
writeFileSync(
  join(portable, 'README.txt'),
  `${productName} ${version} — Windows ${arch}\r\n\r\n` +
    '请将整个文件夹解压到有写入权限的位置，然后运行 NekoBox.exe。\r\n' +
    '首次运行会在程序旁创建 data 目录。升级时先退出软件，保留原有 data；不要用空目录覆盖旧数据。\r\n' +
    '便携包不包含个人游戏库、游戏本体、登录凭据或存档。\r\n' +
    '需要 Microsoft Edge WebView2 Runtime：https://developer.microsoft.com/microsoft-edge/webview2/\r\n' +
    '授权条款见 LICENSE、THIRD_PARTY_NOTICES.txt 和 licenses 目录。\r\n',
);
const zip = join(destination, `${prefix}-portable.zip`);
rmSync(zip, { force: true });
let command;
let args;
if (process.platform === 'darwin') {
  command = '/usr/bin/ditto';
  args = ['-c', '-k', '--norsrc', '--noextattr', '--keepParent', portable, zip];
} else if (process.platform === 'win32') {
  command = 'powershell.exe';
  const quote = (path) => `'${path.replaceAll("'", "''")}'`;
  args = [
    '-NoProfile',
    '-NonInteractive',
    '-Command',
    `$ErrorActionPreference='Stop'; Compress-Archive -LiteralPath ${quote(portable)} -DestinationPath ${quote(zip)} -CompressionLevel Optimal`,
  ];
} else {
  command = 'zip';
  args = ['-q', '-r', zip, prefix];
}
try {
  const result = spawnSync(command, args, { cwd: temporary, stdio: 'inherit' });
  if (result.error || result.status !== 0)
    throw Error(result.error?.message || '便携包压缩失败。');
} finally {
  rmSync(temporary, { recursive: true, force: true });
}
writeReleaseChecksums(destination);
console.log(`已验证 ${arch} PE 架构，发行文件：\n${installer}\n${zip}`);
