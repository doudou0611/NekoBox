import { computed, onUnmounted, ref, watch } from 'vue';
import {
  coverAtmosphereTokens,
  extractCoverPalette,
  type CoverPalette,
} from '../services/coverPalette';

const cache = new Map<string, Promise<CoverPalette | null>>();

function sampleCover(url: string): Promise<CoverPalette | null> {
  const cached = cache.get(url);
  if (cached) return cached;
  const pending = new Promise<CoverPalette | null>((resolve) => {
    const image = new Image();
    // The local cover protocol allows the app origin; never taint the canvas.
    image.crossOrigin = 'anonymous';
    const timer = setTimeout(() => finish(null), 8000);
    function finish(palette: CoverPalette | null) {
      clearTimeout(timer);
      image.onload = null;
      image.onerror = null;
      resolve(palette);
    }
    image.onerror = () => finish(null);
    image.onload = () => {
      try {
        const canvas = document.createElement('canvas');
        canvas.width = canvas.height = 32;
        const context = canvas.getContext('2d', { willReadFrequently: true });
        if (!context) return finish(null);
        context.drawImage(image, 0, 0, 32, 32);
        finish(extractCoverPalette(context.getImageData(0, 0, 32, 32).data));
      } catch {
        finish(null);
      }
    };
    image.src = url;
  });
  cache.set(url, pending);
  if (cache.size > 64) cache.delete(cache.keys().next().value!);
  void pending.then((palette) => {
    if (!palette && cache.get(url) === pending) cache.delete(url);
  });
  return pending;
}

export function useCoverAtmosphere(
  cover: () => string | undefined,
  theme: () => 'light' | 'dark',
) {
  const palette = ref<CoverPalette | null>(null);
  let revision = 0;
  watch(
    cover,
    async (url) => {
      const owner = ++revision;
      palette.value = null;
      if (!url) return;
      const sampled = await sampleCover(url);
      if (owner === revision) palette.value = sampled;
    },
    { immediate: true },
  );
  onUnmounted(() => revision++);
  return {
    atmosphere: computed(() => coverAtmosphereTokens(palette.value, theme())),
    palette_ready: computed(() => palette.value !== null),
  };
}
