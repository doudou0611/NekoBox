import { readFileSync, readdirSync, writeFileSync } from 'node:fs';
import { resolve, join } from 'node:path';
import { createHash } from 'node:crypto';
import { pathToFileURL } from 'node:url';

const repository = 'https://github.com/doudou0611/NekoBox';
export function createUpdateManifest({
  version,
  signatures,
  tag = `v${version}`,
  notes = '',
  publishedAt = new Date().toISOString(),
}) {
  if (!/^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/.test(version))
    throw Error('正式发布版本必须采用完整 SemVer，例如 0.1.1。');
  if (![version, `v${version}`, `V${version}`].includes(tag))
    throw Error('Release 标签必须与版本一致，例如 0.1.2、v0.1.2 或 V0.1.2。');
  if (!Number.isFinite(Date.parse(publishedAt))) throw Error('发布日期无效。');
  const platforms = {};
  const portable = {};
  for (const [arch, target] of [
    ['x64', 'windows-x86_64'],
    ['arm64', 'windows-aarch64'],
  ]) {
    const signature = signatures[arch]?.trim();
    if (!signature)
      throw Error(`缺少 ${arch} 安装包的签名，不能生成更新清单。`);
    const prefix = `${repository}/releases/download/${tag}/NekoBox_${version}_${arch}`;
    platforms[target] = { signature, url: `${prefix}-setup.exe` };
    portable[target] = { url: `${prefix}-portable.zip` };
  }
  return {
    version,
    notes,
    pub_date: new Date(publishedAt).toISOString(),
    platforms,
    portable,
  };
}

export function writeReleaseChecksums(directory) {
  const files = readdirSync(directory)
    .filter((name) =>
      /(-setup\.exe(?:\.sig)?|-portable\.zip|latest\.json)$/.test(name),
    )
    .sort();
  writeFileSync(
    join(directory, 'SHA256SUMS.txt'),
    files
      .map(
        (name) =>
          `${createHash('sha256')
            .update(readFileSync(join(directory, name)))
            .digest('hex')}  ${name}`,
      )
      .join('\n') + '\n',
  );
}

function main() {
  const root = resolve(import.meta.dirname, '..');
  const { version } = JSON.parse(
    readFileSync(join(root, 'src-tauri/tauri.conf.json'), 'utf8'),
  );
  const directory = resolve(root, '.tools/releases', `v${version}`);
  const signatures = {};
  for (const arch of ['x64', 'arm64']) {
    const prefix = `NekoBox_${version}_${arch}`;
    // Require all four binaries before advertising either platform.
    for (const suffix of ['-setup.exe', '-portable.zip'])
      readFileSync(join(directory, prefix + suffix));
    signatures[arch] = readFileSync(
      join(directory, `${prefix}-setup.exe.sig`),
      'utf8',
    );
  }
  const notes = process.env.NEKOBOX_RELEASE_NOTES_PATH
    ? readFileSync(resolve(process.env.NEKOBOX_RELEASE_NOTES_PATH), 'utf8')
    : '更新说明请查看本版本的 GitHub Release 页面。';
  const tag = process.env.NEKOBOX_RELEASE_TAG ?? `v${version}`;
  const manifest = createUpdateManifest({ version, signatures, notes, tag });
  writeFileSync(
    join(directory, 'latest.json'),
    JSON.stringify(manifest, null, 2) + '\n',
  );
  writeReleaseChecksums(directory);
  console.log(
    `更新清单已生成：${join(directory, 'latest.json')}\n请创建 ${tag} 标签，上传两个安装包、两个便携包、两个 .sig、latest.json 和 SHA256SUMS.txt；全部上传后再发布。`,
  );
}
if (
  process.argv[1] &&
  import.meta.url === pathToFileURL(resolve(process.argv[1])).href
)
  main();
