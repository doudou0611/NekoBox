import { reactive } from 'vue';
import { version } from '../../package.json';
import { api, desktop, errorText } from './library';
import type { AppUpdateStatus } from '../types/updates';

export const updates = reactive({
  status: {
    phase: 'idle',
    current_version: version,
    architecture: 'unsupported',
    installation: 'unsupported',
    version: null,
    notes: '',
    release_url: 'https://github.com/doudou0611/NekoBox/releases/latest',
    download_url: null,
    signed: false,
    downloaded: 0,
    total: null,
    message: '启动时自动检查，也可以手动检查更新。',
  } as AppUpdateStatus,
  busy: false,
  error: '',
  notice: false,
});
let startupChecked = false;
// The backend bounds each HTTP request; also bound the whole IPC operation so
// a stalled service cannot leave the update controls busy indefinitely.
export const UPDATE_CHECK_TIMEOUT_MS = 45_000;

export async function refreshUpdateStatus() {
  if (!desktop) return;
  updates.status = await api('get_app_update_status', {});
}
export async function checkAppUpdate() {
  if (
    !desktop ||
    updates.busy ||
    ['downloading', 'ready', 'installing'].includes(updates.status.phase)
  )
    return;
  updates.busy = true;
  updates.error = '';
  updates.notice = false;
  updates.status.phase = 'checking';
  updates.status.message = '正在连接 GitHub…';
  let timer: ReturnType<typeof setTimeout> | undefined;
  try {
    updates.status = await Promise.race([
      api('check_app_update', {}),
      new Promise<never>((_, reject) => {
        timer = setTimeout(
          () => reject(new Error('更新检查暂时超时，可以稍后在设置中重试。')),
          UPDATE_CHECK_TIMEOUT_MS,
        );
      }),
    ]);
    updates.notice = updates.status.phase === 'available';
  } catch (cause) {
    updates.error = errorText(cause);
    updates.status.phase = 'error';
    updates.status.message = updates.error;
  } finally {
    clearTimeout(timer);
    updates.busy = false;
  }
}
export async function checkStartupUpdate() {
  if (startupChecked) return;
  startupChecked = true;
  await checkAppUpdate();
}
export async function downloadAppUpdate() {
  if (
    !desktop ||
    updates.busy ||
    !updates.status.signed ||
    updates.status.phase !== 'available'
  )
    return;
  updates.busy = true;
  updates.error = '';
  updates.status.phase = 'downloading';
  let polling = false;
  let finished = false;
  const timer = setInterval(async () => {
    if (polling) return;
    polling = true;
    try {
      const status = await api('get_app_update_status', {});
      if (!finished) updates.status = status;
    } catch {
      /* The download command supplies the final error. */
    } finally {
      polling = false;
    }
  }, 500);
  try {
    const status = await api('download_app_update', {});
    finished = true;
    updates.status = status;
  } catch (cause) {
    finished = true;
    updates.error = errorText(cause);
    try {
      await refreshUpdateStatus();
    } catch {
      updates.status.phase = 'available';
    }
  } finally {
    clearInterval(timer);
    updates.busy = false;
  }
}
export async function installAppUpdate() {
  if (!desktop || updates.busy || updates.status.phase !== 'ready') return;
  updates.busy = true;
  updates.error = '';
  try {
    updates.status = await api('install_app_update', { confirmed: true });
  } catch (cause) {
    updates.error = errorText(cause);
  } finally {
    updates.busy = false;
  }
}
export async function openUpdateRelease(download = false) {
  if (!desktop) return;
  try {
    await api('open_app_update_release', { download });
  } catch (cause) {
    updates.error = errorText(cause);
  }
}
