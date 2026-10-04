import type { PreviewGame } from '../preview/data';
import type { GameQuery } from '../types/domain';

/** The preview uses the same saved query rules as the desktop database. */
export function matchesGameQuery(game: PreviewGame, query: GameQuery): boolean {
  const search = query.search.trim().toLocaleLowerCase();
  const f = query.filters ?? {};
  const seconds = game.playtime_seconds ?? game.duration_minutes * 60;
  const tagIds = game.tag_ids ?? game.tags;
  return (
    (!search ||
      `${game.title} ${game.subtitle} ${game.tags.join(' ')}`
        .toLocaleLowerCase()
        .includes(search)) &&
    (!query.statuses.length || query.statuses.includes(game.status)) &&
    (!query.sources.length ||
      query.sources.some((source) => game.sources.includes(source))) &&
    query.tag_ids.every((id) => tagIds.includes(id)) &&
    (query.favorite == null || game.favorite === query.favorite) &&
    (f.developer == null ||
      game.developer.toLocaleLowerCase() === f.developer.toLocaleLowerCase()) &&
    (f.release_year == null || game.year === String(f.release_year)) &&
    (f.min_playtime_seconds == null || seconds >= f.min_playtime_seconds) &&
    (f.max_playtime_seconds == null || seconds <= f.max_playtime_seconds) &&
    (f.played_after == null ||
      game.last_played_order >= Date.parse(f.played_after)) &&
    (f.metadata_pending == null ||
      Boolean(game.metadata_pending) === f.metadata_pending) &&
    (f.has_save_backup == null ||
      Boolean(game.has_save_backup) === f.has_save_backup) &&
    (f.multiple_installations == null ||
      (game.install_count ?? 1) > 1 === f.multiple_installations) &&
    (f.metadata_incomplete == null ||
      Boolean(game.metadata_incomplete) === f.metadata_incomplete)
  );
}
export function sortGames(
  games: PreviewGame[],
  query: GameQuery,
): PreviewGame[] {
  const direction = query.direction === 'asc' ? 1 : -1;
  return [...games].sort(
    (a, b) =>
      direction *
        (query.sort === 'title'
          ? a.title.localeCompare(b.title, 'zh-CN')
          : query.sort === 'playtime'
            ? (a.playtime_seconds ?? a.duration_minutes * 60) -
              (b.playtime_seconds ?? b.duration_minutes * 60)
            : query.sort === 'last_played_at'
              ? a.last_played_order - b.last_played_order
              : a.added_order - b.added_order) ||
      a.game_id.localeCompare(b.game_id),
  );
}
