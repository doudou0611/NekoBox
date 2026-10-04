import { reactive } from 'vue';
import { createPreviewGames, type PreviewStatus } from './data';
import type { MotionPreference } from '../composables/motionPolicy';
import { validateGroupName, type PreviewGroup } from './groups';
import type { GameQuery } from '../types/domain';
import type { PaletteId } from './palettes';
export type GalleryView = 'grid' | 'list' | 'wall';
interface PreviewToast {
  toast_id: number;
  message: string;
}
interface PreviewDialog {
  title: string;
  description: string;
  confirm_label: string;
  action?: () => void;
  opener?: () => HTMLElement | null;
}
export const preview = reactive({
  games: createPreviewGames(),
  groups: [] as PreviewGroup[],
  query: '',
  status_filter: 'all' as PreviewStatus | 'all' | 'favorite',
  sort: 'recent' as 'recent' | 'title' | 'playtime',
  gallery_view: 'grid' as GalleryView,
  gallery_query: {
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
    filters: {},
  } as GameQuery,
  gallery_state: {
    source: '',
    tag: '',
    year: '',
    developer: '',
    playtime: '',
    backup: '',
    multiple: '',
    recent: '',
    system: 'all',
    collection_id: '',
    scroll_top: 0,
  },
  theme: 'light' as 'dark' | 'light',
  palette: 'wisteria' as PaletteId,
  motion_preference: 'system' as MotionPreference,
  show_empty: false,
  missing_cover: false,
  home_visited: false,
  toasts: [] as PreviewToast[],
  dialog: null as PreviewDialog | null,
});
let toast_id = 0;
const timers = new Map<number, ReturnType<typeof setTimeout>>();
export function notify(message: string) {
  const id = ++toast_id;
  preview.toasts.push({ toast_id: id, message });
  if (preview.toasts.length > 3) dismissToast(preview.toasts[0]!.toast_id);
  timers.set(
    id,
    setTimeout(() => dismissToast(id), 5200),
  );
}
export function dismissToast(id: number) {
  clearTimeout(timers.get(id));
  timers.delete(id);
  preview.toasts = preview.toasts.filter((toast) => toast.toast_id !== id);
}
export function showDemoAction(action: string) {
  notify(`原型演示：${action}尚未接入。没有运行游戏，也未读取或修改本地文件。`);
}
export function toggleFavorite(game_id: string) {
  const game = preview.games.find((item) => item.game_id === game_id);
  if (!game) return;
  game.favorite = !game.favorite;
  notify(`演示收藏已${game.favorite ? '添加' : '取消'}，仅在本次预览内生效。`);
}
export function savePreviewGroup(
  name: string,
  game_ids: string[],
  group_id?: string,
): string | null {
  const error = validateGroupName(name, preview.groups, group_id);
  if (error) return error;
  const existing = preview.groups.find((group) => group.group_id === group_id);
  if (group_id && !existing) return '分组已不存在，请重新打开分组编辑器。';
  const valid_ids = new Set(preview.games.map((game) => game.game_id));
  const members = [...new Set(game_ids)].filter((id) => valid_ids.has(id));
  if (existing) {
    existing.name = name.trim();
    existing.game_ids = members;
  } else {
    preview.groups.push({
      group_id: `demo-group-${crypto.randomUUID()}`,
      name: name.trim(),
      game_ids: members,
    });
  }
  notify(
    `分组“${name.trim()}”已${existing ? '更新' : '创建'}，仅本次演示有效。`,
  );
  return null;
}
export function deletePreviewGroup(group_id: string) {
  preview.groups = preview.groups.filter(
    (group) => group.group_id !== group_id,
  );
  notify('分组已删除，游戏与收藏保留。');
}
export function resetPreview() {
  preview.games = createPreviewGames();
  preview.groups = [];
  preview.query = '';
  preview.status_filter = 'all';
  preview.show_empty = false;
  preview.missing_cover = false;
  notify('演示数据已重置；未访问正式数据库。');
}
export function clearPreviewTimers() {
  for (const timer of timers.values()) clearTimeout(timer);
  timers.clear();
  preview.toasts = [];
}
