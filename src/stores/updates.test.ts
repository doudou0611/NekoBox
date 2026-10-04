import { beforeEach, afterEach, expect, it, vi } from 'vitest';
const mock = vi.hoisted(() => ({
  api: vi.fn(),
  notify: vi.fn(),
  desktop: true,
}));
vi.mock('./library', () => ({
  api: mock.api,
  notify: mock.notify,
  get desktop() {
    return mock.desktop;
  },
  errorText: (error: Error) => error.message,
}));
let store: typeof import('./updates');
beforeEach(async () => {
  vi.resetModules();
  mock.api.mockReset();
  mock.notify.mockReset();
  mock.desktop = true;
  store = await import('./updates');
});
afterEach(() => vi.useRealTimers());
const deferred = <T>() => {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((done) => {
    resolve = done;
  });
  return { promise, resolve };
};
it('checks once per launch and announces an available update without downloading or installing', async () => {
  mock.api.mockResolvedValue({
    ...store.updates.status,
    phase: 'available',
    version: '0.1.2',
    signed: true,
  });
  await store.checkStartupUpdate();
  await store.checkStartupUpdate();
  expect(mock.api).toHaveBeenCalledExactlyOnceWith('check_app_update', {});
  expect(mock.notify).toHaveBeenCalledTimes(1);
  expect(store.updates.notice).toBe(true);
});
it('deduplicates concurrent checks and exposes connection failures without success claims', async () => {
  const check = deferred<never>();
  mock.api.mockReturnValue(check.promise);
  const first = store.checkAppUpdate();
  await store.checkAppUpdate();
  expect(mock.api).toHaveBeenCalledTimes(1);
  check.resolve(Promise.reject(new Error('GitHub 连接失败')) as never);
  await first;
  expect(store.updates.status.phase).toBe('error');
  expect(store.updates.error).toBe('GitHub 连接失败');
  expect(store.updates.busy).toBe(false);
});
it('portable or unsigned releases cannot trigger automatic download or installation', async () => {
  Object.assign(store.updates.status, {
    phase: 'available',
    signed: false,
    installation: 'portable',
  });
  await store.downloadAppUpdate();
  await store.installAppUpdate();
  expect(mock.api).not.toHaveBeenCalled();
  await store.openUpdateRelease(true);
  expect(mock.api).toHaveBeenCalledExactlyOnceWith('open_app_update_release', {
    download: true,
  });
});
it('late progress responses cannot overwrite a verified ready package', async () => {
  vi.useFakeTimers();
  Object.assign(store.updates.status, { phase: 'available', signed: true });
  const download = deferred<unknown>();
  const progress = deferred<unknown>();
  mock.api.mockImplementation((name) =>
    name === 'download_app_update' ? download.promise : progress.promise,
  );
  const operation = store.downloadAppUpdate();
  await vi.advanceTimersByTimeAsync(500);
  download.resolve({
    ...store.updates.status,
    phase: 'ready',
    downloaded: 100,
  });
  await operation;
  progress.resolve({
    ...store.updates.status,
    phase: 'downloading',
    downloaded: 50,
  });
  await Promise.resolve();
  await Promise.resolve();
  expect(store.updates.status.phase).toBe('ready');
  expect(store.updates.status.downloaded).toBe(100);
  await vi.advanceTimersByTimeAsync(1000);
  expect(mock.api).toHaveBeenCalledTimes(2);
});
it('failed signature/download stays retryable and never installs', async () => {
  Object.assign(store.updates.status, { phase: 'available', signed: true });
  mock.api
    .mockRejectedValueOnce(new Error('签名校验失败'))
    .mockResolvedValueOnce({ ...store.updates.status });
  await store.downloadAppUpdate();
  expect(store.updates.status.phase).toBe('available');
  expect(store.updates.error).toBe('签名校验失败');
  expect(mock.api.mock.calls.map(([name]) => name)).toEqual([
    'download_app_update',
    'get_app_update_status',
  ]);
});
it('installation requires the ready state and backend refusal preserves the verified download', async () => {
  await store.installAppUpdate();
  expect(mock.api).not.toHaveBeenCalled();
  store.updates.status.phase = 'ready';
  mock.api.mockRejectedValue(new Error('游戏仍在运行'));
  await store.installAppUpdate();
  expect(mock.api).toHaveBeenCalledExactlyOnceWith('install_app_update', {
    confirmed: true,
  });
  expect(store.updates.status.phase).toBe('ready');
  expect(store.updates.error).toBe('游戏仍在运行');
});
it('browser previews never make native update requests', async () => {
  mock.desktop = false;
  await store.checkStartupUpdate();
  await store.refreshUpdateStatus();
  await store.downloadAppUpdate();
  await store.installAppUpdate();
  await store.openUpdateRelease();
  expect(mock.api).not.toHaveBeenCalled();
});
