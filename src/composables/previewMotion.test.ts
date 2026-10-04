import { describe, expect, it } from 'vitest';
import {
  motionDuration,
  resolveMotion,
  shouldAnimateDecorations,
} from './motionPolicy';
import { TransitionMachine, sharedTransform } from './transitionMachine';
import { createPreviewGames, PREVIEW_GAMES } from '../preview/data';

describe('preview motion policy', () => {
  it('respects system reduction before explicit overrides', () => {
    expect(resolveMotion('system', true)).toBe('reduced');
    expect(resolveMotion('system', false)).toBe('full');
  });
  it('honors every explicit mode', () => {
    for (const mode of ['full', 'light', 'reduced'] as const)
      expect(resolveMotion(mode, true)).toBe(mode);
  });
  it('uses specified full timings and a non-spatial reduced mode', () => {
    expect(motionDuration('full', 'open')).toBe(620);
    expect(motionDuration('full', 'close')).toBe(420);
    expect(motionDuration('light', 'open')).toBeLessThan(500);
    expect(motionDuration('reduced', 'open')).toBe(0);
    expect(motionDuration('reduced', 'close')).toBe(0);
  });
  it('pauses only decoration when hidden and disables it in downgraded modes', () => {
    expect(shouldAnimateDecorations('full', false)).toBe(true);
    expect(shouldAnimateDecorations('full', true)).toBe(false);
    expect(shouldAnimateDecorations('light', false)).toBe(false);
    expect(shouldAnimateDecorations('reduced', false)).toBe(false);
  });
});
describe('shared transition ownership', () => {
  it('performs a normal open / return and removes overlay ownership', () => {
    const machine = new TransitionMachine();
    const token = machine.beginOpen('demo-shore')!;
    expect(machine.phase).toBe('opening');
    expect(machine.overlay_active).toBe(true);
    machine.finish(token);
    expect(machine.phase).toBe('detail');
    machine.finish(machine.beginClose()!);
    expect(machine.phase).toBe('idle');
    expect(machine.overlay_active).toBe(false);
    expect(machine.game_id).toBeNull();
  });
  it('is idempotent for repeat clicks on the same work', () => {
    const machine = new TransitionMachine();
    const token = machine.beginOpen('demo-shore');
    expect(machine.beginOpen('demo-shore')).toBeNull();
    expect(machine.token).toBe(token);
  });
  it('ignores an old completion after a newer work was selected', () => {
    const machine = new TransitionMachine();
    const old = machine.beginOpen('demo-shore')!;
    const latest = machine.beginOpen('demo-forest')!;
    expect(machine.finish(old)).toBe(false);
    expect(machine.game_id).toBe('demo-forest');
    expect(machine.overlay_active).toBe(true);
    machine.finish(latest);
    expect(machine.phase).toBe('detail');
  });
  it('allows Back / Escape while opening without a stale finish', () => {
    const machine = new TransitionMachine();
    const open = machine.beginOpen('demo-shore')!;
    const close = machine.beginClose()!;
    expect(machine.phase).toBe('closing');
    expect(machine.finish(open)).toBe(false);
    machine.finish(close);
    expect(machine.overlay_active).toBe(false);
    expect(machine.phase).toBe('idle');
  });
  it('ignores duplicate close and accepts reopen during closing', () => {
    const machine = new TransitionMachine();
    machine.beginOpen('demo-shore');
    const close = machine.beginClose()!;
    expect(machine.beginClose()).toBeNull();
    const reopen = machine.beginOpen('demo-shore')!;
    expect(machine.finish(close)).toBe(false);
    expect(machine.finish(reopen)).toBe(true);
    expect(machine.phase).toBe('detail');
  });
  it('settles resize in detail and cancels old animation ownership', () => {
    const machine = new TransitionMachine();
    const token = machine.beginOpen('demo-shore')!;
    machine.settle(true);
    expect(machine.finish(token)).toBe(false);
    expect(machine.overlay_active).toBe(false);
    expect(machine.phase).toBe('detail');
  });
  it('settles cancellation / source-missing return with no overlay ownership', () => {
    const machine = new TransitionMachine();
    machine.beginOpen('demo-shore');
    const close = machine.beginClose()!;
    machine.settle(false);
    expect(machine.finish(close)).toBe(false);
    expect(machine.overlay_active).toBe(false);
    expect(machine.phase).toBe('idle');
  });
  it('does not close an idle route', () =>
    expect(new TransitionMachine().beginClose()).toBeNull());
});
describe('shared geometry and fixtures', () => {
  it('has a single scale value and never stretches independently in x/y', () => {
    expect(
      sharedTransform(
        { left: 10, top: 20, width: 150, height: 200 },
        { left: 30, top: 50, width: 300, height: 400 },
      ),
    ).toEqual({ x: 20, y: 30, scale: 2 });
  });
  it('rejects an absent / collapsed source or target', () => {
    expect(
      sharedTransform(
        { left: 0, top: 0, width: 0, height: 0 },
        { left: 0, top: 0, width: 200, height: 300 },
      ),
    ).toBeNull();
  });
  it('isolates mutable preview records from fixtures and production ids', () => {
    const games = createPreviewGames();
    expect(games.every((game) => game.game_id.startsWith('demo-'))).toBe(true);
    games[0]!.favorite = !games[0]!.favorite;
    games[0]!.tags.push('test');
    expect(games[0]!.favorite).not.toBe(PREVIEW_GAMES[0]!.favorite);
    expect(PREVIEW_GAMES[0]!.tags).not.toContain('test');
  });
});
