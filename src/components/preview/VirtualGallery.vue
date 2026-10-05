<script setup lang="ts">
import {
  computed,
  nextTick,
  onBeforeUpdate,
  onMounted,
  onUnmounted,
  ref,
  watch,
  TransitionGroup,
} from 'vue';
import type { PreviewGame } from '../../preview/data';
import type { GalleryView } from '../../preview/store';
import { galleryWindow, galleryLayouts } from '../../services/virtualGallery';
import CoverCard from './CoverCard.vue';
import LibraryContextMenu from '../LibraryContextMenu.vue';
import type { GameMenuAction } from '../../services/libraryContextActions';
import { appSettings } from '../../stores/settings';
const props = defineProps<{
  games: PreviewGame[];
  view: GalleryView;
  selected: string[];
  busy: boolean;
  selectable?: boolean;
}>();
const emit = defineEmits<{
  select: [id: string];
  action: [id: string, action: GameMenuAction, opener: HTMLElement | null];
}>();
const stage = ref<HTMLElement>();
const metrics = ref({
  ...(galleryLayouts.get(props.view) ?? {
    columns: 3,
    rowHeight: 440,
    gap: 32,
  }),
  offset: 0,
  viewport: 900,
});
const virtual = computed(() => props.games.length > 100);
const range = computed(() =>
  virtual.value
    ? galleryWindow(
        props.games.length,
        metrics.value.columns,
        metrics.value.rowHeight,
        metrics.value.offset,
        metrics.value.viewport,
      )
    : { start: 0, end: props.games.length, top: 0, bottom: 0 },
);
const visible = computed(() =>
  props.games.slice(range.value.start, range.value.end),
);
type CardLayout = { left: number; top: number; width: number; height: number };
const cardLayouts = new WeakMap<HTMLElement, CardLayout>();
const leaveStyles = new WeakMap<HTMLElement, Record<string, string>>();
function cardLayout(element: HTMLElement): CardLayout {
  const bounds = element.getBoundingClientRect();
  const parent = element.offsetParent?.getBoundingClientRect();
  return {
    left: bounds.left - (parent?.left ?? 0),
    top: bounds.top - (parent?.top ?? 0),
    width: bounds.width,
    height: bounds.height,
  };
}
// Capture every card before patching: removing earlier siblings changes grid positions.
onBeforeUpdate(() => {
  const gallery = stage.value?.querySelector('.gallery-stage');
  if (!gallery || gallery.classList.contains('virtual-gallery')) return;
  for (const child of gallery.children) {
    if (
      child instanceof HTMLElement &&
      child.classList.contains('context-card') &&
      !leaveStyles.has(child)
    )
      cardLayouts.set(child, cardLayout(child));
  }
});
function freezeLeavingCard(element: Element) {
  if (!(element instanceof HTMLElement)) return;
  const layout = cardLayouts.get(element) ?? cardLayout(element);
  const saved: Record<string, string> = {};
  for (const property of ['left', 'top', 'width', 'height'] as const) {
    saved[property] = element.style[property];
    element.style[property] = `${layout[property]}px`;
  }
  leaveStyles.set(element, saved);
}
function restoreLeavingCard(element: Element) {
  if (!(element instanceof HTMLElement)) return;
  const saved = leaveStyles.get(element);
  if (!saved) return;
  for (const property of ['left', 'top', 'width', 'height'] as const)
    element.style[property] = saved[property] ?? '';
  leaveStyles.delete(element);
  cardLayouts.delete(element);
}
let frame = 0;
let observer: ResizeObserver | null = null;
function measure() {
  frame = 0;
  const el = stage.value?.querySelector<HTMLElement>('.gallery-stage');
  if (!el) return;
  const style = getComputedStyle(el);
  const columns = Math.max(
    1,
    style.gridTemplateColumns.split(' ').filter(Boolean).length,
  );
  const gap = parseFloat(style.rowGap) || 0;
  const cards = [...el.querySelectorAll<HTMLElement>('.cover-card')].slice(
    0,
    columns,
  );
  const cardHeight = cards.length
    ? Math.max(...cards.map((card) => card.getBoundingClientRect().height))
    : metrics.value.rowHeight - gap;
  galleryLayouts.set(props.view, {
    columns,
    gap,
    rowHeight: Math.max(1, cardHeight + gap),
  });
  metrics.value = {
    columns,
    gap,
    rowHeight: Math.max(1, cardHeight + gap),
    offset: -el.getBoundingClientRect().top,
    viewport: window.innerHeight,
  };
}
function schedule() {
  if (!frame) frame = requestAnimationFrame(measure);
}
watch(
  () => [props.view, props.games, appSettings.value.gallery_columns],
  async () => {
    await nextTick();
    measure();
    await nextTick();
    measure();
  },
);
onMounted(async () => {
  await nextTick();
  measure();
  await nextTick();
  measure();
  window.addEventListener('scroll', schedule, { passive: true });
  window.addEventListener('resize', schedule, { passive: true });
  observer = new ResizeObserver(schedule);
  if (stage.value) observer.observe(stage.value);
});
onUnmounted(() => {
  cancelAnimationFrame(frame);
  observer?.disconnect();
  window.removeEventListener('scroll', schedule);
  window.removeEventListener('resize', schedule);
});
</script>
<template>
  <div ref="stage">
    <component
      :is="virtual ? 'section' : TransitionGroup"
      :key="virtual ? 'visible-stage' : 'animated-stage'"
      tag="section"
      name="gallery"
      class="gallery-stage"
      :style="{ '--gallery-columns': appSettings.value.gallery_columns }"
      :class="[`view-${view}`, { 'virtual-gallery': virtual }]"
      aria-label="作品列表"
      :data-total-games="games.length"
      :data-rendered-games="visible.length"
      @before-leave="freezeLeavingCard"
      @after-leave="restoreLeavingCard"
      @leave-cancelled="restoreLeavingCard"
    >
      <div
        v-if="range.top"
        key="virtual-top"
        class="gallery-spacer"
        aria-hidden="true"
        :style="{ height: `${Math.max(0, range.top - metrics.gap)}px` }"
      ></div>
      <LibraryContextMenu
        v-for="game in visible"
        :key="game.game_id"
        card
        :label="`${game.title}作品操作`"
        :disabled="busy"
        @action="
          (action, opener) => emit('action', game.game_id, action, opener)
        "
      >
        <CoverCard
          :game="game"
          context="gallery"
          draggable-game
          :selectable="selectable"
          :selected="selected.includes(game.game_id)"
          :selection-disabled="busy"
          @select="emit('select', $event)"
        />
      </LibraryContextMenu>
      <div
        v-if="range.bottom"
        key="virtual-bottom"
        class="gallery-spacer"
        aria-hidden="true"
        :style="{ height: `${Math.max(0, range.bottom - metrics.gap)}px` }"
      ></div>
    </component>
  </div>
</template>
<style scoped>
.gallery-spacer {
  grid-column: 1 / -1;
  pointer-events: none;
}
</style>
