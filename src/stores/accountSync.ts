import { reactive } from 'vue';
import { api, desktop, errorText, notify } from './library';
import { createOperation, updateOperation } from './operations';
import type { AccountSyncReport } from '../types/accounts';
type Provider = 'bangumi' | 'hikarinagi';
export const accountSync = reactive({
  busy: false,
  provider: null as Provider | null,
  results: {} as Partial<Record<Provider, AccountSyncReport>>,
  error: '',
  error_provider: null as Provider | null,
});
export async function syncAccount(provider: Provider) {
  if (!desktop || accountSync.busy) return;
  accountSync.busy = true;
  accountSync.provider = provider;
  accountSync.error = '';
  accountSync.error_provider = provider;
  delete accountSync.results[provider];
  const operation = createOperation(
    `同步 ${provider === 'bangumi' ? 'Bangumi' : 'Hikarinagi'} 游玩数据`,
    'sync',
  );
  updateOperation(operation.id, {
    status: 'running',
    message: '正在逐项上传已绑定作品的状态与评分',
  });
  try {
    const result = await api('sync_account_play_data', {
      provider,
      confirmed: true,
    });
    accountSync.results[provider] = result;
    updateOperation(operation.id, {
      status: result.failed ? 'failed' : 'completed',
      progress: result.processed,
      total: result.processed,
      message: `成功 ${result.synced} 项，失败 ${result.failed} 项，跳过 ${result.skipped} 项`,
    });
    notify(`游玩同步：成功 ${result.synced} 项，失败 ${result.failed} 项。`);
  } catch (cause) {
    accountSync.error = errorText(cause);
    updateOperation(operation.id, {
      status: 'failed',
      message: accountSync.error,
    });
  } finally {
    accountSync.busy = false;
    accountSync.provider = null;
  }
}
