import type { Paginated } from './domain';
import type { PlaySession } from './playtime';
export type ActivityRange = 'week' | 'days30' | 'month' | 'year' | 'all';
export interface ActivityQuery {
  range: ActivityRange;
  calendar_year: number | null;
  session_date: string | null;
  page: number;
  page_size: number;
}
export interface ActivityDay {
  date: string;
  duration_seconds: number;
  session_count: number;
}
export interface ActivitySnapshot {
  as_of: string;
  range: ActivityRange;
  range_start: string;
  range_end: string;
  summary: {
    total_seconds: number;
    played_count: number;
    completed_count: number;
    active_days: number;
    session_count: number;
    active_session_count: number;
  };
  /** Full calendar_year activity, independent of the main statistical range. */
  daily: ActivityDay[];
  trend_granularity: 'day' | 'week' | 'month' | 'year';
  trend: { date: string; end_date: string; duration_seconds: number }[];
  games: {
    game_id: string;
    title: string;
    cover_url: string | null;
    duration_seconds: number;
  }[];
  hours: { hour: number; duration_seconds: number }[];
  calendar_year: number;
  calendar_years: number[];
  sessions: Paginated<PlaySession>;
  session_date: string | null;
}
