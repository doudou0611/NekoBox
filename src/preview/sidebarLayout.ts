export const DEFAULT_SIDEBAR_WIDTH = 292;
// 保持导航区紧凑，把更多首屏空间留给游戏分组。
export const DEFAULT_TOP_RATIO = 0.34;
export const SPLITTER_SIZE = 12;
export function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}
export function sidebarWidthBounds(viewport_width: number) {
  const min = Math.min(240, Math.max(84, viewport_width - 320));
  return { min, max: Math.max(min, Math.min(460, viewport_width - 480)) };
}
export function sidebarPartition(panel_height: number, ratio: number) {
  const available = Math.max(1, panel_height - SPLITTER_SIZE);
  const min = Math.min(136, available * 0.35);
  const max = available - Math.min(144, available * 0.35);
  return { available, min, max, top: clamp(available * ratio, min, max) };
}
