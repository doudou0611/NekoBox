import { computed, reactive } from 'vue';
import {
  api,
  desktop,
  errorText,
  local,
  preview,
  refreshLibrary,
  loadDetail,
} from './library';
import { automaticOwnedCandidate } from '../services/metadataScraper';
import { createOperation, operations, updateOperation } from './operations';
import type { MetadataCandidate } from '../types/domain';

export type OwnedMetadataStatus =
  | 'queued'
  | 'searching'
  | 'applying'
  | 'completed'
  | 'no_match'
  | 'failed'
  | 'skipped'
  | 'cancelled';
export interface OwnedMetadataItem {
  game_id: string;
  title: string;
  cover_url: string;
  status: OwnedMetadataStatus;
  message: string;
  provider: string | null;
}
export const ownedMetadata = reactive({
  open: false,
  running: false,
  stopping: false,
  items: [] as OwnedMetadataItem[],
  operation_id: '',
});
const active = (s: OwnedMetadataStatus) =>
  ['queued', 'searching', 'applying'].includes(s);
export const ownedMetadataStats = computed(() => ({
  total: ownedMetadata.items.length,
  processed: ownedMetadata.items.filter((i) => !active(i.status)).length,
  completed: ownedMetadata.items.filter((i) => i.status === 'completed').length,
  attention: ownedMetadata.items.filter((i) =>
    ['no_match', 'failed', 'cancelled'].includes(i.status),
  ).length,
}));
let controller: AbortController | null = null;

export function openOwnedMetadata() {
  ownedMetadata.open = true;
}
function progress(message?: string) {
  const stats = ownedMetadataStats.value;
  updateOperation(ownedMetadata.operation_id, {
    status: ownedMetadata.running
      ? 'running'
      : ownedMetadata.stopping
        ? 'cancelled'
        : ownedMetadata.items.some((i) => i.status === 'failed')
          ? 'failed'
          : 'completed',
    progress: stats.processed,
    total: stats.total,
    message:
      message ?? `已补充 ${stats.completed} 部 · 待处理 ${stats.attention} 部`,
  });
}
export function stopOwnedMetadata() {
  if (!ownedMetadata.running) return;
  ownedMetadata.stopping = true;
  controller?.abort();
  for (const item of ownedMetadata.items) {
    if (item.status === 'queued') {
      item.status = 'cancelled';
      item.message = '已停止，可稍后重试';
    }
  }
  progress('正在停止；已开始保存的当前作品会先完成。');
}
/** Ownership sync never opens the local import wizard or repeats unresolved work in this session. */
export function queueOwnedMetadata() {
  if (!desktop || ownedMetadata.stopping) return;
  for (const game of preview.games) {
    if (
      !game.hikari_field ||
      local.records[game.game_id]?.metadata_status === 'synced' ||
      ownedMetadata.items.some((i) => i.game_id === game.game_id)
    )
      continue;
    ownedMetadata.items.push({
      game_id: game.game_id,
      title: game.title,
      cover_url: game.cover_url,
      status: 'queued',
      message: '等待补充资料',
      provider: null,
    });
  }
  void runOwnedMetadata();
}
export function retryOwnedMetadata() {
  if (ownedMetadata.running) return;
  for (const item of ownedMetadata.items) {
    if (['failed', 'cancelled', 'no_match'].includes(item.status)) {
      item.status = 'queued';
      item.provider = null;
      item.message = '等待自动重试';
    }
  }
  void runOwnedMetadata();
}
async function processItem(item: OwnedMetadataItem) {
  item.status = 'searching';
  item.message = '正在检查作品资料';
  progress(`${item.title} · ${item.message}`);
  const game = await api('get_game', { game_id: item.game_id });
  if (ownedMetadata.stopping) return;
  if (game.metadata_locked) {
    item.status = 'skipped';
    item.message = '资料已锁定，保留现有内容';
    return;
  }
  // Another window may have bound this game since it entered the queue.
  // Automatic supplementation must not replace that identity or its source order.
  if (
    game.metadata.some(
      (f) => f.remote_id && !['manual', 'hikarifield'].includes(f.provider),
    )
  ) {
    item.status = 'skipped';
    item.message = '已有来源资料，可在游戏详情中重新刮削';
    return;
  }
  let query = item.title;
  const official = game.metadata.find(
    (f) => f.provider === 'hikarifield' && f.field === 'title',
  );
  if (official) {
    try {
      const value: unknown = JSON.parse(official.value);
      if (typeof value === 'string' && value.trim() && value.length <= 200)
        query = value.trim();
    } catch {
      /* Historical metadata without a string value falls back to the display name. */
    }
  }
  let candidate: MetadataCandidate | null = null;
  const config = await api('get_metadata_sources', {});
  if (ownedMetadata.stopping) return;
  const failures: string[] = [];
  let ambiguous = false;
  for (const source of config.sources.filter((s) => s.enabled)) {
    item.provider = source.provider;
    item.message = `正在搜索 ${source.provider}`;
    progress(`${item.title} · ${item.message}`);
    const search = new AbortController();
    controller = search;
    try {
      const found = await api(
        'search_metadata',
        {
          query,
          providers: [source.provider],
          manual: false,
        },
        { signal: search.signal },
      );
      if (ownedMetadata.stopping) return;
      candidate = automaticOwnedCandidate(query, found);
      ambiguous ||= found.length > 0 && !candidate;
      if (candidate) break;
    } catch (e) {
      if (ownedMetadata.stopping) return;
      failures.push(`${source.provider}：${errorText(e)}`);
    } finally {
      controller = null;
    }
  }
  if (!candidate) {
    item.status = failures.length ? 'failed' : 'no_match';
    item.message =
      failures.join('；') ||
      (ambiguous
        ? '未找到唯一匹配，已保留原有资料并继续处理其他游戏。'
        : '当前来源未找到对应作品，已保留官方资料。');
    return;
  }
  item.status = 'applying';
  item.provider = candidate.provider;
  item.message = '正在补充简介、开发商、日期与来源资料';
  progress(`${item.title} · ${item.message}`);
  // A confirmation stages and commits metadata atomically in the backend.
  // Do not abort its Promise: stopping finishes this commit, then stops the queue.
  const match = await api('confirm_metadata_match', {
    game_id: item.game_id,
    title_hint: candidate.title,
    provider: candidate.provider,
    remote_id: candidate.remote_id,
    manual: false,
  });
  item.status = 'completed';
  item.message =
    [match.cover_message, match.supplementation_message]
      .filter(Boolean)
      .join('；') || '资料已补充，个人记录与安装位置已保留';
  try {
    await loadDetail(item.game_id);
    await refreshLibrary({ reloadDetails: true });
  } catch {
    item.message += '；资料已保存，刷新游戏库后可查看。';
  }
}
async function runOwnedMetadata() {
  if (
    ownedMetadata.running ||
    !ownedMetadata.items.some((i) => i.status === 'queued')
  )
    return;
  ownedMetadata.running = true;
  ownedMetadata.stopping = false;
  let operation = operations.find((i) => i.id === ownedMetadata.operation_id);
  if (!operation) {
    operation = createOperation(
      '已购游戏 · 自动补全',
      'scrape',
      ownedMetadata.items.length,
      openOwnedMetadata,
    );
    ownedMetadata.operation_id = operation.id;
  }
  operation.completed_at = null;
  operation.details_label = '查看补全进度';
  operation.cancel = stopOwnedMetadata;
  operation.retry = undefined;
  try {
    while (!ownedMetadata.stopping) {
      const item = ownedMetadata.items.find((i) => i.status === 'queued');
      if (!item) break;
      try {
        await processItem(item);
      } catch (e) {
        item.status = ownedMetadata.stopping ? 'cancelled' : 'failed';
        item.message = ownedMetadata.stopping
          ? '已停止，可稍后重试'
          : errorText(e);
      }
      if (ownedMetadata.stopping && active(item.status)) {
        item.status = 'cancelled';
        item.message = '已停止，可稍后重试';
      }
      progress();
    }
  } finally {
    ownedMetadata.running = false;
    operation.cancel = undefined;
    if (
      ownedMetadata.items.some((i) =>
        ['failed', 'cancelled', 'no_match'].includes(i.status),
      )
    )
      operation.retry = retryOwnedMetadata;
    progress();
    ownedMetadata.stopping = false;
  }
}
