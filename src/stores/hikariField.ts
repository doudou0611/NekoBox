import { reactive } from 'vue';
import { open } from '@tauri-apps/plugin-dialog';
import { api, desktop, errorText, refreshLibrary, notify } from './library';
import { openAccountDialog } from './accountDialog';
import { operations, type OperationItem } from './operations';
import { queueOwnedMetadata } from './ownedMetadata';
import type {
  HikariFieldAccount,
  HikariFieldDownload,
  HikariFieldSettings,
} from '../types/hikarifield';
export const hikariField = reactive({
  account: {
    status: 'signed_out',
    profile: null,
    message: '登录后自动导入你已拥有的游戏。',
  } as HikariFieldAccount,
  settings: { root: null, uuid: '' } as HikariFieldSettings,
  tasks: [] as HikariFieldDownload[],
  busy: false,
  error: '',
  sync_message: '',
  loaded: false,
  starting: [] as string[],
  folder_game: '',
  folder_parent: '',
  folder_busy: false,
  folder_error: '',
});
let timer: ReturnType<typeof setTimeout> | undefined;
let disposed = false;
let polling = false;
export function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  const units = ['KB', 'MB', 'GB', 'TB'];
  let size = bytes / 1024;
  let index = 0;
  while (size >= 1024 && index < units.length - 1) {
    size /= 1024;
    index++;
  }
  return `${size.toFixed(size >= 100 ? 0 : 1)} ${units[index]}`;
}
export function gameDownload(game_id: string) {
  return hikariField.tasks.find(
    (t) => t.game_id === game_id && ['running', 'queued'].includes(t.status),
  );
}
export async function loadHikariField() {
  if (!desktop) return;
  try {
    const [account, settings] = await Promise.all([
      api('hikarifield_account', {}),
      api('get_hikarifield_settings', {}),
    ]);
    hikariField.account = account;
    hikariField.settings = settings;
    hikariField.loaded = true;
  } catch (e) {
    hikariField.error = errorText(e);
  }
}
export async function syncHikariField() {
  hikariField.error = '';
  hikariField.sync_message = '正在读取已拥有的游戏…';
  try {
    const report = await api('sync_hikarifield', {});
    hikariField.sync_message = `已同步 ${report.owned} 部已拥有的游戏，新增 ${report.imported} 部。`;
    await refreshLibrary({ reloadDetails: true });
    queueOwnedMetadata();
  } catch (e) {
    hikariField.sync_message = '';
    hikariField.error = errorText(e);
  }
}
export async function loginHikariField(email: string, password: string) {
  if (!desktop || hikariField.busy) return;
  hikariField.busy = true;
  hikariField.error = '';
  try {
    hikariField.account = await api('login_hikarifield', { email, password });
    password = '';
    await syncHikariField();
  } catch (e) {
    hikariField.error = errorText(e);
  } finally {
    hikariField.busy = false;
  }
}
export async function refreshOwnedHikariField() {
  if (!desktop || hikariField.busy) return;
  hikariField.busy = true;
  try {
    await syncHikariField();
  } finally {
    hikariField.busy = false;
  }
}
export async function logoutHikariField() {
  if (!desktop || hikariField.busy) return;
  hikariField.busy = true;
  hikariField.error = '';
  try {
    hikariField.account = await api('logout_hikarifield', {});
    hikariField.sync_message = '';
  } catch (e) {
    hikariField.error = errorText(e);
  } finally {
    hikariField.busy = false;
  }
}
export async function chooseHikariFolder(): Promise<string | null> {
  const result = await open({
    directory: true,
    multiple: false,
    title: '选择 HikariFieldGames 的存放位置',
    defaultPath: hikariField.settings.root || undefined,
  });
  return typeof result === 'string' ? result : null;
}
export async function changeHikariFolder() {
  if (!desktop || hikariField.folder_busy) return;
  hikariField.folder_busy = true;
  hikariField.error = '';
  try {
    const parent = await chooseHikariFolder();
    if (parent)
      hikariField.settings = await api('set_hikarifield_path', { parent });
  } catch (e) {
    hikariField.error = errorText(e);
  } finally {
    hikariField.folder_busy = false;
  }
}
export async function startHikariDownload(game_id: string) {
  if (
    !desktop ||
    gameDownload(game_id) ||
    hikariField.starting.includes(game_id)
  )
    return;
  if (hikariField.account.status !== 'authenticated') {
    openAccountDialog('hikarifield');
    return;
  }
  if (!hikariField.loaded) await loadHikariField();
  if (!hikariField.settings.root) {
    hikariField.folder_game = game_id;
    hikariField.folder_parent = '';
    hikariField.folder_error = '';
    return;
  }
  hikariField.starting.push(game_id);
  try {
    const task = await api('start_hikarifield_download', { game_id });
    // Capture the native queued state before polling: an already downloaded
    // official installation can finish verification before the first response.
    hikariField.tasks.unshift(task);
    await pollHikariDownloads();
  } catch (e) {
    notify(errorText(e));
  } finally {
    hikariField.starting = hikariField.starting.filter((id) => id !== game_id);
  }
}
export async function confirmHikariFolder() {
  if (!hikariField.folder_parent || hikariField.folder_busy) return;
  hikariField.folder_busy = true;
  hikariField.folder_error = '';
  try {
    const id = hikariField.folder_game;
    hikariField.settings = await api('set_hikarifield_path', {
      parent: hikariField.folder_parent,
    });
    hikariField.folder_game = '';
    await startHikariDownload(id);
  } catch (e) {
    hikariField.folder_error = errorText(e);
  } finally {
    hikariField.folder_busy = false;
  }
}
export async function cancelHikariDownload(task_id: string) {
  try {
    await api('cancel_hikarifield_download', { task_id });
    await pollHikariDownloads();
  } catch (e) {
    notify(errorText(e));
  }
}
export async function pollHikariDownloads() {
  if (!desktop || polling) return;
  polling = true;
  try {
    const previous = new Map(hikariField.tasks.map((t) => [t.id, t.status]));
    const tasks = await api('list_hikarifield_downloads', {});
    hikariField.tasks = tasks;
    let completed = false;
    for (const task of tasks) {
      const id = `hikarifield-${task.id}`;
      const item = operations.find((o) => o.id === id);
      // Erasing a terminal task stays erased for this app session.
      if (
        !item &&
        ['completed', 'failed', 'cancelled'].includes(
          previous.get(task.id) || '',
        )
      )
        continue;
      const patch: OperationItem = {
        id,
        kind: 'download',
        title: task.title,
        status: task.status,
        progress: task.downloaded,
        total: task.total || null,
        progress_unit: 'bytes',
        message: `${task.message}${task.speed ? ` · ${formatBytes(task.speed)}/s` : ''}`,
        created_at: Date.now(),
        completed_at: ['queued', 'running'].includes(task.status)
          ? null
          : Date.now(),
        open_details: null,
        cancel: ['queued', 'running'].includes(task.status)
          ? () => void cancelHikariDownload(task.id)
          : undefined,
        retry: ['failed', 'cancelled'].includes(task.status)
          ? () => void startHikariDownload(task.game_id)
          : undefined,
      };
      if (item) Object.assign(item, patch);
      else operations.unshift(patch);
      if (
        task.status === 'completed' &&
        previous.has(task.id) &&
        previous.get(task.id) !== 'completed'
      ) {
        completed = true;
        notify(`${task.title} 已安装，可以启动游戏。`);
      }
    }
    if (completed) await refreshLibrary({ reloadDetails: true });
  } catch {
    /* The next bounded poll retries; native tasks continue independently. */
  } finally {
    polling = false;
  }
}
export function startHikariPolling() {
  disposed = false;
  const tick = async () => {
    await pollHikariDownloads();
    if (!disposed) timer = setTimeout(() => void tick(), 1000);
  };
  void tick();
}
export function stopHikariPolling() {
  disposed = true;
  clearTimeout(timer);
}
