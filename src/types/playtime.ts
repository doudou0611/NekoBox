export interface PlaySession {
  id: string;
  game_id: string;
  game_title: string;
  install_id: string;
  started_at: string;
  ended_at: string | null;
  duration_seconds: number;
  process_name: string | null;
  end_reason: string | null;
  corrected: boolean;
  correction_reason: string | null;
}
export interface PlaytimeStats {
  total_seconds: number;
  period_seconds: number;
  session_count: number;
  active_session_count: number;
  daily: { date: string; duration_seconds: number }[];
  games: { game_id: string; title: string; duration_seconds: number }[];
}
