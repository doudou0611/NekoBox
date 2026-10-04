import { localizeAvatar } from '../services/remoteImages';
import { reactive } from 'vue';
import { api, desktop, errorText } from './library';
import type { HikarinagiAccount } from '../types/accounts';
export const hikariAccount = reactive({
  account: {
    status: 'signed_out',
    profile: null,
    message: '登录自己的账户以同步游玩数据；未登录也可刮削。',
  } as HikarinagiAccount,
  busy: false,
  error: '',
  avatar_failed: false,
  flow_id: '',
  login_message: '',
});
let timer: ReturnType<typeof setTimeout> | undefined;
let deadline = 0;
async function update(operation: () => Promise<HikarinagiAccount>) {
  if (!desktop || hikariAccount.busy) return;
  hikariAccount.busy = true;
  hikariAccount.error = '';
  try {
    hikariAccount.account = await localizeAvatar(await operation());
    hikariAccount.avatar_failed = false;
  } catch (cause) {
    hikariAccount.error = errorText(cause);
  } finally {
    hikariAccount.busy = false;
  }
}
export async function refreshHikariAccount() {
  await update(() => api('hikarinagi_account', {}));
}
export async function logoutHikariAccount() {
  await cancelHikariLogin();
  await update(() => api('logout_hikarinagi', {}));
}
async function poll(flow_id: string) {
  if (hikariAccount.flow_id !== flow_id) return;
  try {
    if (Date.now() > deadline) {
      await cancelHikariLogin();
      hikariAccount.error = '登录超时，请重新打开官方授权。';
      return;
    }
    const state = await api('poll_hikarinagi_login', { flow_id });
    if (hikariAccount.flow_id !== flow_id) return;
    hikariAccount.login_message = state.message;
    if (state.status === 'pending') {
      timer = setTimeout(() => void poll(flow_id), 900);
      return;
    }
    hikariAccount.flow_id = '';
    hikariAccount.busy = false;
    if (state.status === 'completed' && state.account) {
      hikariAccount.account = await localizeAvatar(state.account);
      hikariAccount.avatar_failed = false;
    } else if (state.status === 'failed') {
      hikariAccount.login_message = '';
      hikariAccount.error = state.message;
    }
  } catch (cause) {
    if (hikariAccount.flow_id === flow_id) {
      await cancelHikariLogin();
      hikariAccount.error = errorText(cause);
    }
  }
}
export async function beginHikariLogin(): Promise<string | null> {
  if (!desktop || hikariAccount.busy) return null;
  hikariAccount.busy = true;
  hikariAccount.error = '';
  hikariAccount.login_message = '正在打开官方授权…';
  try {
    const result = await api('begin_hikarinagi_login', {});
    hikariAccount.flow_id = result.flow_id;
    const url = new URL(result.authorization_url);
    if (
      url.protocol !== 'https:' ||
      url.hostname !== 'id.hikarinagi.org' ||
      url.pathname !== '/oidc/auth' ||
      url.username ||
      url.password
    )
      throw new Error('官方授权地址无效。');
    deadline = Date.now() + result.expires_in_seconds * 1000;
    timer = setTimeout(() => void poll(result.flow_id), 900);
    return result.authorization_url;
  } catch (cause) {
    await cancelHikariLogin();
    hikariAccount.busy = false;
    hikariAccount.error = errorText(cause);
    return null;
  }
}
export async function cancelHikariLogin() {
  if (timer) clearTimeout(timer);
  const flow_id = hikariAccount.flow_id;
  hikariAccount.flow_id = '';
  if (flow_id) {
    await api('cancel_hikarinagi_login', { flow_id }).catch(() => {});
    hikariAccount.busy = false;
    hikariAccount.login_message = '已取消登录。';
  }
}
