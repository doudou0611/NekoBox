import {
  copyFileSync,
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  statSync,
  writeFileSync,
} from 'node:fs';
import { basename, dirname, join, relative, resolve } from 'node:path';
import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';

const root = resolve(import.meta.dirname, '..');
const sources = resolve(root, 'licenses/sources');
mkdirSync(sources, { recursive: true });
function run(command, args) {
  const result = spawnSync(command, args, {
    cwd: root,
    encoding: 'utf8',
    maxBuffer: 32 * 1024 * 1024,
    shell: process.platform === 'win32' && command === 'pnpm',
  });
  if (result.error || result.status !== 0)
    throw Error(result.error?.message || result.stderr || `${command} failed`);
  return JSON.parse(result.stdout);
}
async function download(url) {
  const response = await fetch(url, { signal: AbortSignal.timeout(30000) });
  if (!response.ok)
    throw Error(`下载授权文件失败：${url} (${response.status})`);
  return Buffer.from(await response.arrayBuffer());
}
function licenseFiles(directory) {
  const files = [];
  function visit(current) {
    for (const entry of readdirSync(current, { withFileTypes: true })) {
      const file = join(current, entry.name);
      if (entry.isDirectory() && !['node_modules', '.git'].includes(entry.name))
        visit(file);
      else if (
        entry.isFile() &&
        /licen[cs]e|copying|copyright|notice|authors/i.test(entry.name)
      )
        files.push(file);
    }
  }
  visit(directory);
  return files.sort().map((file) => ({
    name: relative(directory, file).replaceAll('\\', '/'),
    text: readFileSync(file, 'utf8'),
  }));
}
const packages = new Map();
for (const platform of ['x86_64-pc-windows-msvc', 'aarch64-pc-windows-msvc']) {
  const metadata = run(process.execPath, [
    'scripts/rust.mjs',
    'metadata',
    '--locked',
    '--filter-platform',
    platform,
    '--manifest-path',
    'src-tauri/Cargo.toml',
    '--format-version',
    '1',
  ]);
  for (const pkg of metadata.packages) {
    if (pkg.source) packages.set(`${pkg.name}@${pkg.version}`, pkg);
  }
}
const mplPath = resolve(root, 'licenses/MPL-2.0.txt');
const mplText = existsSync(mplPath)
  ? readFileSync(mplPath, 'utf8')
  : (
      await download('https://www.mozilla.org/media/MPL/2.0/index.txt')
    ).toString('utf8');
if (!mplText.includes('Mozilla Public License Version 2.0'))
  throw Error('MPL 授权文本内容异常。');
if (!existsSync(mplPath)) writeFileSync(mplPath, mplText);
const sections = [
  'NekoBox — Third-party notices',
  '',
  'Generated from the locked Windows x64/ARM64 Cargo dependency graphs and the installed pnpm production dependency graph.',
  'This inventory conservatively includes build-time and transitive packages; inclusion does not mean every listed package is linked into the executable.',
  'NekoBox source is licensed under MIT. Third-party components retain their own copyrights and license terms.',
  'Unmodified MPL-2.0 crate source archives are included in licenses/sources/. Extract these gzip-compressed tar archives to obtain the corresponding source, including copyright and license headers.',
  'The MPL-2.0 license text is included in licenses/MPL-2.0.txt.',
  '',
];
const lock = readFileSync(resolve(root, 'src-tauri/Cargo.lock'), 'utf8');
const checksums = new Map();
for (const block of lock.split('[[package]]')) {
  const name = /^name = "([^"]+)"/m.exec(block)?.[1];
  const version = /^version = "([^"]+)"/m.exec(block)?.[1];
  const checksum = /^checksum = "([^"]+)"/m.exec(block)?.[1];
  if (checksum) checksums.set(`${name}@${version}`, checksum);
}
let mplCount = 0;
for (const pkg of [...packages.values()].sort((a, b) =>
  `${a.name}@${a.version}`.localeCompare(`${b.name}@${b.version}`),
)) {
  const directory = dirname(pkg.manifest_path);
  const texts = licenseFiles(directory);
  if (!pkg.license) throw Error(`依赖缺少授权声明：${pkg.name}`);
  if (pkg.license_file && !texts.some((item) => item.name === pkg.license_file))
    texts.push({
      name: pkg.license_file,
      text: readFileSync(resolve(directory, pkg.license_file), 'utf8'),
    });
  if (texts.length === 0 && pkg.license !== 'MPL-2.0') {
    const vcs = JSON.parse(
      readFileSync(join(directory, '.cargo_vcs_info.json')),
    );
    const repository = pkg.repository?.replace(/\.git$/, '').replace(/\/$/, '');
    if (!repository?.startsWith('https://github.com/'))
      throw Error(`需人工补充授权文件：${pkg.name}`);
    const base = `${repository.replace('https://github.com/', 'https://raw.githubusercontent.com/')}/${vcs.git.sha1}`;
    const cache = resolve(
      root,
      '.tools/release-license-cache',
      `${pkg.name}-${pkg.version}`,
      vcs.git.sha1,
    );
    if (existsSync(cache)) texts.push(...licenseFiles(cache));
    if (!texts.length) {
      const downloaded = await Promise.all(
        [
          'LICENSE',
          'LICENSE-MIT',
          'LICENSE-APACHE',
          'LICENSE_MIT',
          'LICENSE_APACHE',
          'LICENSE.txt',
          'COPYING',
        ].map(async (name) => {
          const response = await fetch(`${base}/${name}`, {
            signal: AbortSignal.timeout(30000),
          });
          if (response.ok) return { name, text: await response.text() };
          if (response.status !== 404)
            throw Error(
              `无法读取上游授权文件：${pkg.name} (${response.status})`,
            );
          return null;
        }),
      );
      for (const item of downloaded.filter(Boolean)) {
        mkdirSync(cache, { recursive: true });
        writeFileSync(join(cache, item.name), item.text);
        texts.push(item);
      }
    }
    if (texts.length === 0) throw Error(`需人工补充授权文件：${pkg.name}`);
  }
  const sourceUrl = `https://static.crates.io/crates/${pkg.name}/${pkg.name}-${pkg.version}.crate`;
  sections.push(
    '='.repeat(78),
    `Rust: ${pkg.name} ${pkg.version}`,
    `License: ${pkg.license}`,
    `Authors: ${(pkg.authors || []).join('; ')}`,
    `Repository: ${pkg.repository || ''}`,
    `Source: ${sourceUrl}`,
    '',
  );
  if (pkg.license === 'MPL-2.0') {
    const archive = `${pkg.name}-${pkg.version}.crate`;
    const cache = resolve(
      directory,
      '../../../cache',
      basename(dirname(directory)),
      archive,
    );
    const bytes = existsSync(cache)
      ? readFileSync(cache)
      : await download(sourceUrl);
    if (
      createHash('sha256').update(bytes).digest('hex') !==
      checksums.get(`${pkg.name}@${pkg.version}`)
    )
      throw Error(`依赖源码校验失败：${archive}`);
    writeFileSync(join(sources, archive), bytes);
    sections.push(`Included source: licenses/sources/${archive}`, '');
    if (!texts.length) texts.push({ name: 'MPL-2.0', text: mplText });
    mplCount++;
  }
  for (const item of texts)
    sections.push(`--- ${item.name} ---`, item.text.trimEnd(), '');
}
const npmPackages = new Map();
function collect(node) {
  for (const dependency of Object.values(node.dependencies || {})) {
    if (dependency.path) npmPackages.set(dependency.path, dependency);
    collect(dependency);
  }
}
for (const tree of run('pnpm', [
  'list',
  '--prod',
  '--depth',
  'Infinity',
  '--json',
]))
  collect(tree);
let npmCount = 0;
for (const [directory] of [...npmPackages].sort(([a], [b]) =>
  a.localeCompare(b),
)) {
  // pnpm also lists optional native packages that are not installed on this OS.
  if (!existsSync(join(directory, 'package.json'))) continue;
  const pkg = JSON.parse(readFileSync(join(directory, 'package.json'), 'utf8'));
  const texts = licenseFiles(directory);
  if (!texts.length && pkg.name.startsWith('@rolldown/binding-')) {
    // Rolldown's native packages share the parent package's MIT license.
    const parent = [...npmPackages.keys()].find((path) => {
      if (!existsSync(join(path, 'package.json'))) return false;
      const other = JSON.parse(
        readFileSync(join(path, 'package.json'), 'utf8'),
      );
      return other.name === 'rolldown' && other.version === pkg.version;
    });
    if (parent) texts.push(...licenseFiles(parent));
  }
  if (!texts.length) throw Error(`前端依赖缺少授权文件：${pkg.name}`);
  const license =
    typeof pkg.license === 'object' ? pkg.license.type : pkg.license;
  sections.push(
    '='.repeat(78),
    `npm: ${pkg.name} ${pkg.version}`,
    `License: ${license || 'See included license text'}`,
    `Source: https://www.npmjs.com/package/${pkg.name}/v/${pkg.version}`,
    '',
  );
  for (const item of texts)
    sections.push(`--- ${item.name} ---`, item.text.trimEnd(), '');
  npmCount++;
}
const output = resolve(root, 'THIRD_PARTY_NOTICES.txt');
writeFileSync(output, sections.join('\n'));
if (!statSync(output).size) throw Error('第三方授权声明为空。');
copyFileSync(
  resolve(root, 'LICENSE'),
  resolve(root, 'licenses/MIT.txt'),
);
console.log(
  `授权文件已生成：${packages.size} 个 Rust 依赖、${npmCount} 个前端依赖、${mplCount} 份 MPL 源码。`,
);
