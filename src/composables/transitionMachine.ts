export type TransitionPhase = 'idle' | 'opening' | 'detail' | 'closing';
/** DOM-independent coordinator. Stale completion handlers can never own cleanup. */
export class TransitionMachine {
  phase: TransitionPhase = 'idle';
  game_id: string | null = null;
  token = 0;
  overlay_active = false;
  beginOpen(game_id: string): number | null {
    if (
      this.game_id === game_id &&
      (this.phase === 'opening' || this.phase === 'detail')
    )
      return null;
    this.token += 1;
    this.game_id = game_id;
    this.phase = 'opening';
    this.overlay_active = true;
    return this.token;
  }
  beginClose(): number | null {
    if (this.phase === 'idle' || this.phase === 'closing') return null;
    this.token += 1;
    this.phase = 'closing';
    this.overlay_active = true;
    return this.token;
  }
  owns(token: number): boolean {
    return token === this.token;
  }
  finish(token: number): boolean {
    if (!this.owns(token)) return false;
    this.phase = this.phase === 'closing' ? 'idle' : 'detail';
    this.overlay_active = false;
    if (this.phase === 'idle') this.game_id = null;
    return true;
  }
  settle(in_detail: boolean) {
    this.token += 1;
    this.phase = in_detail ? 'detail' : 'idle';
    this.overlay_active = false;
    if (!in_detail) this.game_id = null;
  }
}
export interface TransitionRect {
  left: number;
  top: number;
  width: number;
  height: number;
}
export function sharedTransform(
  source: TransitionRect,
  target: TransitionRect,
): { x: number; y: number; scale: number } | null {
  if (
    source.width <= 0 ||
    source.height <= 0 ||
    target.width <= 0 ||
    target.height <= 0
  )
    return null;
  return {
    x: target.left - source.left,
    y: target.top - source.top,
    scale: target.width / source.width,
  };
}
