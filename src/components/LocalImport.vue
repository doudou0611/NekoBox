<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from 'vue';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { open } from '@tauri-apps/plugin-dialog';
import { isScanActive, scanNotification } from '../stores/scanFeedback';
import {
  desktop,
  local,
  api,
  errorText,
  refreshLibrary,
  notify,
} from '../stores/library';
import type { MetadataCandidate } from '../types/domain';
import type {
  ExecutableCandidate,
  ImportPreviewCandidate,
} from '../types/local';
import PreviewIcon from './preview/PreviewIcon.vue';
import PreviewCover from './preview/PreviewCover.vue';
import { createOperation, updateOperation } from '../stores/operations';
import {
  isPrepared,
  isImported,
  importReviewStats,
  needsScrape,
  prepareReview,
  selectReviewMatch,
} from '../services/importReview';
import type { ImportPreparation } from '../types/importPreparation';
import { refreshHikarinagi } from '../stores/hikarinagi';
import {
  metadataSources,
  loadMetadataSources,
} from '../stores/metadataSources';
import {
  automaticCandidate,
  type MetadataProvider,
} from '../services/metadataScraper';

type ImportStage = 'choice' | 'review';
type ImportKind = 'single' | 'directory';
interface ReviewItem {
  install_path: string;
  selected_executable: string;
  search_name: string;
  preview: ImportPreviewCandidate | null;
  selected: boolean;
  edited_name: boolean;
  scrape_status:
    | 'pending'
    | 'matched'
    | 'scraping'
    | 'scraped'
    | 'manual'
    | 'no_match'
    | 'failed';
  scrape_message: string;
  match: MetadataCandidate | null;
  preparation: ImportPreparation | null;
  manual_match: boolean;
}
interface NativeDropPayload {
  paths?: string[];
}

const dialog = ref<HTMLDialogElement>();
const manual_dialog = ref<HTMLDialogElement>();
const single_dialog = ref<HTMLDialogElement>();
const single_item = computed(() => review_items.value[0]);
const single_import = computed(
  () => kind.value === 'single' && stage.value === 'review',
);
const stage = ref<ImportStage>('choice');
const kind = ref<ImportKind>('directory');
const roots = ref<string[]>([]);
const review_items = ref<ReviewItem[]>([]);
const review_loading = ref(false);
const busy = ref(false);
const error = ref('');
const drop_active = ref(false);
const background_scrape = ref(false);
const scrape_running = ref(false);
const scrape_operation = ref('');
const batch_id = ref('');
const batch_ids = new Set<string>();
const cancelling = ref(false);
let scrape_generation = 0;
let taskProviders: MetadataProvider[] = [];
const manual_item = ref<ReviewItem | null>(null);
const manual_provider = ref<'vndb' | 'bangumi' | 'hikarinagi'>('bangumi');
const manual_query = ref('');
const manual_candidates = ref<MetadataCandidate[]>([]);
const manual_loading = ref(false);
let timer: ReturnType<typeof setInterval> | undefined;
let polling = false;
let disposed = false;
let native_drop_unlisten: UnlistenFn | undefined;
let manual_search_sequence = 0;
let manual_search_controller: AbortController | null = null;
function cancelManualSearch() {
  manual_search_sequence++;
  manual_search_controller?.abort();
  manual_search_controller = null;
  manual_loading.value = false;
}

const review_stats = computed(() => importReviewStats(review_items.value));
const scraped_count = computed(() => review_stats.value.scraped);
const pending_count = computed(() => review_stats.value.pending);
const selected_count = computed(() => review_stats.value.selected);
const review_progress = computed(() => {
  const selected = review_items.value.filter(
    (item) => item.selected && !isImported(item),
  );
  const processed = selected.filter((item) =>
    ['scraped', 'manual', 'failed', 'no_match'].includes(item.scrape_status),
  ).length;
  return {
    processed,
    total: selected.length,
    percentage: selected.length
      ? Math.round((processed / selected.length) * 100)
      : 0,
  };
});
const current_scrape = computed(() =>
  review_items.value.find(
    (item) => !isImported(item) && item.scrape_status === 'scraping',
  ),
);

const unresolved_count = computed(
  () =>
    review_items.value.filter(
      (item) =>
        item.selected &&
        !item.preview?.existing_game_id &&
        executableCandidates(item).length > 1 &&
        !item.selected_executable,
    ).length,
);

function resetReview() {
  cancelManualSearch();
  discardPreparations(review_items.value);
  batch_id.value = '';
  for (const id of batch_ids)
    void api('cancel_import_batch', { batch_id: id }).catch((cause) =>
      notify(errorText(cause)),
    );
  batch_ids.clear();
  stage.value = 'choice';
  kind.value = 'directory';
  roots.value = [];
  review_items.value = [];
  review_loading.value = false;
  busy.value = false;
  error.value = '';
  background_scrape.value = false;
  manual_item.value = null;
  manual_candidates.value = [];
}
function closeImport() {
  manual_item.value = null;
  local.import_open = false;
}
function discardPreparations(items: ReviewItem[]) {
  const preparation_ids = items.flatMap((item) =>
    item.preparation ? [item.preparation.preparation_id] : [],
  );
  if (preparation_ids.length)
    void api('discard_import_metadata', { preparation_ids }).catch(() => {});
}
async function cancelImport() {
  if (busy.value && !scrape_running.value) return;
  if (cancelling.value) return;
  const cancelledBatches = [...batch_ids];
  cancelling.value = true;
  cancelManualSearch();
  scrape_generation += 1;
  scrape_running.value = false;
  updateOperation(scrape_operation.value, {
    status: 'cancelled',
    message: '已取消本轮导入，正在清理下载缓存',
  });
  busy.value = true;
  try {
    for (const id of cancelledBatches)
      await api('cancel_import_batch', { batch_id: id });
  } catch (cause) {
    error.value = errorText(cause);
    for (const item of review_items.value) {
      item.preparation = null;
      item.scrape_status = 'pending';
      item.scrape_message = '本轮已取消，请重新开始刮削';
    }
    updateOperation(scrape_operation.value, {
      status: 'cancelled',
      message: `本轮已取消；缓存清理未完成：${error.value}`,
    });
    busy.value = false;
    cancelling.value = false;
    return;
  }
  batch_ids.clear();
  cancelling.value = false;
  busy.value = false;
  batch_id.value = '';
  updateOperation(scrape_operation.value, {
    status: 'cancelled',
    message: '已取消；本轮新下载且未被其他作品或任务引用的缓存已清理',
  });
  closeImport();
  resetReview();
}
function resetMatch(item: ReviewItem) {
  discardPreparations([item]);
  item.edited_name = true;
  item.match = null;
  item.preparation = null;
  item.manual_match = false;
  item.scrape_status = 'pending';
  item.scrape_message = '';
}
watch(
  () => local.import_open,
  (is_open) => {
    if (is_open) {
      void refreshHikarinagi();
      void loadMetadataSources().catch(
        (cause) => (error.value = errorText(cause)),
      );
      if (!review_items.value.length && !background_scrape.value) resetReview();
      void nextTick(() => {
        if (!single_import.value && dialog.value && !dialog.value.open)
          dialog.value.showModal();
      });
    } else if (dialog.value?.open) dialog.value.close();
  },
);

watch(manual_item, (item) => {
  if (!item) cancelManualSearch();
  if (item && !single_import.value)
    void nextTick(() => {
      if (manual_dialog.value && !manual_dialog.value.open && manual_item.value)
        manual_dialog.value.showModal();
    });
  else if (manual_dialog.value?.open) manual_dialog.value.close();
});

watch(
  () => [local.import_open, single_import.value],
  async () => {
    await nextTick();
    if (local.import_open && single_import.value) {
      dialog.value?.close();
      if (!single_dialog.value?.open) single_dialog.value?.showModal();
    } else single_dialog.value?.close();
  },
);

function parentPath(path: string) {
  const slash = Math.max(path.lastIndexOf('/'), path.lastIndexOf('\\'));
  return slash > 0 ? path.slice(0, slash) : path;
}
function isExecutable(path: string) {
  return /\.exe$/i.test(path);
}
function metadataLabel(status: ReviewItem['scrape_status']) {
  return {
    pending: '待刮削',
    matched: '已匹配 · 待刮削',
    scraping: '刮削中',
    scraped: '已刮削',
    manual: '已刮削 · 手动',
    no_match: '无匹配 · 本地导入可用',
    failed: '刮削失败',
  }[status];
}
function isScraped(item: ReviewItem) {
  return isPrepared(item);
}
function makePreviewItem(candidate: ImportPreviewCandidate): ReviewItem {
  return {
    install_path: candidate.directory,
    selected_executable: candidate.selected_executable ?? '',
    search_name: candidate.search_name,
    preview: candidate,
    selected: !candidate.existing_game_id,
    edited_name: false,
    scrape_status: 'pending',
    scrape_message: '',
    match: null,
    preparation: null,
    manual_match: false,
  };
}
function executableCandidates(item: ReviewItem): ExecutableCandidate[] {
  return item.preview?.executables ?? [];
}

async function startDirectoryImport(selected_roots: string[]) {
  discardPreparations(review_items.value);
  stage.value = 'review';
  kind.value = 'directory';
  roots.value = selected_roots;
  review_items.value = [];
  review_loading.value = true;
  error.value = '';
  if (!desktop) {
    review_loading.value = false;
    error.value = '浏览器预览不能读取真实目录，请在 Windows 桌面版本中选择。';
    return;
  }
  try {
    const preview = await api('preview_import', {
      roots: selected_roots,
      follow_symlinks: false,
    });
    review_items.value = preview.items.map(makePreviewItem);
    if (preview.issue_count)
      notify(
        `目录预览完成：发现 ${preview.items.length} 个候选，${preview.issue_count} 个读取问题。`,
      );
    review_loading.value = false;
  } catch (cause) {
    review_loading.value = false;
    error.value = errorText(cause);
  }
}

async function startSingleImport(directory: string, executable_path?: string) {
  resetReview();
  kind.value = 'single';
  stage.value = 'review';
  roots.value = [directory];
  review_loading.value = true;
  const run = ++scrape_generation;
  try {
    await loadMetadataSources();
    if (run !== scrape_generation || !local.import_open) return;
    manual_provider.value =
      metadataSources.sources[0]?.provider ?? 'hikarinagi';
    const preview = await api('preview_import', {
      roots: [directory],
      follow_symlinks: false,
      single_directory: !executable_path,
      ...(executable_path ? { single_executable: executable_path } : {}),
    });
    if (run !== scrape_generation || !local.import_open) return;
    if (preview.items.length !== 1)
      throw new Error('所选文件夹没有可导入的游戏文件，请重新选择游戏文件夹。');
    const item = makePreviewItem(preview.items[0]!);
    if (executable_path) item.selected_executable = executable_path;
    review_items.value = [item];
    if (item.preview?.existing_game_id)
      throw new Error('该游戏文件夹已在库中，请到游戏详情管理资料。');
    const started = await api('begin_import_batch', {});
    if (run !== scrape_generation || !local.import_open) {
      await api('cancel_import_batch', { batch_id: started.batch_id });
      return;
    }
    batch_id.value = started.batch_id;
    batch_ids.add(started.batch_id);
    manual_item.value = item;
    manual_query.value = item.preview?.folder_name || item.search_name;
    review_loading.value = false;
    await searchManual();
  } catch (cause) {
    if (run === scrape_generation) error.value = errorText(cause);
  } finally {
    if (run === scrape_generation) review_loading.value = false;
  }
}
async function chooseSingleDirectory() {
  error.value = '';
  if (!desktop) {
    error.value = '浏览器预览不能调用资源管理器，请在 Windows 桌面版本中选择。';
    return;
  }
  try {
    const result = await open({
      directory: true,
      multiple: false,
      title: '选择需要导入的游戏文件夹',
    });
    const path = Array.isArray(result) ? result[0] : result;
    if (typeof path === 'string') await startSingleImport(path);
  } catch (cause) {
    error.value = errorText(cause);
  }
}
function chooseSingleMatch(candidate: MetadataCandidate) {
  const item = single_item.value;
  if (!item || busy.value) return;
  if (
    item.match?.provider === candidate.provider &&
    item.match?.remote_id === candidate.remote_id
  )
    return;
  discardPreparations([item]);
  selectReviewMatch(item, candidate);
}
async function scrapeSingleMatch(candidate: MetadataCandidate) {
  if (busy.value || review_loading.value || manual_loading.value) return;
  chooseSingleMatch(candidate);
  await confirmSingleImport();
}
async function changeSingleSource() {
  cancelManualSearch();
  if (single_item.value) resetMatch(single_item.value);
  manual_candidates.value = [];
  await searchManual();
}
async function confirmSingleImport() {
  const item = single_item.value;
  if (
    !item ||
    !item.match ||
    isImported(item) ||
    busy.value ||
    review_loading.value ||
    manual_loading.value
  )
    return;
  if (unresolved_count.value) {
    error.value = '请先选择这个游戏的启动入口。';
    return;
  }
  const run = scrape_generation;
  error.value = '';
  busy.value = true;
  try {
    if (!isPrepared(item)) await prepareItem(item, run);
  } catch (cause) {
    if (run === scrape_generation) error.value = errorText(cause);
    busy.value = false;
    return;
  }
  busy.value = false;
  if (run === scrape_generation) await confirmReview();
}

async function chooseDirectory() {
  error.value = '';
  if (!desktop) {
    error.value = '浏览器预览不能调用资源管理器，请在 Windows 桌面版本中选择。';
    return;
  }
  try {
    const result = await open({
      directory: true,
      multiple: true,
      title: '选择 Galgame 游戏目录（可多选）',
    });
    const selected = Array.isArray(result) ? result : result ? [result] : [];
    if (selected.length) await startDirectoryImport(selected);
  } catch (cause) {
    error.value = errorText(cause);
  }
}
function returnToChoice() {
  resetReview();
}
async function scrapeItem(item: ReviewItem, run: number) {
  const check = () => {
    if (run !== scrape_generation) throw new Error('本轮导入已取消。');
  };
  item.scrape_status = 'scraping';
  item.scrape_message = '';
  const pinned = item.manual_match ? item.match : null;
  let last_error: unknown = null;
  if (pinned) {
    check();
    try {
      await prepareItem(item, run);
      check();
      return;
    } catch (cause) {
      check();
      last_error = cause;
    }
  }
  for (const provider of taskProviders.filter((p) => p !== pinned?.provider)) {
    check();
    try {
      item.scrape_message = `正在搜索 ${provider}`;
      updateOperation(scrape_operation.value, {
        message: `${item.search_name} · ${item.scrape_message}`,
      });
      const candidates = await api('search_metadata', {
        query: pinned?.title ?? item.search_name.trim(),
        providers: [provider],
        cache: false,
        batch_id: batch_id.value,
      });
      check();
      const match = automaticCandidate(candidates);
      if (!match) continue;
      item.match = match;
      await prepareItem(item, run);
      check();
      return;
    } catch (cause) {
      check();
      last_error = cause;
    }
  }
  item.match = pinned;
  item.scrape_status = last_error ? 'failed' : 'no_match';
  item.scrape_message = last_error
    ? errorText(last_error)
    : '未找到唯一的高置信度候选，请手动匹配。';
}
async function prepareItem(item: ReviewItem, run: number) {
  const operation_id = scrape_operation.value;
  discardPreparations([item]);
  updateOperation(operation_id, {
    message: `${item.search_name} · ${item.match?.provider.toUpperCase()} · 获取完整资料并缓存封面`,
  });
  try {
    const current_batch = batch_id.value;
    await prepareReview(item, async (match) => {
      if (run !== scrape_generation) throw new Error('本轮导入已取消。');
      const preparation = await api('prepare_import_metadata', {
        directory: item.install_path,
        provider: match.provider,
        remote_id: match.remote_id,
        manual: item.manual_match,
        single_source: kind.value === 'single',
        title_hint: match.title,
        batch_id: current_batch,
      });
      if (run !== scrape_generation) {
        void api('discard_import_metadata', {
          preparation_ids: [preparation.preparation_id],
        }).catch(() => {});
        throw new Error('本轮导入已取消。');
      }
      return preparation;
    });
    if (item.preparation?.translation_message)
      notify(item.preparation.translation_message);
  } catch (cause) {
    if (run === scrape_generation)
      updateOperation(operation_id, {
        message: errorText(cause),
      });
    throw cause;
  }
}
async function startScrape(background = false) {
  if (busy.value || review_loading.value || !review_items.value.length) return;
  const items = review_items.value.filter(needsScrape);
  if (!items.length) {
    notify('选中的作品均已刮削完成，无需重复刮削。');
    return;
  }
  error.value = '';
  const run = ++scrape_generation;
  const batch = review_items.value;
  const operation = createOperation('批量刮削', 'scrape', items.length, () => {
    if (review_items.value === batch) local.import_open = true;
    else notify('该批次已导入或取消，请重新选择导入目录。');
  });
  scrape_operation.value = operation.id;
  updateOperation(operation.id, {
    status: 'running',
    progress: 0,
    message: '仅处理待刮削或失败项，获取完整资料并缓存封面',
  });
  busy.value = true;
  scrape_running.value = true;
  background_scrape.value = background;
  manual_item.value = null;
  if (background) {
    closeImport();
    notify('刮削已转入后台，完成后可在右上角任务中心查看。');
  }
  try {
    {
      const started = await api('begin_import_batch', {});
      if (run !== scrape_generation) {
        await api('cancel_import_batch', { batch_id: started.batch_id });
        return;
      }
      batch_id.value = started.batch_id;
      batch_ids.add(started.batch_id);
      taskProviders = started.sources.sources
        .filter((s) => s.enabled)
        .map((s) => s.provider as MetadataProvider);
    }
  } catch (cause) {
    if (run !== scrape_generation) return;
    error.value = errorText(cause);
    busy.value = false;
    scrape_running.value = false;
    updateOperation(operation.id, { status: 'failed', message: error.value });
    return;
  }
  for (let index = 0; index < items.length; index += 1) {
    const item = items[index]!;
    try {
      if (run !== scrape_generation) return;
      await scrapeItem(item, run);
    } catch (cause) {
      if (run !== scrape_generation) return;
      item.scrape_status = 'failed';
      item.scrape_message = errorText(cause);
    }
    if (run !== scrape_generation) return;
    updateOperation(operation.id, {
      progress: index + 1,
      message: `${index + 1}/${items.length} · ${item.search_name}`,
    });
  }
  const success_count = items.filter(isPrepared).length;
  updateOperation(operation.id, {
    status: success_count === items.length ? 'completed' : 'failed',
    progress: items.length,
    message: `刮削成功 ${success_count}/${items.length} 项，其余可修改名称或手动匹配后重试`,
  });
  busy.value = false;
  scrape_running.value = false;
  if (!background_scrape.value) {
    notify(
      `刮削成功 ${success_count} 项，未完成 ${items.length - success_count} 项；成功项资料和封面已就绪，确认导入即可入库。`,
    );
  }
}
function runInBackground() {
  if (!scrape_running.value) {
    void startScrape(true);
    return;
  }
  background_scrape.value = true;
  closeImport();
  notify('刮削继续在后台运行，可通过右上角下载控件查看进度或重新打开结果。');
}
function openManual(item: ReviewItem) {
  cancelManualSearch();
  manual_item.value = item;
  manual_provider.value = 'hikarinagi';
  manual_query.value = item.search_name;
  manual_candidates.value = [];
  error.value = '';
}
async function searchManual() {
  if (!manual_item.value || !manual_query.value.trim() || busy.value) return;
  cancelManualSearch();
  const controller = new AbortController();
  manual_search_controller = controller;
  manual_loading.value = true;
  const sequence = ++manual_search_sequence;
  const item = manual_item.value;
  if (single_import.value) resetMatch(item);
  manual_candidates.value = [];
  error.value = '';
  try {
    const candidates = await api(
      'search_metadata',
      {
        query: manual_query.value.trim(),
        providers: [manual_provider.value],
        manual: true,
        cache: false,
        ...(single_import.value ? { batch_id: batch_id.value } : {}),
      },
      { signal: controller.signal },
    );
    if (sequence === manual_search_sequence && manual_item.value === item)
      manual_candidates.value = candidates;
  } catch (cause) {
    if (sequence === manual_search_sequence && manual_item.value === item)
      error.value = errorText(cause);
  } finally {
    if (sequence === manual_search_sequence) manual_loading.value = false;
  }
}
function chooseManual(candidate: MetadataCandidate) {
  if (!manual_item.value || busy.value) return;
  const item = manual_item.value;
  discardPreparations([item]);
  selectReviewMatch(item, candidate);
  manual_item.value = null;
  manual_candidates.value = [];
  notify('候选已选择，请点击“开始刮削”获取资料和封面。');
}
async function confirmReview() {
  if (review_loading.value || busy.value) return;
  if (unresolved_count.value) {
    error.value = `有 ${unresolved_count.value} 个作品包含多个 EXE，请先选择启动入口。`;
    return;
  }
  const selected = review_items.value.filter(
    (item) => item.selected && !item.preview?.existing_game_id,
  );
  if (selected.some((item) => item.manual_match && !isPrepared(item))) {
    error.value =
      '手动匹配的作品尚未刮削完成，请先点击“开始刮削”；如需本地导入，可修改搜索名清除匹配。';
    return;
  }
  if (!selected.length) {
    error.value = '没有选择需要导入的作品。';
    return;
  }
  busy.value = true;
  error.value = '';
  let imported = 0;
  let configured = 0;
  let failed = 0;
  const operation = createOperation('导入游戏', 'import', selected.length);
  updateOperation(operation.id, {
    status: 'running',
    progress: 0,
    message: '正在写入游戏库',
  });
  for (let index = 0; index < selected.length; index += 1) {
    const item = selected[index]!;
    try {
      const game = await api('import_prepared_game', {
        directory: item.install_path,
        title: item.search_name,
        executable_path: item.selected_executable || null,
        preparation_id: item.preparation?.preparation_id ?? null,
      });
      item.preparation = null;
      imported += 1;
      if (item.selected_executable) configured += 1;
      if (item.preview) {
        item.preview.existing_game_id = game.id;
        item.selected = false;
      }
    } catch (cause) {
      failed += 1;
      item.scrape_status = 'failed';
      item.scrape_message = errorText(cause);
    }
    updateOperation(operation.id, {
      progress: index + 1,
      message: `${index + 1}/${selected.length} · ${item.search_name}`,
    });
  }
  await refreshLibrary();
  updateOperation(operation.id, {
    status: failed ? 'failed' : 'completed',
    progress: selected.length,
    message: failed
      ? `完成 ${selected.length - failed} 项，失败 ${failed} 项（可重试）`
      : `完成 ${imported} 项导入`,
  });
  busy.value = false;
  if (failed) {
    if (single_import.value)
      error.value = selected[0]?.scrape_message ?? '导入失败，请重试。';
    notify(`已导入 ${imported} 项，${failed} 项失败，失败条目仍保留在列表中。`);
    return;
  }
  notify(`已加入 ${imported} 个作品，${configured} 个启动入口已保存。`);
  closeImport();
  resetReview();
}

async function handleDroppedPaths(paths: string[]) {
  if (busy.value || review_loading.value) {
    notify('当前导入任务仍在进行，请完成后再拖入新的目录。');
    return;
  }
  const selected = paths.filter(Boolean);
  if (!selected.length) return;
  if (selected.some(isExecutable))
    await startSingleImport(
      parentPath(selected.find(isExecutable)!),
      selected.find(isExecutable)!,
    );
  else await startDirectoryImport(selected);
}
function onWindowDragOver(event: DragEvent) {
  if (event.dataTransfer?.types.includes('Files')) {
    event.preventDefault();
    drop_active.value = true;
  }
}
function onWindowDragLeave(event: DragEvent) {
  if (event.relatedTarget === null) drop_active.value = false;
}
function onWindowDrop(event: DragEvent) {
  if (!event.dataTransfer?.types.includes('Files')) return;
  event.preventDefault();
  drop_active.value = false;
  if (!desktop)
    notify(
      '浏览器预览不能读取拖放路径，请在 Windows 桌面版本中拖入 EXE 或目录。',
    );
}

async function poll() {
  if (polling || !local.scan || !isScanActive(local.scan.status)) return;
  polling = true;
  try {
    const report = await api('get_scan_task', { task_id: local.scan.task_id });
    local.scan = report;
    if (!isScanActive(report.status)) {
      await refreshLibrary();
      notify(scanNotification(report));
    }
  } catch (cause) {
    error.value = errorText(cause);
  } finally {
    polling = false;
  }
}
async function control(action: 'pause' | 'resume' | 'cancel') {
  if (!local.scan) return;
  try {
    await api('control_scan_task', { task_id: local.scan.task_id, action });
    await poll();
  } catch (cause) {
    error.value = errorText(cause);
  }
}
onMounted(() => {
  window.addEventListener('dragover', onWindowDragOver, true);
  window.addEventListener('dragleave', onWindowDragLeave, true);
  window.addEventListener('drop', onWindowDrop, true);
  if (desktop) {
    timer = setInterval(() => void poll(), 1000);
    void listen<NativeDropPayload>('tauri://drag-drop', (event) => {
      const paths = event.payload?.paths ?? [];
      if (!paths.length) return;
      local.import_open = true;
      void nextTick(() => void handleDroppedPaths(paths));
    }).then((unlisten) => {
      if (disposed) unlisten();
      else native_drop_unlisten = unlisten;
    });
  }
});
onUnmounted(() => {
  disposed = true;
  cancelManualSearch();
  scrape_generation++;
  for (const batch_id of batch_ids)
    void api('cancel_import_batch', { batch_id }).catch(() => {});
  discardPreparations(review_items.value);
  clearInterval(timer);
  native_drop_unlisten?.();
  window.removeEventListener('dragover', onWindowDragOver, true);
  window.removeEventListener('dragleave', onWindowDragLeave, true);
  window.removeEventListener('drop', onWindowDrop, true);
});
</script>
<template>
  <section
    v-if="desktop && local.scan && isScanActive(local.scan.status)"
    class="local-task"
    aria-label="目录扫描任务"
    aria-live="polite"
  >
    <div>
      <strong
        >目录扫描 ·
        {{
          {
            queued: '等待',
            running: '扫描中',
            paused: '已暂停',
            completed: '已结束',
            cancelled: '已取消',
            failed: '失败',
          }[local.scan.status]
        }}</strong
      ><span
        >{{ local.scan.message }} · 已检查
        {{ local.scan.processed }} 个条目</span
      >
    </div>
    <button
      v-if="['queued', 'running'].includes(local.scan.status)"
      class="quiet-button"
      @click="control('pause')"
    >
      暂停
    </button>
    <button
      v-if="local.scan.status === 'paused'"
      class="quiet-button"
      @click="control('resume')"
    >
      继续
    </button>
    <button
      v-if="['queued', 'running', 'paused'].includes(local.scan.status)"
      class="quiet-button"
      @click="control('cancel')"
    >
      取消
    </button>
    <details v-if="local.scan.issue_count || local.scan.truncated">
      <summary>
        扫描问题 {{ local.scan.issue_count }}
        {{ local.scan.truncated ? '· 扫描范围已截断' : '' }}
      </summary>
      <p v-for="(issue, index) in local.scan.issues" :key="index">
        {{ issue.path }}：{{ issue.reason }}
      </p>
      <p v-if="local.scan.issue_count > 100">仅展示前 100 条问题。</p>
    </details>
    <p v-if="error" role="alert">{{ error }}</p>
  </section>
  <div v-if="drop_active" class="import-drop-overlay" aria-live="polite">
    <div>
      <PreviewIcon name="games" :size="32" />
      <strong>松开以识别 Galgame</strong>
      <span>拖入启动 EXE 或包含多个作品的目录</span>
    </div>
  </div>
  <Teleport to="body">
    <dialog
      ref="dialog"
      :class="[
        'ui-dialog local-import',
        {
          'import-choice-dialog': stage === 'choice',
          'import-review-dialog': stage === 'review',
        },
      ]"
      aria-labelledby="import-title"
      @close="!single_import && (local.import_open = false)"
      @cancel.prevent="cancelImport()"
    >
      <form @submit.prevent="confirmReview">
        <header class="import-heading">
          <div>
            <p class="eyebrow">ADD TO LIBRARY</p>
            <h2 id="import-title">添加本地游戏</h2>
          </div>
          <button
            type="button"
            class="import-close"
            aria-label="关闭导入窗口"
            :disabled="busy"
            @click="cancelImport"
          >
            <PreviewIcon name="close" />
          </button>
        </header>
        <template v-if="stage === 'choice'">
          <p class="import-lead">
            让喜欢的作品，在这里相聚。选择适合你的导入方式。
          </p>
          <div class="import-choices" aria-label="导入方式">
            <button
              type="button"
              class="import-choice"
              :disabled="busy"
              @click="chooseSingleDirectory"
            >
              <span class="import-choice-icon"
                ><PreviewIcon name="play"
              /></span>
              <span>
                <strong>导入单个游戏</strong>
                <small>选择游戏文件夹，手动选择单一刮削源和作品</small>
              </span>
              <PreviewIcon name="arrow" :size="18" />
            </button>
            <button
              type="button"
              class="import-choice"
              aria-label="导入游戏目录 · 批量导入"
              :disabled="busy"
              @click="chooseDirectory"
            >
              <span class="import-choice-icon"
                ><PreviewIcon name="games"
              /></span>
              <span>
                <strong>批量导入</strong>
                <small>导入游戏目录，批量识别作品与启动入口</small>
              </span>
              <PreviewIcon name="arrow" :size="18" />
            </button>
            <button
              type="button"
              class="import-choice import-choice-steam"
              :disabled="busy"
              @click="
                local.steam_import_open = true;
                closeImport();
              "
            >
              <span class="import-choice-icon import-choice-steam-icon"
                ><PreviewIcon name="steam"
              /></span>
              <span
                ><strong>从 Steam 导入</strong
                ><small
                  >扫描本机 Steam 库，使用 Steam 资料并补充 Hikarinagi
                  安利墙</small
                ></span
              >
              <PreviewIcon name="arrow" :size="18" />
            </button>
          </div>
          <p class="import-hint">
            也可以直接把 EXE 或目录拖入窗口，自动打开对应的识别界面。
          </p>
        </template>
        <template v-else>
          <div class="review-toolbar">
            <button
              type="button"
              class="quiet-button"
              :disabled="busy"
              @click="returnToChoice"
            >
              <PreviewIcon name="back" :size="16" />重新选择
            </button>
            <span>{{
              kind === 'single' ? '单个游戏识别' : '目录批量识别'
            }}</span>
          </div>
          <ul v-if="roots.length" class="review-roots" aria-label="已选择目录">
            <li v-for="root in roots" :key="root">{{ root }}</li>
          </ul>
          <section class="review-progress-panel" aria-label="批量导入进度">
            <div class="review-progress-heading">
              <div>
                <span
                  class="review-live-dot"
                  :class="{ running: scrape_running }"
                  aria-hidden="true"
                /><strong
                  >已识别
                  <span class="review-recognized-count">{{
                    review_stats.recognized
                  }}</span>
                  部游戏</strong
                >
              </div>
              <span class="review-percentage"
                >{{ review_progress.percentage }}<small>%</small></span
              >
            </div>
            <div
              class="review-progress-track"
              role="progressbar"
              aria-label="已选游戏处理进度"
              :aria-valuemin="0"
              :aria-valuemax="review_progress.total || 1"
              :aria-valuenow="review_progress.processed"
              :aria-valuetext="`已处理 ${review_progress.processed} / ${review_progress.total} 部，已刮削 ${scraped_count} 部`"
            >
              <span :style="{ width: `${review_progress.percentage}%` }" />
            </div>
            <div class="review-progress-caption">
              <span
                >已处理 {{ review_progress.processed }} /
                {{ review_progress.total }} 部</span
              >
              <span
                >已刮削
                <b class="review-scraped-count">{{ scraped_count }}</b> 部<span
                  class="review-pending"
                >
                  · 待刮削
                  <b class="review-pending-count">{{ pending_count }}</b>
                  部</span
                ></span
              >
            </div>
            <div class="review-progress-bottom">
              <p :title="current_scrape?.search_name">
                {{
                  current_scrape
                    ? `正在刮削 · ${current_scrape.search_name}`
                    : '刮削结果暂存，确认导入后加入游戏库。'
                }}
              </p>
              <small v-if="review_stats.imported" class="review-imported-count"
                >已导入（{{ review_stats.imported }}）</small
              >
            </div>
          </section>
          <p v-if="review_loading" class="review-loading" role="status">
            正在识别文件夹与启动 EXE，请稍候…
          </p>
          <p v-else-if="!review_items.length" class="review-empty">
            没有识别到可导入的游戏。可以返回重新选择目录。
          </p>
          <div v-else class="review-list" aria-label="识别结果">
            <article
              v-for="item in review_items"
              :key="item.install_path"
              class="review-card"
              :data-selected="item.selected"
              :data-scraped="isScraped(item)"
              :data-imported="isImported(item)"
              :data-failed="
                !isImported(item) && item.scrape_status === 'failed'
              "
              :aria-busy="
                !isImported(item) && item.scrape_status === 'scraping'
              "
            >
              <div class="review-row">
                <label
                  class="review-select"
                  :title="
                    isImported(item) ? '已在库中，跳过重复导入' : '是否导入'
                  "
                >
                  <input
                    v-model="item.selected"
                    type="checkbox"
                    :aria-label="`选择导入 ${item.search_name}`"
                    :disabled="busy || !!item.preview?.existing_game_id"
                  />
                  <span class="review-check-mark" aria-hidden="true"
                    ><PreviewIcon name="check" :size="14"
                  /></span>
                </label>
                <label class="review-search-name">
                  <span>搜索名称</span>
                  <input
                    v-model="item.search_name"
                    :disabled="busy || isImported(item)"
                    @input="resetMatch(item)"
                  />
                </label>
                <label class="review-executable review-executable-inline">
                  <span>EXE</span>
                  <select
                    v-if="executableCandidates(item).length > 1"
                    v-model="item.selected_executable"
                    :disabled="busy || isImported(item)"
                  >
                    <option value="">请选择</option>
                    <option
                      v-for="candidate in executableCandidates(item)"
                      :key="candidate.path"
                      :value="candidate.path"
                    >
                      {{ candidate.path }}
                    </option>
                  </select>
                  <input
                    v-else
                    :value="item.selected_executable || '未识别到 EXE'"
                    readonly
                    :aria-label="item.selected_executable || '未识别到 EXE'"
                  />
                </label>
                <span
                  class="review-status"
                  :data-status="
                    isImported(item) ? 'imported' : item.scrape_status
                  "
                  :title="
                    isImported(item)
                      ? item.preview?.duplicate_reason ||
                        '路径已在游戏库中，将跳过刮削与重复导入。'
                      : item.scrape_message || metadataLabel(item.scrape_status)
                  "
                  role="status"
                >
                  <Transition name="review-success">
                    <svg
                      v-if="isScraped(item) || isImported(item)"
                      class="review-success-mark"
                      viewBox="0 0 20 20"
                      aria-hidden="true"
                    >
                      <circle cx="10" cy="10" r="9" />
                      <path d="M5.5 10.2 8.4 13.1 14.5 7" pathLength="1" />
                    </svg>
                  </Transition>
                  {{
                    isImported(item)
                      ? '已导入'
                      : metadataLabel(item.scrape_status)
                  }}
                </span>
                <button
                  type="button"
                  class="quiet-button"
                  :disabled="busy || isImported(item) || !item.selected"
                  @click="openManual(item)"
                >
                  手动匹配
                </button>
              </div>
            </article>
          </div>

          <p v-if="error && !manual_item" role="alert" class="import-error">
            {{ error }}
          </p>
          <div class="dialog-actions">
            <button
              type="button"
              class="secondary-button"
              :disabled="busy && !scrape_running"
              @click="cancelImport"
            >
              取消
            </button>
            <div class="dialog-actions-right">
              <button
                type="button"
                class="secondary-button"
                :disabled="
                  review_loading || !selected_count || (busy && !scrape_running)
                "
                @click="runInBackground"
              >
                后台运行
              </button>
              <button
                type="button"
                class="secondary-button"
                :disabled="busy || review_loading || !selected_count"
                @click="startScrape(false)"
              >
                开始刮削
              </button>
              <button
                class="primary-button"
                :disabled="
                  busy ||
                  review_loading ||
                  unresolved_count > 0 ||
                  !selected_count
                "
              >
                {{
                  busy
                    ? '处理中…'
                    : unresolved_count
                      ? `还有 ${unresolved_count} 个入口待选择`
                      : '确认导入'
                }}
              </button>
            </div>
          </div>
        </template>
        <p v-if="stage === 'choice' && error" role="alert" class="import-error">
          {{ error }}
        </p>
      </form>
    </dialog>
    <dialog
      ref="single_dialog"
      class="ui-dialog local-import single-import-dialog"
      aria-labelledby="single-import-title"
      @cancel.prevent="cancelImport()"
    >
      <form v-if="single_import" @submit.prevent="confirmSingleImport">
        <header class="import-heading">
          <div>
            <p class="eyebrow">ADD ONE STORY</p>
            <h2 id="single-import-title">导入单个游戏</h2>
          </div>
          <button
            type="button"
            class="import-close"
            :disabled="busy"
            aria-label="关闭单个导入窗口"
            @click="cancelImport"
          >
            <PreviewIcon name="close" />
          </button>
        </header>
        <p class="import-lead">
          单击选中作品，双击直接刮削并导入；也可以使用下方的确认按钮。
        </p>
        <p v-if="roots[0]" class="single-folder">{{ roots[0] }}</p>
        <p v-if="review_loading" role="status">正在读取游戏文件夹…</p>
        <template
          v-else-if="single_item && !single_item.preview?.existing_game_id"
        >
          <div class="manual-match-toolbar">
            <input
              v-model="manual_query"
              aria-label="单个游戏搜索名称"
              :disabled="busy"
              placeholder="游戏名称或远程 ID"
              @keydown.enter.prevent="searchManual"
            />
            <select
              v-model="manual_provider"
              aria-label="单个游戏刮削源"
              :disabled="busy"
              @change="changeSingleSource"
            >
              <option
                v-for="source in metadataSources.sources"
                :key="source.provider"
                :value="source.provider"
              >
                {{
                  source.provider === 'hikarinagi'
                    ? 'Hikarinagi'
                    : source.provider === 'bangumi'
                      ? 'Bangumi'
                      : 'VNDB'
                }}
              </option>
            </select>
            <button
              type="button"
              class="secondary-button"
              :disabled="manual_loading || busy || !manual_query.trim()"
              @click="searchManual"
            >
              {{ manual_loading ? '搜索中…' : '搜索' }}
            </button>
          </div>
          <p v-if="manual_loading" role="status">正在获取候选作品…</p>
          <div v-else class="single-candidates" aria-label="单个游戏候选作品">
            <button
              v-for="candidate in manual_candidates"
              :key="`${candidate.provider}:${candidate.remote_id}`"
              type="button"
              class="single-candidate"
              :aria-pressed="
                single_item.match?.remote_id === candidate.remote_id &&
                single_item.match?.provider === candidate.provider
              "
              :disabled="busy"
              @click="chooseSingleMatch(candidate)"
              @dblclick="scrapeSingleMatch(candidate)"
            >
              <PreviewCover
                :cover_url="candidate.cover_url ?? ''"
                :title="candidate.title"
              />
              <strong>{{ candidate.title }}</strong
              ><span v-if="candidate.subtitle">{{ candidate.subtitle }}</span
              ><small
                >{{ candidate.provider }} · {{ candidate.remote_id }}</small
              >
            </button>
          </div>
          <p
            v-if="!manual_loading && !manual_candidates.length && !error"
            class="manual-empty"
          >
            没有找到候选，请修改名称或切换刮削源后搜索。
          </p>
          <label
            v-if="executableCandidates(single_item).length > 1"
            class="single-launch"
            >启动入口
            <select v-model="single_item.selected_executable" :disabled="busy">
              <option value="">请选择启动程序</option>
              <option
                v-for="entry in executableCandidates(single_item)"
                :key="entry.path"
                :value="entry.path"
              >
                {{ entry.path }}
              </option>
            </select>
          </label>
          <p v-if="single_item.match" class="import-hint">
            已选择：{{ single_item.match.title }} ·
            {{ single_item.match.provider }}。确认后获取该来源的资料与封面。
          </p>
        </template>
        <p v-if="error" role="alert" class="import-error">{{ error }}</p>
        <footer class="dialog-actions">
          <button
            type="button"
            class="quiet-button"
            :disabled="busy"
            @click="cancelImport"
          >
            取消
          </button>
          <button
            type="submit"
            class="primary-button"
            :disabled="
              busy ||
              review_loading ||
              manual_loading ||
              !single_item?.match ||
              !!single_item.preview?.existing_game_id
            "
          >
            {{ busy ? '正在刮削并导入…' : '确认刮削并导入' }}
          </button>
        </footer>
      </form>
    </dialog>
    <dialog
      ref="manual_dialog"
      class="ui-dialog local-import manual-match-dialog"
      aria-label="手动匹配窗口"
      @cancel.prevent="manual_item = null"
      @close="manual_item = null"
    >
      <section
        v-if="manual_item"
        class="manual-match-panel"
        aria-label="手动匹配"
      >
        <header>
          <strong>手动匹配：{{ manual_item.search_name }}</strong>
          <button
            type="button"
            class="import-close"
            @click="manual_item = null"
          >
            <PreviewIcon name="close" :size="16" />
          </button>
        </header>
        <div class="manual-match-toolbar">
          <input
            v-model="manual_query"
            :disabled="manual_loading"
            placeholder="输入中文名、日文名或远程 ID"
            @keyup.enter="searchManual"
          />
          <select v-model="manual_provider" :disabled="manual_loading">
            <option
              v-for="source in metadataSources.sources"
              :key="source.provider"
              :value="source.provider"
            >
              {{
                source.provider === 'hikarinagi'
                  ? 'Hikarinagi'
                  : source.provider === 'bangumi'
                    ? 'Bangumi'
                    : 'VNDB'
              }}{{ source.enabled ? '' : '（已关闭）' }}
            </option>
          </select>
          <button
            type="button"
            class="secondary-button"
            :disabled="manual_loading"
            @click="searchManual"
          >
            {{ manual_loading ? '搜索中…' : '搜索' }}
          </button>
        </div>
        <ul class="manual-results">
          <li
            v-for="candidate in manual_candidates"
            :key="`${candidate.provider}:${candidate.remote_id}`"
          >
            <div>
              <strong>{{ candidate.title }}</strong
              ><span v-if="candidate.subtitle">{{ candidate.subtitle }}</span
              ><small
                >{{ candidate.provider }} · {{ candidate.remote_id }} ·
                {{ Math.round(candidate.confidence * 100) }}%</small
              >
            </div>
            <PreviewCover
              v-if="candidate.cover_url"
              class="manual-result-cover"
              :cover_url="candidate.cover_url"
              :title="candidate.title"
            />
            <button
              type="button"
              class="quiet-button"
              @click="chooseManual(candidate)"
            >
              使用
            </button>
          </li>
          <li
            v-if="!manual_loading && !manual_candidates.length"
            class="manual-empty"
          >
            输入关键词后搜索资料源。
          </li>
        </ul>
      </section>
      <p v-if="error" role="alert" class="import-error">{{ error }}</p>
    </dialog>
  </Teleport>
</template>
<style scoped>
.local-import.import-choice-dialog {
  width: min(640px, calc(100vw - 32px));
}
.import-choice-dialog .import-choices {
  display: grid;
  grid-template-columns: 1fr;
  gap: 12px;
}
.import-choice-dialog .import-choice {
  position: relative;
  padding: 22px;
  overflow: hidden;
  transition:
    background 240ms ease,
    border-color 240ms ease,
    transform 240ms ease;
}
.import-choice-dialog .import-choice:hover:not(:disabled) {
  transform: translateY(-2px);
  background: var(--accent-wash);
}
.import-choice-dialog .import-choice-icon {
  border-radius: 14px;
  background: var(--accent-wash);
}
.import-choice-dialog .import-choice strong {
  font-family: var(--font-display);
  font-size: 20px;
}
.import-choice-dialog .import-choice small {
  line-height: 1.7;
}
@media (prefers-reduced-motion: reduce) {
  .local-import.import-choice-dialog .import-choice:hover:not(:disabled),
  .local-import.import-choice-dialog .import-choice {
    transition: none;
    transform: none;
  }
}

:global([data-motion='reduced'])
  .local-import.import-choice-dialog
  .import-choice:hover:not(:disabled),
:global([data-motion='reduced'])
  .local-import.import-choice-dialog
  .import-choice {
  transition: none;
  transform: none;
}
.local-import.single-import-dialog {
  width: min(820px, calc(100vw - 32px));
}
.single-folder {
  color: var(--muted);
  font-size: var(--type-small);
  overflow-wrap: anywhere;
}
.single-import-dialog .manual-match-toolbar {
  display: flex;
  gap: 12px;
  flex-wrap: wrap;
}
.single-candidates {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(140px, 1fr));
  gap: 16px;
  margin-top: 20px;
}
.single-candidate {
  display: flex;
  flex-direction: column;
  gap: 6px;
  min-width: 0;
  padding: 10px;
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  background: var(--surface-glass);
  color: var(--text);
  text-align: left;
}
.single-candidate[aria-pressed='true'] {
  border-color: var(--accent);
  box-shadow: 0 0 0 2px var(--accent-wash);
}
.single-candidate :deep(.preview-cover) {
  width: 100%;
  aspect-ratio: 3 / 4;
}
.single-candidate strong,
.single-candidate span {
  overflow-wrap: anywhere;
}
.single-candidate small {
  color: var(--muted);
}
.single-launch {
  display: grid;
  gap: 8px;
  margin-top: 20px;
}
.single-launch select {
  width: 100%;
  min-width: 0;
}

.manual-match-dialog {
  width: min(920px, calc(100vw - 40px));
  max-height: calc(100dvh - 64px);
}
.manual-match-dialog .manual-match-panel {
  border: 0;
  margin: 0;
  padding: 0;
}
.local-task {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--space-12);
  margin: var(--space-16) var(--space-32) 0;
  padding: var(--space-16);
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  background: var(--surface);
  font-size: var(--type-small);
}
.local-task div {
  flex: 1;
  min-width: 180px;
}
.local-task strong,
.local-task span {
  display: block;
}
.local-task span {
  margin-top: 4px;
  color: var(--muted);
}
.local-task details {
  width: 100%;
  overflow-wrap: anywhere;
}
.local-task p {
  margin: 8px 0;
}
.local-import {
  position: fixed;
  inset: 0;
  margin: auto;
  width: min(1040px, calc(100vw - 32px));
  overflow: auto;
  background: var(--surface);
  color: var(--text);
}
.import-heading,
.review-toolbar,
.review-card-heading,
.dialog-actions {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-16);
}
.import-heading {
  margin-bottom: var(--space-24);
}
.import-heading h2 {
  margin: 0;
}
.import-close {
  display: grid;
  width: 36px;
  height: 36px;
  place-items: center;
  border: 1px solid transparent;
  border-radius: var(--radius-pill);
  background: transparent;
  color: var(--muted);
}
.import-close:hover {
  color: var(--text);
  background: var(--surface-hover);
}
.import-lead,
.import-hint,
.review-empty,
.review-loading {
  color: var(--muted);
  font-size: var(--type-small);
  line-height: 1.7;
}
.import-choices {
  display: grid;
  gap: var(--space-12);
  margin: var(--space-24) 0 var(--space-16);
}
.import-choice {
  display: grid;
  grid-template-columns: auto minmax(0, 1fr) auto;
  align-items: center;
  gap: var(--space-16);
  width: 100%;
  padding: var(--space-16);
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  background: var(--surface-hover);
  color: var(--text);
  text-align: left;
  transition:
    border-color var(--micro-duration) var(--ease-standard),
    background var(--micro-duration) var(--ease-standard);
}
.import-choice:hover {
  border-color: var(--accent-ink, var(--accent));
  background: var(--accent-wash);
}
.import-choice:disabled {
  cursor: wait;
  opacity: 0.55;
}
.import-choice-icon {
  display: grid;
  width: 42px;
  height: 42px;
  place-items: center;
  border-radius: var(--radius-md);
  background: var(--accent-wash);
  color: var(--accent-ink, var(--accent));
}
.import-choice strong,
.import-choice small {
  display: block;
}
.import-choice small {
  margin-top: 4px;
  color: var(--muted);
  font-size: 11px;
}
.import-hint {
  margin: 0;
}
.review-toolbar {
  margin-bottom: var(--space-12);
  color: var(--muted);
  font-size: var(--type-small);
}
.review-roots {
  display: grid;
  gap: 4px;
  max-height: 72px;
  overflow: auto;
  margin: 0 0 var(--space-16);
  padding: 0;
  list-style: none;
  color: var(--muted);
  font-size: 11px;
  overflow-wrap: anywhere;
}
.import-review-dialog .import-heading {
  margin-bottom: 14px;
}
.local-import.import-review-dialog[open],
.import-review-dialog > form {
  display: flex;
  flex-direction: column;
  min-height: 0;
}
.import-review-dialog > form {
  flex: 1;
}
.import-review-dialog > form > :not(.review-list) {
  flex-shrink: 0;
}
.import-review-dialog .review-list {
  flex: 0 1 auto;
  min-height: 64px;
}
.import-review-dialog .review-toolbar {
  margin-bottom: 8px;
}
.import-review-dialog .review-roots {
  margin-bottom: 10px;
}
.review-progress-panel {
  padding: 12px 18px;
  margin: 10px 0 12px;
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  background: var(--accent-wash);
}
.review-progress-heading,
.review-progress-heading > div,
.review-progress-caption,
.review-progress-bottom {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  min-width: 0;
}
.review-progress-heading > div {
  justify-content: flex-start;
  gap: 9px;
}
.review-progress-heading strong {
  font-size: 14px;
  font-weight: 500;
  font-variant-numeric: tabular-nums;
}
.review-live-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: var(--accent);
  flex-shrink: 0;
}
.review-live-dot.running {
  animation: review-progress-breathe 1.8s ease-in-out infinite;
}
.review-percentage {
  font-size: 28px;
  line-height: 1.2;
  color: var(--accent-ink, var(--accent));
  font-variant-numeric: tabular-nums;
  letter-spacing: -0.03em;
  flex-shrink: 0;
}
.review-percentage small {
  font-size: 11px;
  margin-left: 3px;
}
.review-progress-track {
  height: 4px;
  overflow: hidden;
  border-radius: var(--radius-pill);
  background: var(--border);
  margin-top: 10px;
}
.review-progress-track > span {
  display: block;
  height: 100%;
  border-radius: inherit;
  background: var(--accent);
  transition: width 280ms var(--ease-standard);
}
.review-progress-caption {
  margin-top: 8px;
  color: var(--muted);
  font-size: 11px;
  font-variant-numeric: tabular-nums;
  flex-wrap: wrap;
  gap: 4px 10px;
}
.review-progress-caption b {
  font-weight: 400;
}
.review-progress-bottom {
  margin-top: 8px;
  color: var(--muted);
  font-size: 11px;
  gap: 8px;
}
.review-progress-bottom p {
  margin: 0;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.review-imported-count {
  font-size: 10px;
  flex-shrink: 0;
}
.review-card-details,
.review-card-heading p,
.review-executable-note {
  color: var(--muted);
  font-size: 11px;
}
@keyframes review-progress-breathe {
  50% {
    opacity: 0.4;
  }
}
@media (prefers-reduced-motion: reduce) {
  .review-progress-track > span {
    transition: none;
  }
  .review-live-dot.running {
    animation: none;
  }
}
:global([data-motion='reduced']) .review-progress-track > span {
  transition: none;
}
:global([data-motion='reduced']) .review-live-dot.running {
  animation: none;
}
.review-list {
  display: grid;
  gap: var(--space-8);
  max-height: min(45vh, 460px);
  overflow: auto;
  margin: var(--space-16) 0;
  padding-right: 4px;
}
.review-card {
  display: grid;
  gap: var(--space-8);
  padding: var(--space-16);
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  background: var(--surface-hover);
  position: relative;
  transition:
    border-color var(--feedback-duration) var(--ease-standard),
    background-color var(--feedback-duration) var(--ease-standard),
    box-shadow var(--feedback-duration) var(--ease-standard);
}
.review-card[data-scraped='true'],
.review-card[data-imported='true'] {
  border-color: var(--success);
  background: color-mix(in srgb, var(--success) 7%, var(--surface-hover));
  box-shadow: inset 0 0 0 1px
    color-mix(in srgb, var(--success) 15%, transparent);
}
.review-card[data-scraped='true']::after {
  content: '';
  position: absolute;
  inset: -1px;
  border: 1px solid var(--success);
  border-radius: inherit;
  opacity: 0;
  pointer-events: none;
  animation: import-success-ring 650ms var(--ease-standard);
}
@keyframes import-success-ring {
  from {
    opacity: 0.7;
    box-shadow: 0 0 0 0 color-mix(in srgb, var(--success) 24%, transparent);
  }
  to {
    opacity: 0;
    box-shadow: 0 0 0 5px transparent;
  }
}
.review-card-heading {
  align-items: flex-start;
}
.review-select {
  position: relative;
  display: grid;
  place-items: center;
  width: 28px;
  height: 30px;
  flex-shrink: 0;
  cursor: pointer;
}
.review-select input {
  appearance: none;
  -webkit-appearance: none;
  display: block;
  width: 20px;
  height: 20px;
  margin: 0;
  border: 1px solid var(--border-strong);
  border-radius: 6px;
  background: var(--surface-glass);
  cursor: inherit;
  transition:
    background 180ms var(--ease-standard),
    border-color 180ms var(--ease-standard),
    box-shadow 180ms var(--ease-standard);
}
.review-select input:checked {
  border-color: var(--accent);
  background: var(--accent);
}
.review-select input:hover:not(:disabled) {
  border-color: var(--accent);
  box-shadow: 0 0 0 3px var(--accent-wash);
}
.review-select input:focus-visible {
  outline: 2px solid var(--accent);
  outline-offset: 3px;
}
.review-select input:disabled {
  cursor: default;
  opacity: 0.55;
}
.review-check-mark {
  position: absolute;
  inset: 0;
  display: grid;
  place-items: center;
  color: var(--accent-text);
  opacity: 0;
  transform: scale(0.8);
  pointer-events: none;
  transition:
    opacity 150ms ease,
    transform 200ms var(--ease-standard);
}
.review-select input:checked + .review-check-mark {
  opacity: 1;
  transform: scale(1);
}
.review-select input:checked:disabled + .review-check-mark {
  opacity: 0.55;
}
@media (prefers-reduced-motion: reduce) {
  .review-select input,
  .review-check-mark {
    transition: none;
  }
}
:global([data-motion='reduced']) .review-select input,
:global([data-motion='reduced']) .review-check-mark {
  transition: none;
}
.review-card[data-selected='false'] {
  opacity: 0.62;
}
.review-card-heading h3 {
  margin: 0;
  font-size: 15px;
  font-weight: 500;
}
.review-search-name {
  display: flex;
  align-items: center;
  gap: 6px;
  margin-top: 5px;
  color: var(--muted);
  font-size: 11px;
}
.review-search-name input {
  width: min(280px, 48vw);
  padding: 4px 7px;
  border: 1px solid var(--border);
  border-radius: var(--radius-control);
  background: var(--surface);
  color: var(--text);
}
.review-card-heading p {
  max-width: 480px;
  margin: 4px 0 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.review-status {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  flex-shrink: 0;
  padding: 4px 8px;
  border: 1px solid var(--border);
  border-radius: var(--radius-pill);
  color: var(--muted);
  font-size: 11px;
  white-space: nowrap;
}
.review-status[data-status='scraped'],
.review-status[data-status='manual'],
.review-status[data-status='imported'] {
  border-color: color-mix(in srgb, var(--success) 50%, var(--border));
  background: color-mix(in srgb, var(--success) 10%, transparent);
  color: var(--success);
}
.review-success-mark {
  width: 16px;
  height: 16px;
  flex: none;
  overflow: visible;
}
.review-success-mark circle {
  fill: color-mix(in srgb, var(--success) 16%, transparent);
}
.review-success-mark path {
  fill: none;
  stroke: currentColor;
  stroke-width: 1.8;
  stroke-linecap: round;
  stroke-linejoin: round;
  stroke-dasharray: 1;
  stroke-dashoffset: 0;
  animation: import-check-draw 420ms var(--ease-standard);
}
@keyframes import-check-draw {
  from {
    stroke-dashoffset: 1;
  }
  to {
    stroke-dashoffset: 0;
  }
}
.review-success-enter-active,
.review-success-leave-active {
  transition:
    opacity var(--feedback-duration) var(--ease-standard),
    transform var(--feedback-duration) var(--ease-standard);
}
.review-success-enter-from,
.review-success-leave-to {
  opacity: 0;
  transform: scale(0.75);
}
.review-card-details {
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-12);
}
.review-card-details .is-warning {
  color: var(--warning, #a26b2a);
}
.review-executable {
  display: grid;
  gap: 6px;
  margin: 0;
  color: var(--muted);
  font-size: 11px;
}
.review-executable select {
  width: 100%;
  padding: 9px 10px;
  border: 1px solid var(--border);
  border-radius: var(--radius-control);
  background: var(--surface);
  color: var(--text);
}
.review-executable-note {
  margin: 0;
  overflow-wrap: anywhere;
}
.review-executable-note.is-muted {
  color: var(--subtle);
}
.review-card-actions {
  display: flex;
  justify-content: flex-end;
  gap: var(--space-8);
}
.review-card {
  display: block;
  padding: var(--space-8) var(--space-12);
}
.review-row {
  display: grid;
  grid-template-columns:
    auto minmax(160px, 0.9fr) minmax(220px, 1.5fr)
    auto auto;
  align-items: center;
  gap: var(--space-8);
  min-height: 38px;
  min-width: 0;
}
.review-row .review-select {
  justify-content: center;
}
.review-row .review-search-name,
.review-row .review-executable-inline {
  display: grid;
  grid-template-columns: auto minmax(0, 1fr);
  align-items: center;
  gap: 6px;
  margin: 0;
  min-width: 0;
}
.review-row .review-search-name span,
.review-row .review-executable-inline span {
  white-space: nowrap;
}
.review-row .review-search-name input,
.review-row .review-executable-inline input,
.review-row .review-executable-inline select {
  width: 100%;
  min-width: 0;
  height: 30px;
  padding: 4px 7px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.review-row .review-executable-inline {
  margin: 0;
}
.review-duplicate {
  color: var(--warning, #a26b2a);
  font-size: 10px;
  white-space: nowrap;
}
.review-row > .quiet-button {
  min-height: 30px;
  padding: 4px 8px;
  white-space: nowrap;
}
.dialog-actions-right {
  display: flex;
  align-items: center;
  gap: var(--space-8);
}
.manual-match-panel {
  display: grid;
  gap: var(--space-12);
  margin: var(--space-16) 0;
  padding: var(--space-16);
  border: 1px solid var(--accent);
  border-radius: var(--radius-md);
  background: var(--accent-wash);
}
.manual-match-panel header,
.manual-match-toolbar,
.manual-results li {
  display: flex;
  align-items: center;
  gap: var(--space-8);
}
.manual-match-panel header {
  justify-content: space-between;
}
.manual-match-toolbar input {
  min-width: 0;
  flex: 1;
  padding: 8px 10px;
  border: 1px solid var(--border);
  border-radius: var(--radius-control);
  background: var(--surface);
  color: var(--text);
}
.manual-match-toolbar select {
  padding: 8px;
  border: 1px solid var(--border);
  border-radius: var(--radius-control);
  background: var(--surface);
  color: var(--text);
}
.manual-results {
  display: grid;
  gap: 6px;
  max-height: 220px;
  overflow: auto;
  margin: 0;
  padding: 0;
  list-style: none;
}
.manual-results li {
  padding: 8px;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  background: var(--surface);
}
.manual-results li > div:not(.manual-result-cover) {
  display: grid;
  min-width: 0;
  flex: 1;
  gap: 2px;
}
.manual-results span,
.manual-results small,
.manual-empty {
  color: var(--muted);
  font-size: 11px;
}
.manual-results .manual-result-cover {
  flex: 0 0 28px;
  width: 28px;
  height: 40px;
  object-fit: cover;
  border-radius: 3px;
}
.import-error {
  margin: var(--space-12) 0;
  color: var(--danger, #b24b5b);
  font-size: var(--type-small);
}
.import-drop-overlay {
  position: fixed;
  z-index: 100;
  inset: 0;
  display: grid;
  place-items: center;
  background: color-mix(in srgb, var(--surface) 72%, transparent);
  backdrop-filter: blur(8px);
  pointer-events: none;
}
.import-drop-overlay > div {
  display: grid;
  justify-items: center;
  gap: var(--space-8);
  padding: 40px 64px;
  border: 1px solid var(--accent);
  border-radius: var(--radius-lg);
  background: var(--surface);
  color: var(--accent-ink, var(--accent));
  box-shadow: var(--shadow-modal);
}
.import-drop-overlay span {
  color: var(--muted);
  font-size: var(--type-small);
}
:global(:root[data-motion='light'] .review-card[data-scraped='true']::after) {
  animation-duration: 280ms;
}
:global(:root[data-motion='light'] .review-success-mark path) {
  animation-duration: 220ms;
}
:global(:root[data-motion='reduced'] .local-import),
:global(:root[data-motion='reduced'] .review-card[data-scraped='true']::after),
:global(:root[data-motion='reduced'] .review-success-mark path) {
  animation: none;
}
:global(:root[data-motion='reduced'] .review-success-enter-from),
:global(:root[data-motion='reduced'] .review-success-leave-to) {
  transform: none;
}
@media (max-width: 620px) {
  .local-import {
    padding: var(--space-20);
  }
}
</style>

<style scoped>
.review-card[data-failed='true'] {
  border: 1px solid var(--danger);
}
.review-status[data-status='failed'] {
  border: 1px solid var(--danger);
  color: var(--danger);
}
</style>
