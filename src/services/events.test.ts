import { describe, expect, it, vi } from 'vitest';
import { createEventSubscription } from './events';
describe('event subscription lifecycle', () => {
  it('cleans a late subscription after abort', async () => {
    let resolve!: (unlisten: () => void) => void;
    const unlisten = vi.fn();
    const callback = vi.fn();
    const controller = new AbortController();
    const subscribe = vi.fn(
      () =>
        new Promise<() => void>((done) => {
          resolve = done;
        }),
    );
    const pending = createEventSubscription(subscribe, () => true)(
      'scan_progress',
      callback,
      controller.signal,
    );
    controller.abort();
    resolve(unlisten);
    const dispose = await pending;
    dispose();
    expect(subscribe).toHaveBeenCalledWith(
      'scan:progress',
      expect.any(Function),
    );
    expect(unlisten).toHaveBeenCalledTimes(1);
  });
  it('disposes once even when abort and manual disposal race', async () => {
    const unlisten = vi.fn();
    const controller = new AbortController();
    const dispose = await createEventSubscription(
      async () => unlisten,
      () => true,
    )('metadata_progress', () => {}, controller.signal);
    dispose();
    controller.abort();
    dispose();
    expect(unlisten).toHaveBeenCalledTimes(1);
  });
  it('never subscribes with an already aborted signal', async () => {
    const controller = new AbortController();
    controller.abort();
    const subscribe = vi.fn();
    await createEventSubscription(subscribe, () => true)(
      'scan_progress',
      () => {},
      controller.signal,
    );
    expect(subscribe).not.toHaveBeenCalled();
  });
  it('does not offer desktop events in browser mode', async () => {
    await expect(
      createEventSubscription(vi.fn(), () => false)('scan_progress', () => {}),
    ).rejects.toMatchObject({ code: 'DESKTOP_UNAVAILABLE' });
  });
});
