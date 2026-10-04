import type { HikarinagiRatesWall } from '../types/hikarinagi';
import type { SourcedField } from '../types/domain';

export interface MetadataRatesFallback {
  provider: 'bangumi' | 'vndb';
  label: string;
  average: number;
  tags: string[];
  fetched_at: string | null;
}

/** Metadata is already ordered by the work's saved provider priority. No network requests. */
export function metadataRatesFallback(
  fields: readonly SourcedField[],
): MetadataRatesFallback | null {
  const text = (field: SourcedField) => {
    try {
      const value: unknown = JSON.parse(field.value);
      return typeof value === 'string' ? value.trim() : '';
    } catch {
      return '';
    }
  };
  for (const field of fields) {
    if (
      field.field !== 'source_rating' ||
      field.manually_edited ||
      (field.provider !== 'bangumi' && field.provider !== 'vndb')
    )
      continue;
    const value = text(field);
    const average = Number(value);
    // Both providers use zero for an unrated work; it is not a community score.
    if (!value || !Number.isFinite(average) || average <= 0 || average > 10)
      continue;
    const tags = fields.find(
      (row) =>
        row.provider === field.provider &&
        row.remote_id === field.remote_id &&
        row.field === 'source_tags' &&
        !row.manually_edited,
    );
    const words = tags
      ? text(tags)
          .split('\n')
          .map((word) => word.trim())
      : [];
    const seen = new Set<string>();
    return {
      provider: field.provider,
      label: field.provider === 'bangumi' ? 'Bangumi' : 'VNDB',
      average,
      tags: words
        .filter((word) => {
          if (!word || seen.has(word.toLowerCase())) return false;
          seen.add(word.toLowerCase());
          return true;
        })
        .slice(0, 6),
      fetched_at: field.fetched_at,
    };
  }
  return null;
}

/** A binding alone (or play-status counters) does not guarantee a usable rates wall. */
export function hasHikarinagiRates(wall: HikarinagiRatesWall | null): boolean {
  return (
    !!wall &&
    ((wall.rated_count > 0 &&
      wall.average != null &&
      Number.isFinite(wall.average) &&
      wall.average > 0 &&
      wall.average <= 10) ||
      wall.keywords.some((keyword) => keyword.word.trim() && keyword.count > 0))
  );
}

export interface RatesWallState {
  game_id: string;
  wall: HikarinagiRatesWall | null;
  loading: boolean;
  error: string;
}

/** Ignore results for old works, superseded requests and unmounted cards. */
export function createRatesLoader(
  state: RatesWallState,
  load: (
    game_id: string,
    refresh: boolean,
  ) => Promise<{ wall: HikarinagiRatesWall | null }>,
  message: (error: unknown) => string,
) {
  let version = 0;
  return {
    async refresh(game_id: string, refresh = false) {
      const token = ++version;
      // Always clear the old wall, including rebinds of the same local work.
      state.game_id = game_id;
      state.wall = null;
      state.error = '';
      state.loading = true;
      try {
        const response = await load(game_id, refresh);
        if (token === version) state.wall = response.wall;
      } catch (error) {
        if (token === version) state.error = message(error);
      } finally {
        if (token === version) state.loading = false;
      }
    },
    dispose() {
      version++;
    },
  };
}
