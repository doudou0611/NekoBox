import { reactive, toRefs } from 'vue';
import { isTauri, convertFileSrc } from '@tauri-apps/api/core';
import {
  preview as demo,
  toggleFavorite as demoFavorite,
  deletePreviewGroup as demoDeleteGroup,
  showDemoAction as demoAction,
} from '../preview/store';
import type { PreviewGame } from '../preview/data';
import { validateGroupName, type PreviewGroup } from '../preview/groups';
import type {
  GameSummary,
  GameDetail,
  GameQuery,
  GameStatus,
  CollectionDetail,
  Recommendation,
  HomeSummary,
  SaveCollectionRequest,
} from '../types/domain';
import type { BackendStatus, ScanReport } from '../types/local';
import type {
  CommandName,
  CommandPayloads,
  CommandResults,
} from '../types/api';
import { invokeApi, ApiClientError, type ApiOptions } from '../services/client';
import { notify } from '../preview/store';
export {
  notify,
  dismissToast,
  resetPreview,
  clearPreviewTimers,
} from '../preview/store';

export const desktop = isTauri();
export function showDemoAction(action: string) {
  if (desktop) notify(`${action}尚未接入。`);
  else demoAction(action);
}
/** Layout preferences may share UI state; production games never come from fixtures. */
export const preview = desktop
  ? reactive({
      ...toRefs(demo),
      games: [] as PreviewGame[],
      groups: [] as PreviewGroup[],
    })
  : demo;
export const local = reactive({
  loading: false,
  error: '',
  recommendation_error: '',
  home_error: '',
  status: null as BackendStatus | null,
  records: {} as Record<string, GameSummary | GameDetail>,
  collections: {} as Record<string, CollectionDetail>,
  recommendations: [] as Recommendation[],
  home: null as HomeSummary | null,
  scan: null as ScanReport | null,
  import_open: false,
});
let libraryRevision = 0;
const libraryWrites = new Set<CommandName>([
  'update_game',
  'update_game_metadata',
  'set_metadata_lock',
  'replace_game_tags',
  'confirm_metadata_match',
  'unbind_metadata',
  'remove_game',
  'save_collection',
]);
export async function api<K extends CommandName>(
  command: K,
  payload: CommandPayloads[K],
  options?: ApiOptions,
): Promise<CommandResults[K]> {
  const response = await invokeApi(command, payload, options);
  if (!response.success)
    throw new ApiClientError(response.error_code, response.message);
  if (libraryWrites.has(command)) libraryRevision++;
  return response.data;
}
export function errorText(error: unknown): string {
  return error instanceof ApiClientError
    ? error.message
    : error instanceof Error
      ? error.message
      : '操作失败，请重试。';
}
export function cleanDescription(value: string): string {
  return value
    .replace(/\[br\s*\/?\]/gi, '\n')
    .replace(/\[[^\]]+\]/g, '')
    .replace(/\r/g, '')
    .split('\n')
    .map((line) => line.trim())
    .filter(Boolean)
    .join('\n\n');
}
export const query = (overrides: Partial<GameQuery> = {}): GameQuery => ({
  page: 1,
  page_size: 100,
  search: '',
  statuses: [],
  sources: [],
  tag_ids: [],
  favorite: null,
  collection_id: null,
  sort: 'added_at',
  direction: 'desc',
  ...overrides,
});
export function displayGame(
  game: GameSummary,
  detail?: GameDetail,
): PreviewGame {
  return {
    game_id: game.id,
    title: game.title,
    subtitle:
      game.title_ja?.trim() && game.title_ja.trim() !== game.title.trim()
        ? game.title_ja.trim()
        : '',
    developer: game.developer ?? '开发商待补充',
    publisher: game.publisher ?? undefined,
    release_date: game.release_date ?? undefined,
    source_rating: game.source_rating,
    source_tags: game.source_tags,
    cover_url: coverSource(game.cover_url),
    accent: 'var(--accent)',
    status: game.status,
    favorite: game.favorite,
    duration_minutes: Math.floor(game.total_playtime_seconds / 60),
    added_order: Date.parse(game.added_at),
    tags: game.tags.map((t) => t.name),
    tag_ids: game.tags.map((t) => t.id),
    year: game.release_date?.slice(0, 4) ?? '',
    sources: [...new Set(game.installations.map((i) => i.source))],
    last_played_order: game.last_played_at
      ? Date.parse(game.last_played_at)
      : 0,
    launchable: game.installations.some(
      (i) => i.path_valid && Boolean(i.executable_path),
    ),
    playtime_seconds: game.total_playtime_seconds,
    install_count: game.installations.length,
    has_save_backup: game.has_save_backup,
    metadata_incomplete: !game.cover_url?.trim() || !game.developer?.trim(),
    metadata_pending: game.metadata_status === 'pending_confirmation',
    description: detail?.description
      ? cleanDescription(detail.description)
      : '尚未填写作品简介。当前资料来自本地目录与手动配置。',
  };
}
export function coverSource(path: string | null): string {
  if (!path) return '';
  if (!desktop || /^(https?:|asset:|data:)/i.test(path)) return path;
  if (/^covers\/[a-f0-9]{64}\.(jpg|png|webp|gif)$/.test(path)) {
    return convertFileSrc(path.slice('covers/'.length), 'cover');
  }
  return convertFileSrc(path);
}
async function allGames(q: GameQuery): Promise<GameSummary[]> {
  const first = await api('list_games', q);
  const games = [...first.items];
  for (let page = 2; page <= Math.ceil(first.total / q.page_size); page++) {
    const next = await api('list_games', { ...q, page });
    games.push(...next.items);
  }
  return games;
}
function loadedDetail(
  record?: GameSummary | GameDetail,
): GameDetail | undefined {
  return record && 'description' in record && 'metadata' in record
    ? record
    : undefined;
}
let refresh_version = 0;
let detailRefreshPending = false;
export async function refreshLibrary(
  options: { reloadDetails?: boolean } = {},
) {
  if (!desktop) return;
  if (options.reloadDetails) detailRefreshPending = true;
  const reloadDetails = detailRefreshPending;
  const version = ++refresh_version;
  const revision = libraryRevision;
  local.loading = true;
  try {
    const [status, games, collections, recommendations, home] =
      await Promise.all([
        api('backend_status', {}),
        allGames(query()),
        api('list_collections', {}),
        api('get_recommendations', { limit: 4, excluded_game_ids: [] }).then(
          (value) => ({ value, error: '' }),
          (error: unknown) => ({ value: null, error: errorText(error) }),
        ),
        api('get_home_summary', {}).then(
          (value) => ({ value, error: '' }),
          (error: unknown) => ({ value: null, error: errorText(error) }),
        ),
      ]);
    const details = await Promise.all(
      collections.map(async (c) => {
        const detail = await api('get_collection', { collection_id: c.id });
        if (detail.query) {
          detail.member_ids = (
            await allGames(query({ collection_id: c.id }))
          ).map((g) => g.id);
        }
        return detail;
      }),
    );
    const refreshedDetails = new Map<string, GameDetail>();
    if (reloadDetails) {
      await Promise.all(
        games
          .filter((g) => loadedDetail(local.records[g.id]))
          .map(async (g) => {
            refreshedDetails.set(
              g.id,
              await api('get_game', { game_id: g.id }),
            );
          }),
      );
    }
    if (version !== refresh_version) return;
    if (revision !== libraryRevision) {
      await refreshLibrary(options);
      return;
    }
    if (reloadDetails) {
      detailRefreshPending = false;
      libraryRevision++;
    }
    local.status = status;
    // List responses are summaries, not a signal that detail fields were cleared.
    // Merge at commit time so a detail loaded during this request also survives.
    // Build only from current IDs to evict genuinely removed games.
    local.records = Object.fromEntries(
      games.map((g) => {
        const detail =
          refreshedDetails.get(g.id) ?? loadedDetail(local.records[g.id]);
        return [g.id, detail ? { ...detail, ...g } : g];
      }),
    );
    local.collections = Object.fromEntries(details.map((c) => [c.id, c]));
    if (recommendations.value) local.recommendations = recommendations.value;
    local.recommendation_error = recommendations.error;
    if (home.value) local.home = home.value;
    local.home_error = home.error;
    preview.games = games
      .filter((g) => !g.hidden)
      .map((g) => displayGame(g, loadedDetail(local.records[g.id])));
    preview.groups = details.map((c) => ({
      group_id: c.id,
      name: c.name,
      game_ids: c.member_ids,
      smart: c.kind === 'smart',
      member_source: 'database',
      hidden: c.hidden,
      icon: c.icon,
      color: c.color,
      cover_url: c.cover_url,
      query: c.query,
    }));
    local.error = '';
    if (!local.scan && status.last_scan_task_id)
      local.scan = await api('get_scan_task', {
        task_id: status.last_scan_task_id,
      });
  } catch (error) {
    if (version === refresh_version) local.error = errorText(error);
  } finally {
    if (version === refresh_version) local.loading = false;
  }
}
export async function refreshRecommendations(excluded_game_ids: string[] = []) {
  if (!desktop) return;
  local.recommendations = await api('get_recommendations', {
    limit: 4,
    excluded_game_ids,
  });
}
export async function setRecommendationPreference(
  game_id: string,
  preference: 'not_interested' | 'snoozed' | 'none',
  expires_at: string | null = null,
) {
  if (!desktop) return;
  await api('set_recommendation_preference', {
    game_id,
    preference,
    expires_at,
  });
  await refreshLibrary();
}
export function applyGameDetail(detail: GameDetail) {
  libraryRevision++;
  local.records[detail.id] = detail;
  const index = preview.games.findIndex((g) => g.game_id === detail.id);
  if (index >= 0) preview.games[index] = displayGame(detail, detail);
}
export async function loadDetail(game_id: string): Promise<void> {
  if (!desktop) return;
  try {
    const revision = libraryRevision;
    const detail = await api('get_game', { game_id });
    if (revision !== libraryRevision) return loadDetail(game_id);
    applyGameDetail(detail);
  } catch (error) {
    notify(errorText(error));
  }
}
const mutations = new Map<string, Promise<boolean>>();
export function updateLibraryGame(
  id: string,
  patch: (game: GameSummary) => Partial<GameSummary>,
  refreshSmart = true,
) {
  const previous = mutations.get(id) ?? Promise.resolve(false);
  const operation = previous
    .then(async () => {
      const game = local.records[id];
      if (!game) return false;
      const next = { ...game, ...patch(game) };
      const detail = await api('update_game', {
        game_id: id,
        title: next.title,
        status: next.status,
        favorite: next.favorite,
        hidden: next.hidden,
        user_rating: next.user_rating,
      });
      local.records[id] = detail;
      const index = preview.games.findIndex((g) => g.game_id === id);
      if (index >= 0) {
        if (detail.hidden) preview.games.splice(index, 1);
        else preview.games[index] = displayGame(detail, detail);
      }
      if (
        refreshSmart &&
        Object.values(local.collections).some((c) => c.kind === 'smart')
      )
        await refreshLibrary();
      return true;
    })
    .catch((error) => {
      notify(errorText(error));
      return false;
    });
  mutations.set(id, operation);
  void operation.finally(() => {
    if (mutations.get(id) === operation) mutations.delete(id);
  });
  return operation;
}
export function toggleFavorite(id: string) {
  if (!desktop) {
    demoFavorite(id);
    return;
  }
  void updateLibraryGame(id, (g) => ({ favorite: !g.favorite }));
}
export async function setStatus(id: string, status: GameStatus) {
  if (!desktop) {
    const game = preview.games.find((g) => g.game_id === id);
    if (game) game.status = status;
    notify('演示状态已更新，未写入数据库。');
    return;
  }
  await updateLibraryGame(id, () => ({ status }));
}
export async function batchUpdateLibraryGames(
  ids: string[],
  patch: Partial<Pick<GameSummary, 'favorite' | 'status'>>,
) {
  const succeeded: string[] = [];
  const failed: string[] = [];
  for (const id of [...new Set(ids)]) {
    if (!desktop) {
      const game = preview.games.find((item) => item.game_id === id);
      if (game) {
        Object.assign(game, patch);
        succeeded.push(id);
      } else failed.push(id);
    } else if (await updateLibraryGame(id, () => patch, false))
      succeeded.push(id);
    else failed.push(id);
  }
  if (desktop) await refreshLibrary();
  notify(
    `${desktop ? '' : '演示：'}已更新 ${succeeded.length} 部作品${failed.length ? `，${failed.length} 部失败，可重试` : ''}。`,
  );
  return { succeeded, failed };
}
export async function saveLibraryCollection(
  request: SaveCollectionRequest,
): Promise<string | null> {
  const error = validateGroupName(
    request.name,
    preview.groups,
    request.id ?? undefined,
  );
  if (error) return error;
  if (!desktop) {
    const group: PreviewGroup = {
      group_id: request.id ?? `demo-group-${crypto.randomUUID()}`,
      name: request.name.trim(),
      game_ids: request.member_ids ?? [],
      smart: request.kind === 'smart',
      query: request.query,
      icon: request.icon,
      color: request.color,
      cover_url: request.cover_url,
      hidden: request.hidden,
    };
    const index = preview.groups.findIndex(
      (g) => g.group_id === group.group_id,
    );
    if (index < 0) preview.groups.push(group);
    else preview.groups[index] = group;
    notify('演示分组已保存，仅本次预览有效。');
    return null;
  }
  try {
    await api('save_collection', request);
    await refreshLibrary();
    notify('分组已保存到便携数据库。');
    return null;
  } catch (error) {
    return errorText(error);
  }
}
export async function saveSmartCollection(
  name: string,
  filters: GameQuery,
): Promise<string | null> {
  return saveLibraryCollection({
    id: null,
    name,
    kind: 'smart',
    icon: 'spark',
    color: null,
    cover_url: null,
    position: preview.groups.length,
    hidden: false,
    query: { ...filters, collection_id: null },
    member_ids: null,
  });
}
let reordering = false;
export async function movePreviewGroup(
  groupId: string,
  beforeId: string,
  placement: 'before' | 'after' = 'before',
): Promise<string | null> {
  if (reordering) return '正在保存分组顺序，请稍后。';
  const next = [...preview.groups];
  const index = next.findIndex((g) => g.group_id === groupId);
  if (
    index < 0 ||
    groupId === beforeId ||
    !next.some((g) => g.group_id === beforeId)
  )
    return null;
  const [group] = next.splice(index, 1);
  next.splice(
    next.findIndex((g) => g.group_id === beforeId) +
      (placement === 'after' ? 1 : 0),
    0,
    group,
  );
  reordering = true;
  try {
    if (desktop)
      await api('reorder_collections', {
        collection_ids: next.map((g) => g.group_id),
      });
    preview.groups = next;
    if (desktop) await refreshLibrary();
    return null;
  } catch (error) {
    await refreshLibrary();
    return errorText(error);
  } finally {
    reordering = false;
  }
}
export async function reorderPreviewGroup(
  groupId: string,
  direction: 'up' | 'down',
): Promise<string | null> {
  const groups = preview.groups;
  const index = groups.findIndex((g) => g.group_id === groupId);
  const target = direction === 'up' ? index - 1 : index + 1;
  if (index < 0 || target < 0 || target >= groups.length) return null;
  return direction === 'up'
    ? movePreviewGroup(groupId, groups[target].group_id)
    : movePreviewGroup(groups[target].group_id, groupId);
}
function normalizeCollectionQuery(value: GameQuery): GameQuery {
  const result: GameQuery = JSON.parse(JSON.stringify(value));
  const filters = result.filters ?? {};
  filters.developer = filters.developer?.trim() || null;
  filters.release_year = Number(filters.release_year) || null;
  result.filters = filters;
  return result;
}
export async function savePreviewGroup(
  name: string,
  game_ids: string[],
  group_id?: string,
  options: Partial<
    Pick<
      SaveCollectionRequest,
      'icon' | 'color' | 'cover_url' | 'hidden' | 'query'
    >
  > = {},
): Promise<string | null> {
  const existing = group_id
    ? preview.groups.find((g) => g.group_id === group_id)
    : null;
  return saveLibraryCollection({
    id: group_id ?? null,
    name,
    kind: existing?.smart ? 'smart' : 'normal',
    icon: options.icon !== undefined ? options.icon : (existing?.icon ?? null),
    color:
      options.color !== undefined ? options.color : (existing?.color ?? null),
    cover_url:
      options.cover_url !== undefined
        ? options.cover_url
        : (existing?.cover_url ?? null),
    position: group_id
      ? preview.groups.findIndex((g) => g.group_id === group_id)
      : preview.groups.length,
    hidden: options.hidden ?? existing?.hidden ?? false,
    query: existing?.smart
      ? normalizeCollectionQuery(options.query ?? existing.query ?? query())
      : null,
    member_ids: existing?.smart ? null : game_ids,
  });
}
export async function deletePreviewGroup(id: string): Promise<string | null> {
  if (!desktop) {
    demoDeleteGroup(id);
    return null;
  }
  try {
    await api('delete_collection', { id, confirmed: true });
    await refreshLibrary();
    notify('分组已删除，游戏与本地文件保留。');
    return null;
  } catch (error) {
    return errorText(error);
  }
}
export async function launchGame(id: string) {
  if (!desktop) {
    notify('原型演示：没有运行游戏。');
    return;
  }
  const game = local.records[id];
  const installs = game?.installations.filter(
    (i) => i.path_valid && i.executable_path,
  );
  if (!installs?.length) {
    notify('请在详情的版本与来源中选择并保存启动入口。');
    return;
  }
  if (installs.length > 1) {
    notify('此作品有多个安装版本，请在版本与来源中明确选择要启动的版本。');
    return;
  }
  await launchInstallation(installs[0]!.id);
}
export async function launchInstallation(install_id: string) {
  try {
    await api('launch_game', { install_id, options: { user_initiated: true } });
    notify('Windows 已创建游戏进程。');
    await refreshLibrary();
  } catch (error) {
    notify(errorText(error));
  }
}
export function openImport() {
  local.import_open = true;
}
