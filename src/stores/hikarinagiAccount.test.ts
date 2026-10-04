import { afterEach, beforeEach, expect, it, vi } from 'vitest';
const mock = vi.hoisted(() => ({ api: vi.fn() }));
vi.mock('./library', () => ({
  api: mock.api,
  desktop: true,
  errorText: (e: Error) => e.message,
}));
import {
  hikariAccount,
  beginHikariLogin,
  cancelHikariLogin,
} from './hikarinagiAccount';
beforeEach(() => {
  vi.useFakeTimers();
  mock.api.mockReset();
  hikariAccount.busy = false;
  hikariAccount.flow_id = '';
  hikariAccount.error = '';
  hikariAccount.account = { status: 'signed_out', profile: null, message: '' };
});
afterEach(async () => {
  await cancelHikariLogin();
  vi.useRealTimers();
});
it('shows a failed authorization only once and releases the login state', async () => {
  mock.api
    .mockResolvedValueOnce({
      flow_id: 'current',
      authorization_url: 'https://id.hikarinagi.org/oidc/auth',
      expires_in_seconds: 180,
    })
    .mockResolvedValueOnce({
      flow_id: 'current',
      status: 'failed',
      account: null,
      message: 'Hikarinagi 账户接口不存在，请更新软件后重试。',
    });
  await beginHikariLogin();
  await vi.advanceTimersByTimeAsync(900);
  expect(hikariAccount.error).toContain('账户接口不存在');
  expect(hikariAccount.login_message).toBe('');
  expect(hikariAccount.busy).toBe(false);
  expect(hikariAccount.flow_id).toBe('');
});
it('polls an authorized account without storing browser tokens and stops after success', async () => {
  const account = {
    status: 'authenticated',
    profile: { id: 2, username: 'hikari', nickname: '测试', avatar_url: null },
    message: 'ok',
  };
  mock.api
    .mockResolvedValueOnce({
      flow_id: 'current',
      authorization_url: 'https://id.hikarinagi.org/oidc/auth?state=current',
      expires_in_seconds: 180,
    })
    .mockResolvedValueOnce({
      flow_id: 'current',
      status: 'completed',
      message: 'ok',
      account,
    });
  await beginHikariLogin();
  expect(hikariAccount.busy).toBe(true);
  await vi.advanceTimersByTimeAsync(900);
  expect(hikariAccount.account).toEqual(account);
  expect(hikariAccount.flow_id).toBe('');
  expect(hikariAccount.busy).toBe(false);
  expect(mock.api).toHaveBeenCalledTimes(2);
});
it('cancels pending consent and ignores a late response from the abandoned flow', async () => {
  let resolve!: (v: unknown) => void;
  mock.api
    .mockResolvedValueOnce({
      flow_id: 'current',
      authorization_url: 'https://id.hikarinagi.org/oidc/auth',
      expires_in_seconds: 180,
    })
    .mockImplementationOnce(
      () =>
        new Promise((r) => {
          resolve = r;
        }),
    )
    .mockResolvedValueOnce(true);
  await beginHikariLogin();
  await vi.advanceTimersByTimeAsync(900);
  await cancelHikariLogin();
  resolve({
    status: 'completed',
    account: { status: 'authenticated', profile: { id: 99 } },
  });
  await Promise.resolve();
  expect(hikariAccount.account.status).toBe('signed_out');
  expect(hikariAccount.busy).toBe(false);
  expect(mock.api).toHaveBeenCalledWith('cancel_hikarinagi_login', {
    flow_id: 'current',
  });
});
it('rejects an unexpected authorization host and releases the backend flow', async () => {
  mock.api
    .mockResolvedValueOnce({
      flow_id: 'current',
      authorization_url: 'https://evil.invalid/oidc/auth',
      expires_in_seconds: 180,
    })
    .mockResolvedValueOnce(true);
  expect(await beginHikariLogin()).toBeNull();
  expect(hikariAccount.busy).toBe(false);
  expect(hikariAccount.error).toContain('地址无效');
  expect(mock.api).toHaveBeenCalledWith('cancel_hikarinagi_login', {
    flow_id: 'current',
  });
});
