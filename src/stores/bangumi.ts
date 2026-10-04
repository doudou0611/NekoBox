import { localizeAvatar } from '../services/remoteImages';
import { reactive } from 'vue';
import { api, desktop, errorText } from './library';
import type { BangumiAccount } from '../types/local';

export const bangumi = reactive({
  account: {
    status: 'signed_out',
    profile: null,
    message: '尚未登录 Bangumi。',
  } as BangumiAccount,
  busy: false,
  error: '',
  avatar_failed: false,
});
async function update(operation: () => Promise<BangumiAccount>) {
  if (bangumi.busy) return;
  bangumi.busy = true;
  bangumi.error = '';
  try {
    bangumi.account = await localizeAvatar(await operation());
    bangumi.avatar_failed = false;
  } catch (e) {
    bangumi.error = errorText(e);
  } finally {
    bangumi.busy = false;
  }
}
export async function refreshBangumi() {
  if (desktop) await update(() => api('bangumi_account', {}));
}
export async function loginBangumi(access_token: string) {
  if (desktop) await update(() => api('login_bangumi', { access_token }));
}
export async function logoutBangumi() {
  if (desktop) await update(() => api('logout_bangumi', {}));
}
