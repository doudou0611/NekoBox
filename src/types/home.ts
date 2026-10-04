import type { GameStatus } from './domain';
export interface HomeRange {
  start_at: string;
  end_at: string;
}
export interface HomeDay extends HomeRange {
  date: string;
}
export interface HomeWindow {
  time_zone: string;
  today: HomeRange;
  week: HomeRange;
  week_days: HomeDay[];
  memory_day: HomeRange | null;
}
export interface HomeDashboard {
  status_counts: Record<GameStatus, number> & { total: number };
  active_game_ids: string[];
  recently_played_game_ids: string[];
  today_seconds: number;
  week_seconds: number;
  week_daily: { date: string; duration_seconds: number }[];
  week_games: { game_id: string; duration_seconds: number }[];
  other_seconds: number;
  memory_games: {
    game_id: string;
    duration_seconds: number;
    sessions: {
      id: string;
      started_at: string;
      ended_at: string | null;
      duration_seconds: number;
    }[];
  }[];
  queried_at: string;
  window: HomeWindow;
}
