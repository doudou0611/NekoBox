import type { PreviewGame } from '../preview/data';
import { sidebarGroups } from '../preview/groups';
import { preview } from '../stores/library';
import {
  appSettings,
  loadAppSettings,
  saveAppSettings,
} from '../stores/settings';

export const COMPACT_GAME_ORDER = 'sidebar-all';

export function orderedSidebarGames(games: PreviewGame[], groupId: string) {
  const ids = appSettings.value.sidebar_game_order?.[groupId] ?? [];
  const rank = new Map(ids.map((id, index) => [id, index]));
  return [...games].sort(
    (a, b) =>
      (rank.get(a.game_id) ?? ids.length) - (rank.get(b.game_id) ?? ids.length),
  );
}

export function sidebarMembers(groupId: string): PreviewGame[] {
  const games = [...preview.games].sort(
    (a, b) => b.added_order - a.added_order,
  );
  return orderedSidebarGames(
    groupId === COMPACT_GAME_ORDER
      ? games
      : (sidebarGroups(games, preview.groups).find(
          (group) => group.group_id === groupId,
        )?.games ?? []),
    groupId,
  );
}

let pending: Promise<unknown> = Promise.resolve();
export function moveSidebarGame(
  groupId: string,
  gameId: string,
  targetId: string,
  placement: 'before' | 'after',
): Promise<void> {
  const operation = pending
    .catch(() => {})
    .then(async () => {
      await loadAppSettings();
      // Re-read live members and the last saved order after preceding saves finish.
      const ids = sidebarMembers(groupId).map((game) => game.game_id);
      if (gameId === targetId) return;
      const index = ids.indexOf(gameId);
      if (index < 0 || !ids.includes(targetId))
        throw Error('作品或分组已变化，请重新拖动。');
      ids.splice(index, 1);
      ids.splice(
        ids.indexOf(targetId) + (placement === 'after' ? 1 : 0),
        0,
        gameId,
      );
      await saveAppSettings(
        {
          ...appSettings.value,
          sidebar_game_order: {
            ...appSettings.value.sidebar_game_order,
            [groupId]: ids,
          },
        },
        ['sidebar_game_order'],
      );
    });
  pending = operation;
  return operation;
}
