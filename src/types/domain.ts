export const GAME_STATUSES = [
  'not_started',
  'playing',
  'paused',
  'completed',
  'dropped',
  'pending_confirmation',
] as const;
export type GameStatus = (typeof GAME_STATUSES)[number];
export const INSTALL_SOURCES = ['local', 'steam', 'manual', 'unknown'] as const;
export type InstallSource = (typeof INSTALL_SOURCES)[number];
export const METADATA_STATUSES = [
  'local_only',
  'queued',
  'syncing',
  'synced',
  'failed',
  'pending_confirmation',
] as const;
export type MetadataStatus = (typeof METADATA_STATUSES)[number];
export const TASK_STATUSES = [
  'queued',
  'running',
  'paused',
  'completed',
  'cancelled',
  'failed',
] as const;
export type TaskStatus = (typeof TASK_STATUSES)[number];
export type Timestamp = string;
export interface GameInstallation {
  id: string;
  game_id: string;
  absolute_path: string;
  executable_path: string | null;
  source: InstallSource;
  steam_app_id: string | null;
  path_valid: boolean;
}
export interface Tag {
  id: string;
  name: string;
  color: string | null;
}
export interface GameSummary {
  hikari_field?: { app_id: number; released: boolean } | null;
  id: string;
  title: string;
  title_zh: string | null;
  title_ja: string | null;
  title_en: string | null;
  cover_url: string | null;
  developer: string | null;
  publisher: string | null;
  release_date: string | null;
  source_rating: number | null;
  source_tags: string[];
  status: GameStatus;
  favorite: boolean;
  hidden: boolean;
  user_rating: number | null;
  total_playtime_seconds: number;
  last_played_at: Timestamp | null;
  added_at: Timestamp;
  metadata_status: MetadataStatus;
  installations: GameInstallation[];
  tags: Tag[];
  has_save_backup?: boolean;
}
export interface Screenshot {
  id: string;
  game_id: string;
  install_id: string | null;
  image_url: string;
  thumbnail_url: string;
  title: string | null;
  is_spoiler: boolean;
  captured_at: Timestamp | null;
  created_at: Timestamp;
}
export interface Note {
  id: string;
  game_id: string;
  title: string;
  content_markdown: string;
  is_spoiler: boolean;
  created_at: Timestamp;
  updated_at: Timestamp;
}
export interface ExternalSource {
  kind: string;
  label: string;
  provider: string;
  url: string;
  remote_id: string | null;
  official: boolean;
}
export interface SourcedField {
  remote_id?: string | null;
  field: string;
  value: string;
  provider: string;
  fetched_at: Timestamp | null;
  cached: boolean;
  manually_edited: boolean;
}
export interface GameDetail extends GameSummary {
  metadata_locked?: boolean;
  description: string | null;
  metadata: SourcedField[];
}
export interface GameFilters {
  developer?: string | null;
  release_year?: number | null;
  min_playtime_seconds?: number | null;
  max_playtime_seconds?: number | null;
  played_after?: string | null;
  metadata_pending?: boolean | null;
  has_save_backup?: boolean | null;
  multiple_installations?: boolean | null;
  metadata_incomplete?: boolean | null;
}
export interface GameQuery {
  page: number;
  page_size: number;
  search: string;
  statuses: GameStatus[];
  sources: InstallSource[];
  tag_ids: string[];
  favorite: boolean | null;
  collection_id: string | null;
  sort: 'title' | 'added_at' | 'last_played_at' | 'playtime';
  direction: 'asc' | 'desc';
  filters?: GameFilters;
}
export interface Paginated<T> {
  items: T[];
  page: number;
  page_size: number;
  total: number;
}
export interface CollectionSummary {
  id: string;
  name: string;
  kind: 'normal' | 'smart';
  icon: string | null;
  color: string | null;
  cover_url: string | null;
  position: number;
  hidden: boolean;
  game_count: number;
}
export interface CollectionDetail extends CollectionSummary {
  query: GameQuery | null;
  member_ids: string[];
}
export interface SaveCollectionRequest {
  id: string | null;
  name: string;
  kind: 'normal' | 'smart';
  icon: string | null;
  color: string | null;
  cover_url: string | null;
  position: number;
  hidden: boolean;
  query: GameQuery | null;
  member_ids?: string[] | null;
}
export interface Recommendation {
  game_id: string;
  reason: string;
  source: 'local' | 'external';
  is_installed: boolean;
  action: 'launch' | 'view_details' | 'official_page';
  official_url: string | null;
}
export interface HomeSummary {
  dashboard?: import('./home').HomeDashboard | null;
  game_count: number;
  playing_count: number;
  completed_count: number;
  week_playtime_seconds: number;
  pending_match_count: number;
  save_issue_count: number;
  continue_game_ids: string[];
  recent_game_ids: string[];
}
export interface TaskProgress {
  task_id: string;
  status: TaskStatus;
  phase: string;
  processed: number;
  total: number | null;
  message: string;
}
export interface ScanRootsRequest {
  roots: string[];
  follow_symlinks: false;
}
export interface ScanTask {
  task_id: string;
  status: TaskStatus;
}
export interface LaunchSession {
  session_id: string;
  game_id: string;
  install_id: string;
  started_at: Timestamp;
}
export interface SaveSnapshot {
  id: string;
  save_profile_id: string;
  game_id: string;
  install_id: string;
  created_at: Timestamp;
  size_bytes: number;
  sha256: string;
  label: string | null;
  note: string | null;
  file_count: number;
  creation_reason: string;
}
export interface RestoreResult {
  snapshot_id: string;
  safety_backup_id: string;
  added_files: number;
  modified_files: number;
  deleted_files: number;
}
export interface MetadataCandidate {
  provider: string;
  remote_id: string;
  title: string;
  subtitle?: string | null;
  cover_url?: string | null;
  has_chinese_description?: boolean | null;
  confidence: number;
  matched_fields: string[];
  explanation: string;
  fetched_at: Timestamp;
  cached: boolean;
}
export interface MatchResult {
  game_id: string;
  provider: string;
  remote_id: string;
  matched_at: Timestamp;
  translation_message?: string | null;
  supplementation_message?: string | null;
  cover_message?: string | null;
}
export interface PreferenceResult {
  game_id: string;
  preference: 'not_interested' | 'snoozed' | 'none';
  expires_at: Timestamp | null;
}

export interface RecommendationPreferenceEntry extends PreferenceResult {
  game_title: string;
}
