import { isTauri } from '@tauri-apps/api/core';
import { Effect, EffectState, getCurrentWindow } from '@tauri-apps/api/window';

let material_queue: Promise<boolean> = Promise.resolve(true);

// Native operations must settle in order: a late enable cannot undo a newer disable.
export function syncDesktopMaterial(
  theme: 'dark' | 'light',
  enabled: boolean,
  material: string,
): Promise<boolean> {
  material_queue = material_queue.then(async () => {
    if (!(await syncDesktopTheme(theme))) return false;
    try {
      const window = getCurrentWindow();
      if (
        !enabled ||
        !['windows-acrylic', 'macos-vibrancy'].includes(material)
      ) {
        // An empty effects array does NOT clear existing effects on Windows.
        await window.clearEffects();
      } else {
        await window.setEffects(
          material === 'windows-acrylic'
            ? { effects: [Effect.Acrylic], color: [0, 0, 0, 1] }
            : { effects: [Effect.Sidebar], state: EffectState.Active },
        );
      }
      return true;
    } catch {
      return false;
    }
  });
  return material_queue;
}

// Only a window appearance permission; no filesystem, shell or production IPC.
export async function syncDesktopTheme(
  theme: 'dark' | 'light',
): Promise<boolean> {
  if (!isTauri()) return false;
  try {
    await getCurrentWindow().setTheme(theme);
    return true;
  } catch {
    // Do not expose raw native errors or keep low-contrast mismatched materials.
    return false;
  }
}
