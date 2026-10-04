import { describe, expect, it } from 'vitest';
import { createMemoryHistory } from 'vue-router';
import protocol from '../../shared/protocol.json';
import fixture from '../../shared/game-fixture.json';
import { ERROR_CODES, IMPLEMENTED_COMMANDS, type CommandName } from './api';
import type { GameDetail } from './domain';
import {
  GAME_STATUSES,
  INSTALL_SOURCES,
  METADATA_STATUSES,
  TASK_STATUSES,
} from './domain';
import { EVENTS } from './events';
import { createAppRouter, NAVIGATION } from '../router';
const game: GameDetail = {
  ...fixture,
  status: 'not_started',
  metadata_status: 'local_only',
  user_rating: null,
  installations: fixture.installations.map((installation) => ({
    ...installation,
    source: 'local',
  })),
};
const commandManifest: Record<CommandName, string> = protocol.commands;
describe('public contract parity', () => {
  it('matches central enum and event names', () => {
    expect(ERROR_CODES).toEqual(protocol.error_codes);
    expect(GAME_STATUSES).toEqual(protocol.game_statuses);
    expect(INSTALL_SOURCES).toEqual(protocol.install_sources);
    expect(METADATA_STATUSES).toEqual(protocol.metadata_statuses);
    expect(TASK_STATUSES).toEqual(protocol.task_statuses);
    expect(EVENTS).toEqual(protocol.events);
  });
  it('does not pretend planned commands are implemented', () => {
    expect(
      Object.entries(commandManifest)
        .filter(([, state]) => state === 'implemented')
        .map(([name]) => name),
    ).toEqual([...IMPLEMENTED_COMMANDS]);
  });
  it('keeps a shared game fixture without real user data', () => {
    expect(game).toEqual(fixture);
    expect(game.installations[0]?.game_id).toBe(game.id);
    expect(game.metadata[0]?.provider).toBe('manual');
    expect(game.installations[0]?.path_valid).toBe(false);
  });
  it('has exactly five primary destinations and no separate collection route', () => {
    expect(NAVIGATION.map((route) => route.label)).toEqual([
      '首页',
      '游戏',
      '活动',
      '存档',
      '设置',
    ]);
  });
  it('resolves game detail and defaults invalid locations to home', async () => {
    const router = createAppRouter(createMemoryHistory());
    await router.push('/games/demo-game');
    expect(router.currentRoute.value.name).toBe('game-detail');
    expect(router.currentRoute.value.params.game_id).toBe('demo-game');
    await router.push('/not-a-route');
    expect(router.currentRoute.value.name).toBe('home');
  });
});
