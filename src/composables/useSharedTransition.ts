import { nextTick, onMounted, onUnmounted, reactive, ref, watch } from 'vue';
import type { Router } from 'vue-router';
import { preview } from '../stores/library';
import { motionDuration } from './motionPolicy';
import { motion_mode } from './useMotionPolicy';
import {
  TransitionMachine,
  sharedTransform,
  type TransitionRect,
} from './transitionMachine';
interface SourceSnapshot {
  game_id: string;
  from_route: string;
  scroll_top: number;
  focus_key: string;
}
interface OverlayCapture {
  game_id: string;
  cover_url: string;
  title: string;
  accent: string;
  cover_rect: TransitionRect;
  title_rect: TransitionRect;
  font_size: number;
  border_radius: string;
  color: string;
  letter_spacing: string;
}
const machine = new TransitionMachine();
export const transition_debug = reactive({
  phase: 'idle',
  token: 0,
  overlay_count: 0,
});
export const shared_overlay = ref<OverlayCapture | null>(null);
export const overlay_cover = ref<HTMLElement | null>(null);
export const overlay_title = ref<HTMLElement | null>(null);
let router: Router;
let source: SourceSnapshot | null = null;
let animations: Animation[] = [];
let hidden: Array<{ element: HTMLElement; visibility: string }> = [];
function syncDebug() {
  transition_debug.phase = machine.phase;
  transition_debug.token = machine.token;
  transition_debug.overlay_count = shared_overlay.value ? 1 : 0;
  document.documentElement.dataset.sharedTransition = shared_overlay.value
    ? 'active'
    : 'idle';
}
function rect(element: Element): TransitionRect {
  const value = element.getBoundingClientRect();
  return {
    left: value.left,
    top: value.top,
    width: value.width,
    height: value.height,
  };
}
function find(
  role: string,
  game_id: string,
  area?: Element,
): HTMLElement | null {
  return (area ?? document).querySelector<HTMLElement>(
    `[data-shared-${role}="${CSS.escape(game_id)}"]`,
  );
}
function hide(element: HTMLElement | null) {
  if (!element || hidden.some((item) => item.element === element)) return;
  hidden.push({ element, visibility: element.style.visibility });
  element.style.visibility = 'hidden';
}
function clearVisual() {
  for (const animation of animations) animation.cancel();
  animations = [];
  for (const item of hidden) item.element.style.visibility = item.visibility;
  hidden = [];
  shared_overlay.value = null;
  syncDebug();
}
function capture(game_id: string, area?: Element): OverlayCapture | null {
  const game = preview.games.find((item) => item.game_id === game_id);
  if (!game) return null;
  const cover = find('cover', game_id, area);
  const title = find('title', game_id, area);
  if (!game || !cover || !title) return null;
  const cover_rect = rect(cover);
  const title_rect = rect(title);
  if (
    cover_rect.width <= 0 ||
    cover_rect.top >= window.innerHeight ||
    cover_rect.top + cover_rect.height <= 0
  )
    return null;
  return {
    game_id,
    cover_url: game.cover_url,
    title: game.title,
    accent: game.accent,
    cover_rect,
    title_rect,
    font_size: parseFloat(getComputedStyle(title).fontSize),
    border_radius: getComputedStyle(cover).borderRadius,
    color: getComputedStyle(title).color,
    letter_spacing: getComputedStyle(title).letterSpacing,
  };
}
function currentCapture(game_id: string): OverlayCapture | null {
  if (
    shared_overlay.value?.game_id === game_id &&
    overlay_cover.value &&
    overlay_title.value
  ) {
    const value = shared_overlay.value;
    return {
      ...value,
      cover_rect: rect(overlay_cover.value),
      title_rect: rect(overlay_title.value),
      font_size:
        (value.font_size * rect(overlay_title.value).width) /
        Math.max(value.title_rect.width, 1),
    };
  }
  return capture(
    game_id,
    document.querySelector('[data-detail-stage]') ?? undefined,
  );
}
function focusDestination(in_detail: boolean, force = false) {
  const active = document.activeElement;
  if (
    !force &&
    active instanceof HTMLElement &&
    active !== document.body &&
    active.isConnected
  )
    return;
  const target = in_detail
    ? document.querySelector<HTMLElement>('[data-detail-heading]')
    : ((source
        ? document.querySelector<HTMLElement>(
            `[data-preview-open="${CSS.escape(source.focus_key)}"]`,
          )
        : null) ?? document.querySelector<HTMLElement>('[data-page-heading]'));
  target?.focus({ preventScroll: true });
}
function settle() {
  const in_detail = router.currentRoute.value.name === 'game-detail';
  machine.settle(in_detail);
  if (in_detail)
    machine.game_id = String(router.currentRoute.value.params.game_id);
  clearVisual();
  focusDestination(in_detail);
}
async function animateToDestination(token: number, closing: boolean) {
  await nextTick();
  // One frame guarantees target route and restored scroll have committed layout.
  await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
  if (!machine.owns(token)) return;
  const game_id = machine.game_id;
  const area = closing
    ? source && source.game_id === game_id
      ? document.querySelector(
          `[data-preview-card-key="${CSS.escape(source.focus_key)}"]`,
        )
      : null
    : document.querySelector('[data-detail-stage]');
  const target_cover = game_id && area ? find('cover', game_id, area) : null;
  const target_title = game_id && area ? find('title', game_id, area) : null;
  const duration = motionDuration(
    motion_mode.value,
    closing ? 'close' : 'open',
  );
  const capture_value = shared_overlay.value;
  if (
    !capture_value ||
    !target_cover ||
    !target_title ||
    !overlay_cover.value ||
    !overlay_title.value ||
    !duration ||
    !overlay_cover.value.animate
  ) {
    machine.finish(token);
    clearVisual();
    focusDestination(!closing);
    return;
  }
  const destination = rect(target_cover);
  const transform = sharedTransform(capture_value.cover_rect, destination);
  if (
    !transform ||
    destination.top >= window.innerHeight ||
    destination.top + destination.height <= 0 ||
    Math.abs(
      capture_value.cover_rect.width / capture_value.cover_rect.height -
        destination.width / destination.height,
    ) > 0.02
  ) {
    machine.finish(token);
    clearVisual();
    focusDestination(!closing);
    return;
  }
  hide(target_cover);
  hide(target_title);
  const title_destination = rect(target_title);
  const title_scale =
    parseFloat(getComputedStyle(target_title).fontSize) /
    capture_value.font_size;
  const options: KeyframeAnimationOptions = {
    duration,
    easing: 'cubic-bezier(0.22, 1, 0.36, 1)',
    fill: 'forwards',
  };
  animations = [
    overlay_cover.value.animate(
      [
        {
          transform: 'translate(0, 0) scale(1)',
        },
        {
          transform: `translate(${transform.x}px, ${transform.y}px) scale(${transform.scale})`,
        },
      ],
      options,
    ),
    overlay_title.value.animate(
      [
        { transform: 'translate(0, 0) scale(1)' },
        {
          transform: `translate(${title_destination.left - capture_value.title_rect.left}px, ${title_destination.top - capture_value.title_rect.top}px) scale(${title_scale})`,
        },
      ],
      options,
    ),
  ];
  try {
    await Promise.all(animations.map((animation) => animation.finished));
  } catch {
    /* Cancellation is intentional and never a business error. */
  }
  if (!machine.owns(token)) return;
  machine.finish(token);
  clearVisual();
  focusDestination(!closing);
}
export function openPreviewGame(game_id: string, event?: Event) {
  if (!router) return;
  const token = machine.beginOpen(game_id);
  if (token === null) return;
  const area =
    event?.currentTarget instanceof Element
      ? (event.currentTarget.closest('[data-preview-card]') ?? undefined)
      : undefined;
  const captured = capture(game_id, area);
  clearVisual();
  source = {
    game_id,
    from_route:
      router.currentRoute.value.name === 'game-detail' && area && source
        ? source.from_route
        : router.currentRoute.value.fullPath,
    scroll_top:
      router.currentRoute.value.name === 'game-detail' && area && source
        ? source.scroll_top
        : window.scrollY,
    focus_key:
      event?.currentTarget instanceof HTMLElement
        ? (event.currentTarget.dataset.previewOpen ?? game_id)
        : game_id,
  };
  if (motion_mode.value !== 'reduced' && captured) {
    shared_overlay.value = captured;
    hide(find('cover', game_id, area));
    hide(find('title', game_id, area));
  }
  syncDebug();
  void router
    .push({ name: 'game-detail', params: { game_id } })
    .then((failure) => {
      if (failure && machine.owns(token)) settle();
    })
    .catch(() => {
      if (machine.owns(token)) settle();
    });
}
export function returnFromPreviewDetail() {
  const destination = source?.from_route ?? '/games';
  const token = machine.token;
  void router
    .replace(destination)
    .then((failure) => {
      if (failure && machine.owns(token)) settle();
    })
    .catch(() => {
      if (machine.owns(token)) settle();
    });
}
export function useSharedTransitionHost(app_router: Router) {
  router = app_router;
  const remove_before = router.beforeEach((to, from) => {
    if (from.name === 'game-detail' && to.name !== 'game-detail') {
      const game_id = String(from.params.game_id);
      const captured = currentCapture(game_id);
      machine.game_id = game_id;
      if (machine.phase === 'idle') machine.phase = 'detail';
      const token = machine.beginClose();
      clearVisual();
      if (token !== null && captured && motion_mode.value !== 'reduced') {
        shared_overlay.value = captured;
        const detail_area =
          document.querySelector('[data-detail-stage]') ?? undefined;
        hide(find('cover', game_id, detail_area));
        hide(find('title', game_id, detail_area));
      }
      syncDebug();
    } else if (to.name !== 'game-detail' && from.name !== 'game-detail') {
      machine.settle(false);
      clearVisual();
    }
  });
  const remove_after = router.afterEach(async (to, from, failure) => {
    // A superseded router navigation must not settle a newer animation intent.
    if (failure) return;
    if (
      to.name === 'game-detail' &&
      machine.phase === 'opening' &&
      machine.game_id === String(to.params.game_id)
    ) {
      const token = machine.token;
      window.scrollTo({ top: 0 });
      await nextTick();
      if (
        !machine.owns(token) ||
        router.currentRoute.value.fullPath !== to.fullPath
      )
        return;
      focusDestination(true, true);
      void animateToDestination(token, false);
    } else if (from.name === 'game-detail' && to.name !== 'game-detail') {
      const token = machine.token;
      await nextTick();
      if (
        !machine.owns(token) ||
        router.currentRoute.value.fullPath !== to.fullPath
      )
        return;
      window.scrollTo({
        top:
          source && to.fullPath === source.from_route ? source.scroll_top : 0,
      });
      focusDestination(false, true);
      if (source && to.fullPath !== source.from_route) {
        machine.finish(token);
        clearVisual();
        focusDestination(false);
      } else void animateToDestination(token, true);
    } else {
      machine.settle(to.name === 'game-detail');
      if (to.name === 'game-detail') {
        machine.game_id = String(to.params.game_id);
        if (source?.game_id !== machine.game_id) source = null;
      }
      const token = machine.token;
      clearVisual();
      await nextTick();
      if (
        !machine.owns(token) ||
        router.currentRoute.value.fullPath !== to.fullPath
      )
        return;
      window.scrollTo({ top: 0 });
      focusDestination(to.name === 'game-detail', true);
    }
  });
  const onResize = () => {
    if (shared_overlay.value) settle();
  };
  const onWheel = () => {
    if (shared_overlay.value) settle();
  };
  const onEscape = (event: KeyboardEvent) => {
    if (
      event.key !== 'Escape' ||
      event.defaultPrevented ||
      preview.dialog ||
      document.querySelector(
        'dialog[open], [data-sidebar-overlay][data-state="open"], [data-detail-overlay][data-state="open"]',
      )
    )
      return;
    if (
      router.currentRoute.value.name === 'game-detail' ||
      machine.phase === 'opening'
    ) {
      event.preventDefault();
      returnFromPreviewDetail();
    }
  };
  const stop_motion_watch = watch(motion_mode, (mode) => {
    if (mode === 'reduced' && shared_overlay.value) settle();
  });
  onMounted(() => {
    window.addEventListener('resize', onResize);
    window.addEventListener('wheel', onWheel, { passive: true });
    window.addEventListener('keydown', onEscape);
  });
  onUnmounted(() => {
    remove_before();
    remove_after();
    stop_motion_watch();
    window.removeEventListener('resize', onResize);
    window.removeEventListener('wheel', onWheel);
    window.removeEventListener('keydown', onEscape);
    machine.settle(false);
    clearVisual();
    source = null;
  });
}
