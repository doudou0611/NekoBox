import { reactive } from 'vue';
/** Isolated fictional dashboard fixtures. Never used as fallback for desktop failure. */
import type { PreviewGame } from './data';
import type { HomeDashboard, HomeWindow } from '../types/home';
import { statusCounts, localDate } from '../services/homeDashboard';
export function demoHomeDashboard(
  games: PreviewGame[],
  window: HomeWindow,
): HomeDashboard {
  const week_daily = window.week_days.map((d, i) => ({
    date: d.date,
    duration_seconds:
      games.length && d.start_at <= window.today.start_at
        ? [1920, 3600, 0, 5400, 2100, 8040, 0][i]!
        : 0,
  }));
  const week_seconds = week_daily.reduce((n, d) => n + d.duration_seconds, 0),
    today_seconds =
      week_daily.find(
        (d) => d.date === localDate(new Date(window.today.start_at)),
      )?.duration_seconds ?? 0;
  const recents = [...games]
    .filter((g) => g.last_played_order > 0)
    .sort((a, b) => b.last_played_order - a.last_played_order);
  return {
    status_counts: statusCounts(games),
    active_game_ids: [],
    recently_played_game_ids: recents.map((g) => g.game_id).slice(0, 6),
    today_seconds,
    week_seconds,
    week_daily,
    week_games: recents.slice(0, 2).map((g, i) => ({
      game_id: g.game_id,
      duration_seconds:
        i === 0
          ? Math.floor(week_seconds * 0.72)
          : week_seconds - Math.floor(week_seconds * 0.72),
    })),
    other_seconds: 0,
    memory_games:
      window.memory_day && games.length
        ? [
            {
              game_id:
                games.find((g) => g.status === 'completed')?.game_id ??
                games[0]!.game_id,
              duration_seconds: 4800,
              sessions: [
                {
                  id: 'demo-memory',
                  started_at: window.memory_day.start_at,
                  ended_at: window.memory_day.end_at,
                  duration_seconds: 4800,
                },
              ],
            },
          ]
        : [],
    queried_at: new Date().toISOString(),
    window,
  };
}

export const demoHomePreferences = reactive<
  Record<
    string,
    { preference: 'not_interested' | 'snoozed'; expires_at: string | null }
  >
>({});
export function demoCalendarGames(
  games: PreviewGame[],
  now: Date,
): PreviewGame[] {
  return games.map((g, i) => ({
    ...g,
    release_date:
      g.release_date ??
      (i < 2
        ? localDate(
            new Date(
              now.getFullYear() - 2,
              now.getMonth(),
              now.getDate() + 3 + i * 3,
            ),
          )
        : undefined),
  }));
}
