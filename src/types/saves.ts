export interface SaveProfile {
  id: string;
  game_id: string;
  install_id: string;
  source_path: string;
  backup_before_launch: boolean;
  backup_after_exit: boolean;
  retention_count: number;
  source_available: boolean;
  last_error: string | null;
}
export interface ConfigureSaveProfile {
  id: string | null;
  install_id: string;
  source_path: string;
  backup_before_launch: boolean;
  backup_after_exit: boolean;
  retention_count: number;
}
export interface RestorePreview {
  preview_id: string;
  confirmation_token: string;
  snapshot_id: string;
  source_path: string;
  expires_at: string;
  added_files: number;
  modified_files: number;
  deleted_files: number;
  preserved_files: number;
  changes: { path: string; change: 'added' | 'modified' | 'deleted' }[];
  truncated: boolean;
}
