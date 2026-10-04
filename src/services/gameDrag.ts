import { shallowRef } from 'vue';
import { preview, notify, errorText } from '../stores/library';
import { addLibraryGamesToGroup } from './gameBatch';
import { moveSidebarGame, sidebarMembers } from './sidebarGameOrder';

interface GameDrag {
  game_id: string;
  title: string;
  x: number;
  y: number;
  group_id: string;
  hint: string;
  allowed: boolean;
  action: 'add' | 'sort';
  target_id: string;
  placement: 'before' | 'after';
}
export const gameDrag = shallowRef<GameDrag | null>(null);
export const gameDropBusy = shallowRef(false);
export const gameDropCompleted = shallowRef<string | null>(null);
let cancelPending: (() => void) | null = null;
export function cancelGameDrag() {
  cancelPending?.();
}

function targetAt(x: number, y: number, sourceGroup: string, gameId: string) {
  const hit = document.elementFromPoint(x, y);
  const row = hit?.closest<HTMLElement>('[data-game-sort-id]');
  const sortGroup = row?.dataset.gameSortGroup;
  if (sourceGroup && row && sortGroup === sourceGroup) {
    const targetId = row.dataset.gameSortId ?? '';
    const members = sidebarMembers(sourceGroup);
    const target = members.find((game) => game.game_id === targetId);
    const placement =
      y < row.getBoundingClientRect().top + row.clientHeight / 2
        ? 'before'
        : 'after';
    return {
      action: 'sort' as const,
      group_id: sourceGroup,
      target_id: targetId,
      placement: placement as 'before' | 'after',
      allowed:
        targetId !== gameId &&
        !!target &&
        members.some((game) => game.game_id === gameId),
      hint:
        targetId === gameId
          ? '拖到同组其他游戏以排序'
          : `放到“${target?.title ?? ''}”${placement === 'before' ? '之前' : '之后'}`,
    };
  }
  const element = hit?.closest<HTMLElement>('[data-game-drop-group]');
  const id = element?.dataset.gameDropGroup ?? '';
  const group = preview.groups.find((group) => group.group_id === id);
  if (group && !group.smart && !group.hidden)
    return {
      action: 'add' as const,
      target_id: '',
      placement: 'after' as const,
      group_id: id,
      allowed: true,
      hint: `加入“${group.name}”`,
    };
  return {
    action: 'add' as const,
    target_id: '',
    placement: 'after' as const,
    group_id: id,
    allowed: false,
    hint: group?.smart
      ? '智能分组按规则自动归类'
      : id
        ? '请拖到自定义分组'
        : '拖到侧栏分组以加入',
  };
}

/** Pointer dragging keeps Tauri's existing native EXE/directory drops intact. */
export function armGameDrag(
  event: PointerEvent,
  game_id: string,
  disabled = false,
) {
  if (
    disabled ||
    gameDropBusy.value ||
    event.button !== 0 ||
    !event.isPrimary ||
    event.pointerType === 'touch' ||
    event.ctrlKey ||
    event.metaKey ||
    event.shiftKey ||
    event.altKey
  )
    return;
  const element = event.target instanceof Element ? event.target : null;
  if (element?.closest('input,select,textarea,[data-favorite]')) return;
  const game = preview.games.find((game) => game.game_id === game_id);
  if (!game) return;
  cancelPending?.();
  const origin = { x: event.clientX, y: event.clientY };
  const sourceGroup =
    element?.closest<HTMLElement>('[data-game-sort-group]')?.dataset
      .gameSortGroup ?? '';
  const pointer_id = event.pointerId;
  let dragging = false;
  let cancelled = false;
  let frame = 0;
  const blockClick = (click: MouseEvent) => {
    click.preventDefault();
    click.stopImmediatePropagation();
  };
  const update = (x: number, y: number) => {
    gameDrag.value = {
      game_id,
      title: game.title,
      x,
      y,
      ...targetAt(x, y, sourceGroup, game_id),
    };
  };
  const scroll = () => {
    const state = gameDrag.value;
    const list = document.querySelector<HTMLElement>(
      '#sidebar-game-overview .groups-scroll',
    );
    if (state && list) {
      const b = list.getBoundingClientRect();
      if (
        state.x >= b.left &&
        state.x <= b.right &&
        state.y >= b.top &&
        state.y <= b.bottom
      ) {
        const delta =
          state.y < b.top + 32 ? -10 : state.y > b.bottom - 32 ? 10 : 0;
        if (delta) {
          list.scrollTop += delta;
          update(state.x, state.y);
        }
      }
    }
    frame = requestAnimationFrame(scroll);
  };
  const cleanup = () => {
    window.removeEventListener('pointermove', move, true);
    window.removeEventListener('pointerup', up, true);
    window.removeEventListener('pointercancel', cancel, true);
    window.removeEventListener('blur', cancel);
    window.removeEventListener('keydown', key, true);
    cancelAnimationFrame(frame);
    gameDrag.value = null;
    cancelPending = null;
    // A drag's following click must not open the game or the target group.
    window.setTimeout(
      () => document.removeEventListener('click', blockClick, true),
      0,
    );
  };
  const cancel = () => cleanup();
  const key = (keyEvent: KeyboardEvent) => {
    if (keyEvent.key !== 'Escape' || !dragging || cancelled) return;
    keyEvent.preventDefault();
    keyEvent.stopImmediatePropagation();
    // Keep the release/click guard until the held pointer is released.
    cancelled = true;
    gameDrag.value = null;
    cancelAnimationFrame(frame);
  };
  const move = (moveEvent: PointerEvent) => {
    if (moveEvent.pointerId !== pointer_id || cancelled) return;
    if (
      !dragging &&
      Math.hypot(moveEvent.clientX - origin.x, moveEvent.clientY - origin.y) < 8
    )
      return;
    if (!dragging) {
      dragging = true;
      document.addEventListener('click', blockClick, true);
      frame = requestAnimationFrame(scroll);
    }
    moveEvent.preventDefault();
    update(moveEvent.clientX, moveEvent.clientY);
  };
  const up = (upEvent: PointerEvent) => {
    if (upEvent.pointerId !== pointer_id) return;
    if (dragging && !cancelled) {
      upEvent.preventDefault();
      update(upEvent.clientX, upEvent.clientY);
    }
    const state = gameDrag.value;
    cleanup();
    if (cancelled || !dragging || !state?.allowed || gameDropBusy.value) return;
    if (state.action === 'sort') {
      gameDropBusy.value = true;
      void moveSidebarGame(
        state.group_id,
        game_id,
        state.target_id,
        state.placement,
      )
        .catch((error) => notify(errorText(error)))
        .finally(() => {
          gameDropBusy.value = false;
        });
      return;
    }
    // Revalidate against live data at release; a filtered/replaced card may be gone.
    const target = preview.groups.find(
      (group) => group.group_id === state.group_id,
    );
    if (
      !target ||
      target.smart ||
      target.hidden ||
      !preview.games.some((game) => game.game_id === game_id)
    )
      return;
    gameDropBusy.value = true;
    void addLibraryGamesToGroup(state.group_id, [game_id])
      .then((error) => {
        if (error) notify(error);
        else gameDropCompleted.value = state.group_id;
      })
      .finally(() => {
        gameDropBusy.value = false;
      });
  };
  cancelPending = cleanup;
  window.addEventListener('pointermove', move, {
    capture: true,
    passive: false,
  });
  window.addEventListener('pointerup', up, true);
  window.addEventListener('pointercancel', cancel, true);
  window.addEventListener('blur', cancel);
  window.addEventListener('keydown', key, true);
}
