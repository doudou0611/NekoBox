import { computed, onMounted, onUnmounted, ref, watchEffect } from 'vue';
import { resolveMotion, shouldAnimateDecorations } from './motionPolicy';
const system_reduced = ref(
  typeof window !== 'undefined' &&
    window.matchMedia('(prefers-reduced-motion: reduce)').matches,
);
const document_hidden = ref(false);
export const motion_mode = computed(() =>
  resolveMotion('full', system_reduced.value),
);
export function useMotionPolicy() {
  let media: MediaQueryList | undefined;
  const updateMedia = () => {
    system_reduced.value = media?.matches ?? false;
  };
  const updateVisibility = () => {
    document_hidden.value = document.hidden;
  };
  onMounted(() => {
    media = window.matchMedia('(prefers-reduced-motion: reduce)');
    updateMedia();
    updateVisibility();
    media.addEventListener('change', updateMedia);
    document.addEventListener('visibilitychange', updateVisibility);
  });
  watchEffect(() => {
    document.documentElement.dataset.motion = motion_mode.value;
    document.documentElement.dataset.decorations = shouldAnimateDecorations(
      motion_mode.value,
      document_hidden.value,
    )
      ? 'running'
      : 'paused';
  });
  onUnmounted(() => {
    media?.removeEventListener('change', updateMedia);
    document.removeEventListener('visibilitychange', updateVisibility);
  });
  return { motion_mode, system_reduced };
}
