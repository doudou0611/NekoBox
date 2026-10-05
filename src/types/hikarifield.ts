export interface HikariFieldAccount {
  status: 'authenticated' | 'signed_out';
  profile: { id: number; name: string } | null;
  message: string;
}
export interface HikariFieldSettings {
  root: string | null;
  uuid: string;
}
export interface HikariFieldDownload {
  id: string;
  game_id: string;
  title: string;
  status: 'queued' | 'running' | 'completed' | 'failed' | 'cancelled';
  message: string;
  downloaded: number;
  total: number;
  speed: number;
  install_path: string;
}
