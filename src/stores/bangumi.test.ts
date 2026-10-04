import { beforeEach, expect, it, vi } from 'vitest';
const mock = vi.hoisted(() => ({ api: vi.fn() }));
vi.mock('../services/remoteImages', () => ({
  localizeAvatar: async (account: unknown) => account,
}));
vi.mock('./library', () => ({
  api: mock.api,
  desktop: true,
  errorText: (error: Error) => error.message,
}));
import {
  bangumi,
  loginBangumi,
  logoutBangumi,
  refreshBangumi,
} from './bangumi';
const profile = {
  id: 1,
  username: 'tester',
  nickname: '测试',
  avatar_url: 'https://lain.bgm.tv/pic/user/l/test.jpg',
};
beforeEach(() => {
  mock.api.mockReset();
  bangumi.account = { status: 'signed_out', profile: null, message: '' };
  bangumi.busy = false;
  bangumi.error = '';
  bangumi.avatar_failed = false;
});
it('updates shared avatar/profile after successful login without retaining the token', async () => {
  mock.api.mockResolvedValue({
    status: 'authenticated',
    profile,
    message: '已登录',
  });
  await loginBangumi('fixture-secret');
  expect(mock.api).toHaveBeenCalledWith('login_bangumi', {
    access_token: 'fixture-secret',
  });
  expect(bangumi.account.profile).toEqual(profile);
  expect(JSON.stringify(bangumi)).not.toContain('fixture-secret');
});
it('keeps a failed login signed out and shows the storage/authorization error', async () => {
  mock.api.mockRejectedValue(new Error('凭据库无法写入'));
  await loginBangumi('fixture-secret');
  expect(bangumi.account.status).toBe('signed_out');
  expect(bangumi.error).toBe('凭据库无法写入');
  expect(bangumi.busy).toBe(false);
});
it('restores account from the backend and clears profile and avatar on logout', async () => {
  mock.api.mockResolvedValueOnce({
    status: 'offline',
    profile,
    message: '离线',
  });
  await refreshBangumi();
  expect(bangumi.account.status).toBe('offline');
  bangumi.avatar_failed = true;
  mock.api.mockResolvedValueOnce({
    status: 'signed_out',
    profile: null,
    message: '已退出',
  });
  await logoutBangumi();
  expect(mock.api).toHaveBeenLastCalledWith('logout_bangumi', {});
  expect(bangumi.account.profile).toBeNull();
  expect(bangumi.avatar_failed).toBe(false);
});
