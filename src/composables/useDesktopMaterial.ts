import { isTauri } from '@tauri-apps/api/core';
import { onUnmounted, ref, watch, watchEffect } from 'vue';
import { preview } from '../preview/store';
import { syncDesktopMaterial } from '../services/windowMaterial';

// Native preferences are loaded and persisted by App; localStorage supplies the first frame.
function savedGlass() {
  try {
    return localStorage.getItem('galgame.glass') !== 'off';
  } catch {
    return true;
  }
}
export const global_glass_enabled = ref(savedGlass());

export function useDesktopMaterial() {
  watchEffect(() => {
    // Sidebar and main content deliberately share one material preference.
    // Keep the legacy data attribute as a mirror for existing host CSS/tests.
    const enabled = global_glass_enabled.value ? 'on' : 'off';
    try {
      localStorage.setItem('galgame.glass', enabled);
    } catch {
      /* Current state remains usable. */
    }
    document.documentElement.dataset.desktopGlass = enabled;
    document.documentElement.dataset.globalGlass = enabled;
  });
  if (!isTauri()) {
    document.documentElement.dataset.windowMaterial = 'browser';
    return;
  }
  // The native startup marker arrives after Vue mounts. Observe it without
  // treating a temporarily missing marker as a failed native effect.
  const material = ref(document.documentElement.dataset.windowMaterial ?? '');
  const observer = new MutationObserver(() => {
    material.value = document.documentElement.dataset.windowMaterial ?? '';
  });
  observer.observe(document.documentElement, {
    attributes: true,
    attributeFilter: ['data-window-material'],
  });
  let token = 0;
  onUnmounted(() => {
    token++;
    observer.disconnect();
  });
  watch(
    [() => preview.theme, () => global_glass_enabled.value, material],
    async ([theme, enabled, host_material]) => {
      const owner = ++token;
      if (!host_material) return;
      const synced = await syncDesktopMaterial(theme, enabled, host_material);
      if (!synced && owner === token) {
        global_glass_enabled.value = false;
      }
    },
    { immediate: true },
  );
}
