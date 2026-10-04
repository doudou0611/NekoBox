import { describe, expect, it } from 'vitest';
import {
  contrast,
  coverAtmosphereTokens,
  extractCoverPalette,
  type CoverColor,
} from './coverPalette';

const pixels = (color: CoverColor, count: number, alpha = 255) =>
  Array.from({ length: count }, () => [...color, alpha]).flat();
const readColor = (value: string): CoverColor =>
  value.match(/\d+/g)!.slice(0, 3).map(Number) as unknown as CoverColor;

describe('cover-driven atmosphere', () => {
  it('extracts artwork colors rather than a large white border', () => {
    const palette = extractCoverPalette([
      ...pixels([247, 237, 231], 80),
      ...pixels([220, 90, 140], 15),
      ...pixels([140, 190, 75], 5),
    ]);
    expect(palette).toEqual({
      primary: [220, 90, 140],
      secondary: [140, 190, 75],
    });
  });
  it('ignores transparent pixels and uses a neutral fallback for blank artwork', () => {
    expect(extractCoverPalette(pixels([255, 0, 0], 20, 0))).toBeNull();
    expect(extractCoverPalette(pixels([255, 255, 255], 20))).toBeNull();
    expect(coverAtmosphereTokens(null, 'light')).toEqual({});
  });
  it('keeps monochrome covers monochrome', () => {
    expect(extractCoverPalette(pixels([145, 145, 145], 20))).toEqual({
      primary: [145, 145, 145],
      secondary: [145, 145, 145],
    });
  });
  it('changes the stage palette when artwork changes', () => {
    const warm = extractCoverPalette(pixels([220, 90, 140], 20));
    const cool = extractCoverPalette(pixels([25, 130, 200], 20));
    expect(coverAtmosphereTokens(warm, 'light')['--hero-base']).not.toBe(
      coverAtmosphereTokens(cool, 'light')['--hero-base'],
    );
  });
  for (const theme of ['light', 'dark'] as const) {
    it(`preserves readable text and buttons for saturated ${theme} artwork`, () => {
      for (const primary of [
        [255, 255, 0],
        [255, 20, 50],
        [0, 30, 240],
        [40, 230, 100],
        [100, 100, 100],
      ] as CoverColor[]) {
        const tokens = coverAtmosphereTokens(
          { primary, secondary: primary },
          theme,
        );
        const base = readColor(tokens['--hero-base']!);
        expect(
          contrast(base, readColor(tokens['--hero-ink']!)),
        ).toBeGreaterThanOrEqual(4.5);
        expect(
          contrast(base, readColor(tokens['--hero-muted']!)),
        ).toBeGreaterThanOrEqual(4.5);
        expect(
          contrast(
            readColor(tokens['--hero-button-bg']!),
            readColor(tokens['--hero-button-text']!),
          ),
        ).toBeGreaterThanOrEqual(4.5);
      }
    });
  }
});
