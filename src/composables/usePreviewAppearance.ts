import { watchEffect } from 'vue';
import { preview } from '../preview/store';
import { PALETTES, paletteTokens } from '../preview/palettes';

export function usePreviewAppearance() {
  try {
    const saved = JSON.parse(
      localStorage.getItem('galgame.appearance') || 'null',
    );
    if (saved && PALETTES.some((p) => p.id === saved.palette))
      preview.palette = saved.palette;
    if (saved && ['dark', 'light'].includes(saved.theme))
      preview.theme = saved.theme;
  } catch {
    /* An unavailable UI preferences store does not block the app. */
  }

  watchEffect(() => {
    const root = document.documentElement;
    try {
      localStorage.setItem(
        'galgame.appearance',
        JSON.stringify({ theme: preview.theme, palette: preview.palette }),
      );
    } catch {
      /* Keep applying the current appearance. */
    }
    root.dataset.theme = preview.theme;
    root.dataset.palette = preview.palette;
    const palette =
      PALETTES.find((item) => item.id === preview.palette) ?? PALETTES[0]!;
    for (const [name, value] of Object.entries(
      paletteTokens(palette, preview.theme),
    ))
      root.style.setProperty(name, value);
  });
}
