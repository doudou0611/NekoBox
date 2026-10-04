import { computed, onMounted, onUnmounted, reactive, ref } from 'vue';
import {
  clamp,
  DEFAULT_SIDEBAR_WIDTH,
  DEFAULT_TOP_RATIO,
  sidebarPartition,
  sidebarWidthBounds,
} from '../preview/sidebarLayout';

export const sidebar_layout = reactive({
  width: DEFAULT_SIDEBAR_WIDTH,
  top_ratio: DEFAULT_TOP_RATIO,
});
export function resetSidebarLayout() {
  sidebar_layout.width = DEFAULT_SIDEBAR_WIDTH;
  sidebar_layout.top_ratio = DEFAULT_TOP_RATIO;
}
export function useSidebarLayout() {
  const panels = ref<HTMLElement>();
  const viewport_width = ref(window.innerWidth);
  const panel_height = ref(window.innerHeight - 140);
  const bounds = computed(() => sidebarWidthBounds(viewport_width.value));
  const width = computed(() =>
    clamp(sidebar_layout.width, bounds.value.min, bounds.value.max),
  );
  const partition = computed(() =>
    sidebarPartition(panel_height.value, sidebar_layout.top_ratio),
  );
  const dragging = ref<'width' | 'split' | null>(null);
  let pointer_id = -1;
  let start_x = 0;
  let start_y = 0;
  let start_size = 0;
  let observer: ResizeObserver | undefined;
  const updateViewport = () => {
    viewport_width.value = window.innerWidth;
  };
  onMounted(() => {
    observer = new ResizeObserver(() => {
      panel_height.value = panels.value?.clientHeight ?? 0;
    });
    if (panels.value) observer.observe(panels.value);
    window.addEventListener('resize', updateViewport);
  });
  onUnmounted(() => {
    observer?.disconnect();
    window.removeEventListener('resize', updateViewport);
  });
  function beginResize(axis: 'width' | 'split', event: PointerEvent) {
    if (event.button !== 0 || !event.isPrimary) return;
    event.preventDefault();
    dragging.value = axis;
    pointer_id = event.pointerId;
    start_x = event.clientX;
    start_y = event.clientY;
    start_size = axis === 'width' ? width.value : partition.value.top;
    (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
  }
  function moveResize(event: PointerEvent) {
    if (event.pointerId !== pointer_id || !dragging.value) return;
    if (dragging.value === 'width') {
      sidebar_layout.width = clamp(
        start_size + event.clientX - start_x,
        bounds.value.min,
        bounds.value.max,
      );
    } else {
      const { min, max, available } = partition.value;
      sidebar_layout.top_ratio =
        clamp(start_size + event.clientY - start_y, min, max) / available;
    }
  }
  function endResize(event?: PointerEvent) {
    if (event && event.pointerId !== pointer_id) return;
    dragging.value = null;
    pointer_id = -1;
  }
  function onResizeKey(axis: 'width' | 'split', event: KeyboardEvent) {
    const decrement = axis === 'width' ? 'ArrowLeft' : 'ArrowUp';
    const increment = axis === 'width' ? 'ArrowRight' : 'ArrowDown';
    if (![decrement, increment, 'Home', 'End'].includes(event.key)) return;
    event.preventDefault();
    const { min, max } = axis === 'width' ? bounds.value : partition.value;
    const current = axis === 'width' ? width.value : partition.value.top;
    const value =
      event.key === 'Home'
        ? min
        : event.key === 'End'
          ? max
          : clamp(current + (event.key === decrement ? -12 : 12), min, max);
    if (axis === 'width') sidebar_layout.width = value;
    else sidebar_layout.top_ratio = value / partition.value.available;
  }
  return {
    panels,
    width,
    bounds,
    partition,
    dragging,
    beginResize,
    moveResize,
    endResize,
    onResizeKey,
  };
}
