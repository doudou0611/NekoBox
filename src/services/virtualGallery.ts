export function galleryWindow(
  count: number,
  columns: number,
  rowHeight: number,
  offset: number,
  viewport: number,
  overscan = 2,
) {
  const cols = Math.max(1, Math.floor(columns));
  const height = Math.max(1, rowHeight);
  const rows = Math.ceil(Math.max(0, count) / cols);
  if (!rows) return { start: 0, end: 0, top: 0, bottom: 0 };
  const first = Math.min(
    rows - 1,
    Math.max(0, Math.floor(offset / height) - overscan),
  );
  const last = Math.min(
    rows,
    Math.max(
      first + 1,
      Math.ceil((Math.max(0, offset) + Math.max(1, viewport)) / height) +
        overscan,
    ),
  );
  return {
    start: first * cols,
    end: Math.min(count, last * cols),
    top: first * height,
    bottom: (rows - last) * height,
  };
}

/** Reuse measured geometry on detail return so the initial extent does not clamp restored scroll. */
export const galleryLayouts = new Map<
  string,
  { columns: number; rowHeight: number; gap: number }
>();
