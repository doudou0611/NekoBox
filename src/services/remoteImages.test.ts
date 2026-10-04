import { beforeEach, expect, it, vi } from 'vitest';
const mock = vi.hoisted(() => ({ api: vi.fn() }));
vi.mock('../stores/library', () => ({
  api: mock.api,
  desktop: true,
  coverSource: (path: string) => `cover://localhost/${path}`,
}));
import { localizeAvatar, remoteImage } from './remoteImages';
beforeEach(() => mock.api.mockReset());
it('shares native downloads and leaves managed image URLs unchanged', async () => {
  mock.api.mockResolvedValue('covers/fixture.png');
  const url = 'https://lain.bgm.tv/pic/cover/l/shared.jpg';
  expect(await Promise.all([remoteImage(url), remoteImage(url)])).toEqual([
    'cover://localhost/covers/fixture.png',
    'cover://localhost/covers/fixture.png',
  ]);
  expect(mock.api).toHaveBeenCalledTimes(1);
  expect(mock.api).toHaveBeenCalledWith('cache_remote_image', {
    url,
    avatar: false,
  });
  expect(await remoteImage('cover://localhost/existing')).toBe(
    'cover://localhost/existing',
  );
});
it('keeps authenticated account details when avatar download fails and permits retry', async () => {
  const url = 'https://lain.bgm.tv/pic/user/l/retry.jpg';
  mock.api.mockRejectedValueOnce(new Error('图片下载失败'));
  const account = await localizeAvatar({
    status: 'authenticated',
    profile: { id: 1, avatar_url: url },
  });
  expect(account.status).toBe('authenticated');
  expect(account.profile).toEqual({ id: 1, avatar_url: null });
  mock.api.mockResolvedValueOnce('covers/avatar.png');
  expect(await remoteImage(url, true)).toBe(
    'cover://localhost/covers/avatar.png',
  );
  expect(mock.api).toHaveBeenCalledTimes(2);
});
