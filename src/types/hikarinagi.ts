export interface HikarinagiSettings {
  enabled: boolean;
  method: 'client_credentials' | 'access_token';
  client_id: string;
  has_client_secret: boolean;
  has_access_token: boolean;
  can_search: boolean;
}
export interface HikarinagiRatesWall {
  remote_id: string;
  source_url: string;
  average: number | null;
  rated_count: number;
  distribution: { score: number; count: number }[];
  status_counts: {
    completed: number;
    going: number;
    on_hold: number;
    dropped: number;
  };
  keywords: { word: string; count: number }[];
  fetched_at: string;
  cached: boolean;
  stale: boolean;
  message: string | null;
}
export interface SaveHikarinagiSettings {
  enabled: boolean;
  method: HikarinagiSettings['method'];
  client_id: string;
  client_secret: string | null;
  access_token: string | null;
  clear_credentials: boolean;
}
export interface HikarinagiReview {
  score: number | null;
  comment: string;
  updated_at: string;
}
export interface SubmitHikarinagiReview {
  game_id: string;
  account_id: number;
  remote_id: string;
  score: number;
  comment: string;
  expected_updated_at: string | null;
}
