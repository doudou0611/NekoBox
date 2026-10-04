import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import {
  createUpdateManifest,
  writeReleaseChecksums,
} from './release-update.mjs';

test('complete manifests pin both architectures to the same release and embed signature contents', () => {
  const manifest = createUpdateManifest({
    version: '0.1.10',
    signatures: { x64: 'x64-signature\n', arm64: 'arm-signature\n' },
    publishedAt: '2026-10-04T00:00:00Z',
  });
  assert.equal(manifest.platforms['windows-x86_64'].signature, 'x64-signature');
  assert.equal(
    manifest.platforms['windows-aarch64'].url,
    'https://github.com/doudou0611/NekoBox/releases/download/v0.1.10/NekoBox_0.1.10_arm64-setup.exe',
  );
  assert.match(manifest.portable['windows-x86_64'].url, /x64-portable.zip$/);
});
test('partial signatures and malformed versions cannot be published as automatic updates', () => {
  const signatures = { x64: 'x', arm64: 'a' };
  assert.throws(() =>
    createUpdateManifest({ version: '0.1.1', signatures: { x64: 'x' } }),
  );
  for (const version of ['V0.1', '0.1', '01.1.0', '0.1.1-beta.1', '../0.1.1'])
    assert.throws(() => createUpdateManifest({ version, signatures }));
  assert.throws(() =>
    createUpdateManifest({
      version: '0.1.1',
      signatures,
      publishedAt: 'invalid',
    }),
  );
});
test('checksums cover manifest/signatures and never include signing keys or staging files', () => {
  const directory = mkdtempSync(join(tmpdir(), 'nekobox-release-'));
  try {
    for (const file of [
      'NekoBox_0.1.1_x64-setup.exe',
      'NekoBox_0.1.1_x64-setup.exe.sig',
      'NekoBox_0.1.1_x64-portable.zip',
      'latest.json',
      'updater.key',
      'notes.md',
    ])
      writeFileSync(join(directory, file), 'fixture');
    writeReleaseChecksums(directory);
    const sums = readFileSync(join(directory, 'SHA256SUMS.txt'), 'utf8');
    assert.equal(sums.trim().split('\n').length, 4);
    assert.match(sums, /latest.json/);
    assert.doesNotMatch(sums, /updater.key|notes.md/);
  } finally {
    rmSync(directory, { recursive: true });
  }
});
