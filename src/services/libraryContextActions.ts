import { shallowRef } from 'vue';
export type LibraryMenuAction = 'rename' | 'delete' | 'manage';
export type GameMenuAction =
  LibraryMenuAction | 'add_to_group' | 'remove_from_group';
export const gameActionRequest = shallowRef<{
  game_id: string;
  group_id?: string;
  action: GameMenuAction;
  opener: HTMLElement | null;
} | null>(null);
export function requestGameAction(
  game_id: string,
  action: GameMenuAction,
  opener: HTMLElement | null,
  group_id?: string,
) {
  gameActionRequest.value = { game_id, group_id, action, opener };
}
export const groupActionRequest = shallowRef<{
  group_id: string;
  action: LibraryMenuAction;
  opener: HTMLElement | null;
} | null>(null);
export function requestGroupAction(
  group_id: string,
  action: GameMenuAction,
  opener: HTMLElement | null,
) {
  if (action === 'add_to_group' || action === 'remove_from_group') return;
  groupActionRequest.value = { group_id, action, opener };
}
