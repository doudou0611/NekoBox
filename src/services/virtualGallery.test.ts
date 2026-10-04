import { describe, expect, it } from 'vitest';
import { galleryWindow } from './virtualGallery';
describe('visible gallery window', () => {
  it('bounds rendered cards and preserves total row extent for 1000 records', () => {
    const count = 1000,
      columns = 4,
      row = 480;
    for (const offset of [0, 1500, 80000, 119000]) {
      const window = galleryWindow(count, columns, row, offset, 1000);
      expect(window.end - window.start).toBeLessThanOrEqual(32);
      expect(
        window.top +
          Math.ceil((window.end - window.start) / columns) * row +
          window.bottom,
      ).toBe(Math.ceil(count / columns) * row);
      expect(window.start % columns).toBe(0);
      expect(window.end).toBeLessThanOrEqual(count);
    }
  });
  it('handles empty, resized columns, an incomplete last row and a viewport beyond the end', () => {
    expect(galleryWindow(0, 0, 0, 0, 0)).toEqual({
      start: 0,
      end: 0,
      top: 0,
      bottom: 0,
    });
    expect(galleryWindow(7, 3, 100, 999999, 100).end).toBe(7);
    expect(galleryWindow(7, 1, 100, -800, 600).start).toBe(0);
  });
});
