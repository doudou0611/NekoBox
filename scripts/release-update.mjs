import { readFileSync, readdirSync, writeFileSync } from 'node:fs';
import { resolve, join } from 'node:path';
import { createHash } from 'node:crypto';
import { pathToFileURL } from 'node:url';

const repository = 'https://github.com/doudou0611/NekoBox';
export function createUpdateManifest({
  version,
  signatures,
  tag = version,
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

export function validateReleaseManifest(release, manifest) {
  const expected = createUpdateManifest({
    version: manifest.version,
    tag: release.tag_name,
    publishedAt: manifest.pub_date,
    signatures: {
      x64: manifest.platforms?.['windows-x86_64']?.signature,
      arm64: manifest.platforms?.['windows-aarch64']?.signature,
    },
  });
  if (
    release.draft ||
    release.prerelease ||
    release.html_url !== `${repository}/releases/tag/${release.tag_name}`
  )
    throw Error('必须核验本仓库的正式 Release。');
  const required = ['latest.json', 'SHA256SUMS.txt'];
  for (const [arch, platform] of [
    ['x64', 'windows-x86_64'],
    ['arm64', 'windows-aarch64'],
  ]) {
    for (const kind of ['platforms', 'portable']) {
      if (manifest[kind]?.[platform]?.url !== expected[kind][platform].url)
        throw Error(
          `${arch} ${kind} 下载链接与实际 Release 标签 ${release.tag_name} 不一致。`,
        );
    }
    const prefix = `NekoBox_${manifest.version}_${arch}`;
    required.push(
      `${prefix}-setup.exe`,
      `${prefix}-setup.exe.sig`,
      `${prefix}-portable.zip`,
    );
  }
  for (const name of required) {
    const matches = release.assets.filter((asset) => asset.name === name);
    if (
      matches.length !== 1 ||
      matches[0].browser_download_url !==
        `${repository}/releases/download/${release.tag_name}/${name}`
    )
      throw Error(`Release 缺少有效的 ${name} 附件。`);
  }
  return expected;
}

async function verifyPublishedRelease(version) {
  const read = async (url, json = true) => {
    const response = await fetch(url, {
      headers: {
        'User-Agent': `NekoBox/${version}`,
        'Cache-Control': 'no-cache',
      },
      signal: AbortSignal.timeout(30000),
    });
    if (!response.ok)
      throw Error(`发布核验请求失败：${response.status} ${url}`);
    return json ? response.json() : response.text();
  };
  const release = await read(
    `${repository.replace('https://github.com/', 'https://api.github.com/repos/')}/releases/latest?check_at=${Date.now()}`,
  );
  const manifestAsset = release.assets.find(
    (asset) => asset.name === 'latest.json',
  );
  if (!manifestAsset) throw Error('最新 Release 缺少 latest.json。');
  // Replacing a GitHub asset keeps its URL but changes its id; avoid stale redirects.
  const manifestUrl = new URL(manifestAsset.browser_download_url);
  manifestUrl.searchParams.set('asset_id', String(manifestAsset.id));
  const manifest = await read(manifestUrl);
  validateReleaseManifest(release, manifest);
  if (manifest.version !== version)
    throw Error(`线上版本为 ${manifest.version}，当前构建版本为 ${version}。`);
  for (const [arch, platform] of [
    ['x64', 'windows-x86_64'],
    ['arm64', 'windows-aarch64'],
  ]) {
    const asset = release.assets.find(
      (asset) => asset.name === `NekoBox_${version}_${arch}-setup.exe.sig`,
    );
    const signature = await read(asset.browser_download_url, false);
    if (signature.trim() !== manifest.platforms[platform].signature.trim())
      throw Error(`${arch} 清单签名与发布的 .sig 文件不一致。`);
  }
  console.log(
    `线上发布核验通过：${release.tag_name}，两个架构的下载链接、附件和签名一致。`,
  );
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

async function main() {
  const root = resolve(import.meta.dirname, '..');
  const { version } = JSON.parse(
    readFileSync(join(root, 'src-tauri/tauri.conf.json'), 'utf8'),
  );
  if (process.argv.includes('--verify')) return verifyPublishedRelease(version);
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
  const tag = process.env.NEKOBOX_RELEASE_TAG ?? version;
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
  main().catch((error) => {
    console.error(error.message);
    process.exitCode = 1;
  });
