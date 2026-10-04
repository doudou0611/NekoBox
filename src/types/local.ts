import type { GameStatus, InstallSource, TaskProgress } from './domain';
export interface BackendStatus {
  data_directory: string;
  schema_version: number;
  portable: boolean;
  last_scan_task_id: string | null;
}
export interface ImportGameRequest {
  directory: string;
  title: string;
  game_id: string | null;
  skip_metadata?: boolean;
}
export interface PreviewImportRequest {
  roots: string[];
  follow_symlinks: false;
  single_executable?: string | null;
  single_directory?: boolean;
}
export interface UpdateGameRequest {
  game_id: string;
  title: string;
  status: GameStatus;
  favorite: boolean;
  hidden: boolean;
  user_rating: number | null;
}
export interface ExecutableCandidate {
  path: string;
  product_name: string | null;
  file_description: string | null;
  company_name: string | null;
  fingerprint: string;
  title_evidence: { source: string; value: string; weight: number }[];
}
export interface ImportPreviewCandidate {
  directory: string;
  folder_name: string;
  search_name: string;
  executables: ExecutableCandidate[];
  selected_executable: string | null;
  existing_game_id: string | null;
  duplicate_reason: string | null;
}
export interface ImportPreviewReport {
  items: ImportPreviewCandidate[];
  scanned_directories: number;
  skipped_directories: number;
  issue_count: number;
  issues: { path: string; reason: string }[];
}
export interface ConfigureInstallationRequest {
  install_id: string;
  executable_path: string;
  arguments: string[];
  working_directory: string | null;
  environment: Record<string, string>;
  steam_app_id?: string | null;
  main_process_name?: string | null;
  track_after_launcher_exit?: boolean;
  idle_timeout_minutes?: number | null;
  use_locale_emulator?: boolean | null;
  use_magpie?: boolean | null;
}
export interface InstallationDetails {
  id: string;
  game_id: string;
  absolute_path: string;
  executable_path: string | null;
  source: InstallSource;
  steam_app_id: string | null;
  arguments: string[];
  working_directory: string | null;
  environment: Record<string, string>;
  candidates: ExecutableCandidate[];
  main_process_name: string | null;
  track_after_launcher_exit: boolean;
  idle_timeout_minutes: number | null;
  use_locale_emulator: boolean | null;
  use_magpie: boolean | null;
}
export interface ScanReport extends TaskProgress {
  imported: number;
  unchanged: number;
  issue_count: number;
  issues: { path: string; reason: string }[];
  truncated: boolean;
}
export interface BangumiProfile {
  id: number;
  username: string;
  nickname: string;
  avatar_url: string | null;
}
export interface BangumiAccount {
  status: 'signed_out' | 'authenticated' | 'offline' | 'expired';
  profile: BangumiProfile | null;
  message: string;
}
export interface BangumiCoverStatus {
  status: 'idle' | 'running' | 'incomplete' | 'success' | 'no_match' | 'failed';
  message: string;
  updated_at: string | null;
  remote_id: string | null;
}
