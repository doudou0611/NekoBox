export type MotionMode = 'full' | 'light' | 'reduced';
export type MotionPreference = 'system' | MotionMode;
export function resolveMotion(
  preference: MotionPreference,
  system_reduced: boolean,
): MotionMode {
  return preference === 'system'
    ? system_reduced
      ? 'reduced'
      : 'full'
    : preference;
}
export function motionDuration(
  mode: MotionMode,
  direction: 'open' | 'close',
): number {
  if (mode === 'reduced') return 0;
  if (mode === 'light') return direction === 'open' ? 280 : 220;
  return direction === 'open' ? 620 : 420;
}
export function shouldAnimateDecorations(
  mode: MotionMode,
  document_hidden: boolean,
): boolean {
  return mode === 'full' && !document_hidden;
}
