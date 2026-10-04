import { reactive } from 'vue';
import { api, desktop, errorText, refreshLibrary } from './library';
import { createOperation, updateOperation } from './operations';
import type { MetadataRefreshStatus } from '../types/backup';
export const metadataRefresh = reactive({
  state: null as MetadataRefreshStatus | null,
  error: '',
});
let timer: ReturnType<typeof setTimeout> | undefined;
let operationId = '';
export async function pollMetadataRefresh() {
  if (!desktop) return;
  try {
    metadataRefresh.state = await api('get_metadata_refresh', {});
    const state = metadataRefresh.state;
    if (state.status === 'running' && !operationId)
      operationId = createOperation('更新游戏元数据', 'scrape', state.total).id;
    if (operationId)
      updateOperation(operationId, {
        status:
          state.status === 'running'
            ? 'running'
            : state.status === 'cancelled'
              ? 'cancelled'
              : 'completed',
        progress: state.processed,
        total: state.total,
        message:
          state.current ??
          `已更新 ${state.succeeded}，跳过 ${state.skipped}，失败 ${state.failed}`,
      });
    clearTimeout(timer);
    if (state.status === 'running')
      timer = setTimeout(
        () => void pollMetadataRefresh(),
        document.hidden ? 5000 : 1000,
      );
    else if (operationId) {
      operationId = '';
      await refreshLibrary({ reloadDetails: true });
    }
  } catch (e) {
    metadataRefresh.error = errorText(e);
  }
}
export async function startMetadataRefresh() {
  metadataRefresh.error = '';
  metadataRefresh.state = await api('start_metadata_refresh', {
    confirmed: true,
  });
  void pollMetadataRefresh();
}
