import { describe, expect, it } from 'vitest';
import { createPreviewGames } from '../preview/data';
import type { GameQuery } from '../types/domain';
import { matchesGameQuery, sortGames } from './gameQuery';
const base = (): GameQuery => ({
  page: 1,
  page_size: 100,
  search: '',
  statuses: [],
  sources: [],
  tag_ids: [],
  favorite: null,
  collection_id: null,
  sort: 'added_at',
  direction: 'desc',
});
describe('saved game rules', () => {
  it('combines search, source, tag, year, developer and time without losing conditions', () => {
    const games = createPreviewGames();
    const q = {
      ...base(),
      search: '潮汐',
      sources: ['local'] as const,
      tag_ids: ['海边'],
      favorite: true,
      filters: {
        release_year: 2024,
        developer: games[0].developer,
        min_playtime_seconds: 1,
      },
    };
    const query = { ...q, sources: [...q.sources] };
    expect(
      games.filter((g) => matchesGameQuery(g, query)).map((g) => g.game_id),
    ).toEqual(['demo-shore']);
    query.filters.release_year = 2023;
    expect(games.filter((g) => matchesGameQuery(g, query))).toEqual([]);
  });
  it('tracks changing state and uses tag IDs and derived record flags', () => {
    const game = {
      ...createPreviewGames()[0],
      tag_ids: ['tag-id'],
      metadata_pending: true,
      has_save_backup: true,
      install_count: 2,
    };
    const q = {
      ...base(),
      tag_ids: ['tag-id'],
      filters: {
        metadata_pending: true,
        has_save_backup: true,
        multiple_installations: true,
      },
    };
    expect(matchesGameQuery(game, q)).toBe(true);
    game.has_save_backup = false;
    expect(matchesGameQuery(game, q)).toBe(false);
  });
  it('filters and orders 1000 isolated records without mutating source order', () => {
    const games = Array.from({ length: 1000 }, (_, i) => ({
      ...createPreviewGames()[i % 6],
      game_id: `fixture-${i}`,
      added_order: i,
      year: String(2020 + (i % 5)),
    }));
    const q = { ...base(), filters: { release_year: 2024 } };
    const matched = sortGames(
      games.filter((g) => matchesGameQuery(g, q)),
      q,
    );
    expect(matched).toHaveLength(200);
    expect(matched[0].game_id).toBe('fixture-999');
    expect(games[0].game_id).toBe('fixture-0');
  });
});
