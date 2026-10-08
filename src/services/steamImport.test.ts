import { describe, expect, it } from 'vitest';
import { installationLaunchable } from './steamImport';
describe('Steam launch availability', () => {
  const steam = {
    path_valid: true,
    executable_path: null,
    source: 'steam' as const,
    steam_app_id: '1230140',
  };
  it('makes installed Steam titles playable without a selected executable', () => {
    expect(installationLaunchable(steam)).toBe(true);
    expect(installationLaunchable({ ...steam, path_valid: false })).toBe(false);
    expect(installationLaunchable({ ...steam, source: 'manual' })).toBe(false);
  });
  it('rejects missing, malformed and overflowing identifiers', () => {
    for (const id of [null, '0', '-1', '00123', '4294967296', '123.exe'])
      expect(installationLaunchable({ ...steam, steam_app_id: id })).toBe(
        false,
      );
  });
});
