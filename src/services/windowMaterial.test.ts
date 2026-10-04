import { beforeEach, describe, expect, it, vi } from 'vitest';
const adapter = vi.hoisted(() => ({
  native: false,
  setTheme: vi.fn(),
  setEffects: vi.fn(),
  clearEffects: vi.fn(),
}));
vi.mock('@tauri-apps/api/core', () => ({ isTauri: () => adapter.native }));
vi.mock('@tauri-apps/api/window', async (importOriginal) => ({
  ...(await importOriginal<typeof import('@tauri-apps/api/window')>()),
  getCurrentWindow: () => adapter,
}));
import { syncDesktopMaterial, syncDesktopTheme } from './windowMaterial';

describe('native appearance boundary', () => {
  beforeEach(() => {
    adapter.native = false;
    adapter.setTheme.mockReset();
    adapter.setEffects.mockReset();
    adapter.clearEffects.mockReset();
  });
  it('does not call the desktop from a browser', async () => {
    expect(await syncDesktopTheme('dark')).toBe(false);
    expect(adapter.setTheme).not.toHaveBeenCalled();
  });
  it('passes the explicit theme to the window only', async () => {
    adapter.native = true;
    adapter.setTheme.mockResolvedValue(undefined);
    expect(await syncDesktopTheme('light')).toBe(true);
    expect(adapter.setTheme).toHaveBeenCalledWith('light');
  });
  it('sanitizes native rejection and permits opaque fallback', async () => {
    adapter.native = true;
    adapter.setTheme.mockRejectedValue(new Error('private native information'));
    expect(await syncDesktopTheme('dark')).toBe(false);
  });
});

describe('native material preference', () => {
  beforeEach(() => {
    adapter.native = true;
    adapter.setTheme.mockResolvedValue(undefined);
    adapter.setEffects.mockReset().mockResolvedValue(undefined);
    adapter.clearEffects.mockReset().mockResolvedValue(undefined);
  });
  it('actually clears the native Acrylic when glass is disabled', async () => {
    expect(await syncDesktopMaterial('dark', false, 'windows-acrylic')).toBe(
      true,
    );
    expect(adapter.clearEffects).toHaveBeenCalledOnce();
    expect(adapter.setEffects).not.toHaveBeenCalled();
  });
  it('keeps Parallels opaque even when the saved glass preference is on', async () => {
    await syncDesktopMaterial('light', true, 'windows-virtual-machine');
    expect(adapter.clearEffects).toHaveBeenCalledOnce();
    expect(adapter.setEffects).not.toHaveBeenCalled();
  });
  it('restores Acrylic for a supported normal Windows host', async () => {
    await syncDesktopMaterial('dark', true, 'windows-acrylic');
    expect(adapter.setEffects).toHaveBeenCalledWith({
      effects: ['acrylic'],
      color: [0, 0, 0, 1],
    });
  });
  it('serializes rapid changes while a native operation is still pending', async () => {
    let release!: () => void;
    adapter.setEffects.mockImplementationOnce(
      () =>
        new Promise<void>((resolve) => {
          release = resolve;
        }),
    );
    const enable = syncDesktopMaterial('dark', true, 'windows-acrylic');
    const disable = syncDesktopMaterial('dark', false, 'windows-acrylic');
    await vi.waitFor(() => expect(adapter.setEffects).toHaveBeenCalledOnce());
    expect(adapter.clearEffects).not.toHaveBeenCalled();
    release();
    await Promise.all([enable, disable]);
    expect(adapter.clearEffects).toHaveBeenCalledOnce();
  });
  it('recovers after a rejected material operation', async () => {
    adapter.clearEffects.mockRejectedValueOnce(new Error('native failure'));
    expect(await syncDesktopMaterial('dark', false, 'windows-acrylic')).toBe(
      false,
    );
    expect(await syncDesktopMaterial('dark', true, 'windows-acrylic')).toBe(
      true,
    );
  });
});
