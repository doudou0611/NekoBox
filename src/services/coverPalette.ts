export type CoverColor = readonly [number, number, number];
export interface CoverPalette {
  primary: CoverColor;
  secondary: CoverColor;
}

const mix = (
  color: CoverColor,
  target: CoverColor,
  amount: number,
): CoverColor =>
  color.map((value, index) =>
    Math.round(value * (1 - amount) + target[index]! * amount),
  ) as unknown as CoverColor;
const rgb = (color: CoverColor, alpha = 1) =>
  `rgb(${color.join(' ')} / ${alpha})`;

export function luminance(color: CoverColor): number {
  const channels = color.map((value) => {
    const normalized = value / 255;
    return normalized <= 0.04045
      ? normalized / 12.92
      : ((normalized + 0.055) / 1.055) ** 2.4;
  });
  return channels[0]! * 0.2126 + channels[1]! * 0.7152 + channels[2]! * 0.0722;
}

export function contrast(a: CoverColor, b: CoverColor): number {
  const first = luminance(a);
  const second = luminance(b);
  return (Math.max(first, second) + 0.05) / (Math.min(first, second) + 0.05);
}

/** Quantized color clusters keep white margins from overwhelming cover artwork. */
export function extractCoverPalette(
  pixels: ArrayLike<number>,
): CoverPalette | null {
  const clusters = new Map<
    string,
    { color: number[]; count: number; weight: number }
  >();
  for (let index = 0; index + 3 < pixels.length; index += 4) {
    if (pixels[index + 3]! < 128) continue;
    const color: CoverColor = [
      pixels[index]!,
      pixels[index + 1]!,
      pixels[index + 2]!,
    ];
    const max = Math.max(...color);
    const min = Math.min(...color);
    if ((min > 220 && max - min < 35) || max < 24) continue;
    const saturation = (max - min) / Math.max(max, 1);
    const key = color.map((value) => Math.floor(value / 32)).join(',');
    const cluster = clusters.get(key) ?? {
      color: [0, 0, 0],
      count: 0,
      weight: 0,
    };
    color.forEach((value, channel) => (cluster.color[channel]! += value));
    cluster.count++;
    cluster.weight += 0.12 + saturation;
    clusters.set(key, cluster);
  }
  const ranked = [...clusters.values()]
    .sort((a, b) => b.weight - a.weight)
    .map((cluster) => ({
      color: cluster.color.map((value) =>
        Math.round(value / cluster.count),
      ) as unknown as CoverColor,
      weight: cluster.weight,
    }));
  const primary = ranked[0]?.color;
  if (!primary) return null;
  const distance = (color: CoverColor) =>
    color.reduce(
      (sum, value, index) => sum + (value - primary[index]!) ** 2,
      0,
    );
  const secondary = ranked
    .filter((cluster) => distance(cluster.color) > 75 ** 2)
    .sort(
      (a, b) =>
        b.weight * Math.sqrt(distance(b.color)) -
        a.weight * Math.sqrt(distance(a.color)),
    )[0]?.color;
  return { primary, secondary: secondary ?? primary };
}

export function coverAtmosphereTokens(
  palette: CoverPalette | null,
  theme: 'light' | 'dark',
): Record<string, string> {
  // No stale artwork color when a cover is missing or unreadable.
  if (!palette) return {};
  const light = theme === 'light';
  const base = mix(
    palette.primary,
    light ? [255, 253, 249] : [12, 14, 18],
    light ? 0.86 : 0.8,
  );
  const soft = mix(
    palette.secondary,
    light ? [255, 253, 249] : [12, 14, 18],
    light ? 0.84 : 0.78,
  );
  const ink: CoverColor = light ? [35, 31, 37] : [255, 250, 245];
  const muted = mix(ink, base, 0.22);
  const button = mix(
    palette.primary,
    light ? [18, 20, 24] : [255, 253, 249],
    light ? 0.65 : 0.8,
  );
  const buttonInk: CoverColor =
    contrast(button, [255, 255, 255]) >= 4.5 ? [255, 255, 255] : [18, 20, 24];
  return {
    '--cover-accent': rgb(palette.primary),
    '--cover-secondary': rgb(palette.secondary),
    '--cover-mist': rgb(palette.primary, light ? 0.2 : 0.14),
    '--hero-base': rgb(base),
    '--hero-ink': rgb(ink),
    '--hero-muted': rgb(muted),
    '--hero-shade': rgb(base, 0.96),
    '--hero-shade-soft': rgb(soft, light ? 0.7 : 0.75),
    '--hero-button-bg': rgb(button),
    '--hero-button-text': rgb(buttonInk),
  };
}
