import { afterEach, describe, expect, it, vi } from 'vitest';
import { screenshotResource } from './screenshotResource';
describe('registered screenshot URLs', () => {
  const id = 'a1234567-89ab-cdef-0123-456789abcdef';
  afterEach(() => vi.unstubAllGlobals());
  it.each(['windows', 'macos'])('preserves thumbnail routing on %s', (os) => {
    // Match Tauri's runtime conversion, including encodeURIComponent of the whole input.
    vi.stubGlobal('window', {
      __TAURI_INTERNALS__: {
        convertFileSrc: (path: string, protocol: string) =>
          `${os === 'windows' ? `http://${protocol}.localhost` : `${protocol}://localhost`}/${encodeURIComponent(path)}`,
      },
    });
    const prefix =
      os === 'windows'
        ? 'http://screenshot.localhost'
        : 'screenshot://localhost';
    expect(screenshotResource(`screenshot://localhost/${id}`)).toBe(
      `${prefix}/${id}`,
    );
    const path = `thumbnail/${id}/${'a'.repeat(64)}.png`;
    expect(screenshotResource(`screenshot://localhost/${path}`)).toBe(
      `${prefix}/${path}`,
    );
  });
  it('rejects arbitrary paths and remote image addresses', () => {
    for (const url of [
      'https://example.com/image.png',
      'screenshot://localhost/../../secret',
      `screenshot://localhost/${id}/file.png`,
      `screenshot://localhost/thumbnail/${id}`,
      `screenshot://localhost/thumbnail%2F${id}%2F${'a'.repeat(64)}.png`,
      `screenshot://localhost/thumbnail/${id}/../${'a'.repeat(64)}.png`,
    ]) {
      expect(screenshotResource(url)).toBe('');
    }
  });
});
