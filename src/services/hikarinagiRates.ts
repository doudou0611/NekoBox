import type { HikarinagiRatesWall } from '../types/hikarinagi';

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
