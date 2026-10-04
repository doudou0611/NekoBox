import { reactive } from 'vue';
import { api, desktop } from './library';
import { createOperation, updateOperation } from './operations';
import type { BackupStatus } from '../types/backup';
export const backupTasks = reactive({ state: null as BackupStatus | null });
let timer: ReturnType<typeof setInterval> | undefined;
let operation = '';
async function poll() {
  try {
    const state = await api('get_application_backup_status', {});
    backupTasks.state = state;
    if (state.running && !operation)
      operation = createOperation('应用备份与云任务', 'sync').id;
    if (operation) {
      updateOperation(operation, {
        status: state.running
          ? 'running'
          : state.cloud_status === 'failed'
            ? 'failed'
            : 'completed',
        message: state.message || state.phase,
      });
      if (!state.running) operation = '';
    }
  } catch {
    /* A failing background poll never pretends a task completed. */
  }
}
export function startBackupPolling() {
  if (desktop && !timer) {
    void poll();
    timer = setInterval(() => void poll(), 3000);
  }
}
export function stopBackupPolling() {
  clearInterval(timer);
  timer = undefined;
}
