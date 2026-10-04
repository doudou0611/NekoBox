import { beforeEach, expect, it, vi } from 'vitest';
import { createPreviewGames } from '../preview/data';
import type { PreviewGroup } from '../preview/groups';
const state = vi.hoisted(() => ({
  preview: {
    games: [] as ReturnType<typeof createPreviewGames>,
    groups: [] as PreviewGroup[],
  },
  appSettings: {
    value: { sidebar_game_order: {} as Record<string, string[]> },
  },
  load: vi.fn(),
  save: vi.fn(),
}));
vi.mock('../stores/library', () => ({ preview: state.preview }));
vi.mock('../stores/settings', () => ({
  appSettings: state.appSettings,
  loadAppSettings: state.load,
  saveAppSettings: state.save,
}));
import {
  moveSidebarGame,
  sidebarMembers,
  COMPACT_GAME_ORDER,
} from './sidebarGameOrder';
beforeEach(() => {
  vi.resetAllMocks();
  state.preview.games = createPreviewGames();
  state.preview.groups = [
    {
      group_id: 'a',
      name: 'A',
      game_ids: ['demo-shore', 'demo-forest', 'demo-sky'],
    },
    { group_id: 'b', name: 'B', game_ids: ['demo-shore', 'demo-forest'] },
  ];
  state.appSettings.value.sidebar_game_order = {};
  state.save.mockImplementation(async (value) => {
    state.appSettings.value = value;
  });
});
it('sorts before/after per group without changing membership or the global library', async () => {
  const games = state.preview.games.map((game) => game.game_id);
  const groups = JSON.stringify(state.preview.groups);
  await moveSidebarGame('a', 'demo-sky', 'demo-shore', 'before');
  expect(sidebarMembers('a').map((game) => game.game_id)).toEqual([
    'demo-sky',
    'demo-shore',
    'demo-forest',
  ]);
  expect(sidebarMembers('b').map((game) => game.game_id)).toEqual([
    'demo-shore',
    'demo-forest',
  ]);
  await moveSidebarGame('a', 'demo-sky', 'demo-forest', 'after');
  expect(sidebarMembers('a').map((game) => game.game_id)).toEqual([
    'demo-shore',
    'demo-forest',
    'demo-sky',
  ]);
  expect(JSON.stringify(state.preview.groups)).toBe(groups);
  expect(state.preview.games.map((game) => game.game_id)).toEqual(games);
  expect(
    state.save.mock.calls.every(
      (call) => JSON.stringify(call[1]) === '["sidebar_game_order"]',
    ),
  ).toBe(true);
});
it('retains preceding saves, ignores stale IDs and appends newly added members', async () => {
  await Promise.all([
    moveSidebarGame('a', 'demo-sky', 'demo-shore', 'before'),
    moveSidebarGame('b', 'demo-forest', 'demo-shore', 'before'),
  ]);
  state.preview.games = state.preview.games.filter(
    (game) => game.game_id !== 'demo-sky',
  );
  state.preview.groups[0].game_ids.push('demo-snow');
  expect(sidebarMembers('a').map((game) => game.game_id)).toEqual([
    'demo-shore',
    'demo-forest',
    'demo-snow',
  ]);
  expect(sidebarMembers('b').map((game) => game.game_id)).toEqual([
    'demo-forest',
    'demo-shore',
  ]);
});
it('supports favorite/ungrouped/compact order and revalidates live membership', async () => {
  await moveSidebarGame('favorites', 'demo-forest', 'demo-shore', 'before');
  expect(sidebarMembers('favorites')[0].game_id).toBe('demo-forest');
  await moveSidebarGame(COMPACT_GAME_ORDER, 'demo-sky', 'demo-shore', 'before');
  expect(sidebarMembers(COMPACT_GAME_ORDER)[0].game_id).toBe('demo-sky');
  const ungrouped = sidebarMembers('ungrouped');
  await moveSidebarGame(
    'ungrouped',
    ungrouped[1].game_id,
    ungrouped[0].game_id,
    'before',
  );
  expect(sidebarMembers('ungrouped')[0].game_id).toBe(ungrouped[1].game_id);
  await expect(
    moveSidebarGame('a', 'missing', 'demo-shore', 'before'),
  ).rejects.toThrow('作品或分组已变化');
});
it('failed persistence does not claim success or alter displayed order', async () => {
  state.save.mockRejectedValueOnce(new Error('save failed'));
  await expect(
    moveSidebarGame('a', 'demo-sky', 'demo-shore', 'before'),
  ).rejects.toThrow('save failed');
  expect(state.appSettings.value.sidebar_game_order).toEqual({});
  await moveSidebarGame('b', 'demo-forest', 'demo-shore', 'before');
  expect(sidebarMembers('b')[0].game_id).toBe('demo-forest');
});
