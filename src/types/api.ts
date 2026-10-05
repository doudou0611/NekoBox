import type {
  HikariFieldAccount,
  HikariFieldSettings,
  HikariFieldDownload,
} from './hikarifield';
import type { AppUpdateStatus } from './updates';
import type {
  BackupConfigView,
  SaveBackupConfig,
  ApplicationBackup,
  BackupStatus,
  ApplicationRestorePreview,
  MetadataRefreshStatus,
} from './backup';
import type { AppSettings } from './settings';
import type {
  HikarinagiAccount,
  BeginHikarinagiLogin,
  HikarinagiLoginFlow,
  VndbSettings,
  VndbConnection,
  AccountSyncReport,
} from './accounts';
import type {
  TranslationSettings,
  SaveTranslationSettings,
} from './translation';
import type {
  HikarinagiSettings,
  SaveHikarinagiSettings,
  HikarinagiRatesWall,
  HikarinagiReview,
  SubmitHikarinagiReview,
} from './hikarinagi';
import type {
  ImportPreparation,
  PrepareImportRequest,
  CommitImportRequest,
} from './importPreparation';
import type {
  CollectionDetail,
  CollectionSummary,
  GameDetail,
  GameQuery,
  GameSummary,
  HomeSummary,
  LaunchSession,
  MatchResult,
  MetadataCandidate,
  Paginated,
  PreferenceResult,
  RecommendationPreferenceEntry,
  Recommendation,
  RestoreResult,
  SaveCollectionRequest,
  SaveSnapshot,
  Screenshot,
  Note,
  ExternalSource,
  ScanRootsRequest,
  ScanTask,
} from './domain';
import type {
  BackendStatus,
  BangumiAccount,
  BangumiCoverStatus,
  ImportGameRequest,
  PreviewImportRequest,
  ImportPreviewReport,
  UpdateGameRequest,
  ConfigureInstallationRequest,
  InstallationDetails,
  ScanReport,
} from './local';
import type {
  DatabaseImportPreview,
  DatabaseExportResult,
  DatabaseTransferStatus,
} from './transfer';
import type { PlaySession, PlaytimeStats } from './playtime';
import type { ActivityQuery, ActivitySnapshot } from './activity';
import type {
  SaveProfile,
  ConfigureSaveProfile,
  RestorePreview,
} from './saves';
export const ERROR_CODES = [
  'INVALID_REQUEST',
  'NOT_FOUND',
  'PERMISSION_DENIED',
  'PATH_INVALID',
  'CONFLICT',
  'CANCELLED',
  'NETWORK_UNAVAILABLE',
  'RATE_LIMITED',
  'DATABASE_ERROR',
  'INTERNAL_ERROR',
  'NOT_IMPLEMENTED',
  'DESKTOP_UNAVAILABLE',
  'INVALID_RESPONSE',
] as const;
export type ErrorCode = (typeof ERROR_CODES)[number];
export type ApiResponse<T> =
  | {
      success: true;
      error_code: null;
      message: string;
      request_id: string;
      data: T;
    }
  | {
      success: false;
      error_code: ErrorCode;
      message: string;
      request_id: string;
      data: null;
    };
export interface ApiRequest<T> {
  request_id: string;
  payload: T;
}
export interface HealthCheckRequest {
  request_id: string;
}
export interface HealthStatus {
  version: string;
  platform: string;
}
/** Actual implementation status is tracked in shared/protocol.json. */
export interface CommandPayloads {
  hikarifield_account: Record<string, never>;
  login_hikarifield: { email: string; password: string };
  logout_hikarifield: Record<string, never>;
  sync_hikarifield: Record<string, never>;
  get_hikarifield_settings: Record<string, never>;
  set_hikarifield_path: { parent: string };
  start_hikarifield_download: { game_id: string; depot?: string };
  list_hikarifield_downloads: Record<string, never>;
  cancel_hikarifield_download: { task_id: string };

  check_app_update: Record<string, never>;
  get_app_update_status: Record<string, never>;
  download_app_update: Record<string, never>;
  install_app_update: { confirmed: true };
  open_app_update_release: { download: boolean };
  cache_remote_image: { url: string; avatar: boolean };
  open_bangumi_login: Record<string, never>;
  start_metadata_refresh: { confirmed: true };
  get_metadata_refresh: Record<string, never>;
  cancel_metadata_refresh: Record<string, never>;
  get_backup_settings: Record<string, never>;
  save_backup_settings: SaveBackupConfig;
  create_application_backup: {
    categories: string[];
    directory: string;
    password: string | null;
  };
  list_application_backups: Record<string, never>;
  delete_application_backup: {
    backup_id: string;
    path: string;
    confirmed: boolean;
  };
  preview_application_restore: { path: string; password: string | null };
  confirm_application_restore: {
    preview_id: string;
    confirmation_token: string;
    confirmed: true;
    categories: string[];
    save_destinations: Record<string, string>;
  };
  cancel_application_restore: Record<string, never>;
  get_application_backup_status: Record<string, never>;
  test_webdav: Record<string, never>;
  list_webdav_backups: Record<string, never>;
  upload_webdav_backup: { backup_id: string };
  download_webdav_backup: { file_name: string; password: string | null };

  get_app_settings: Record<string, never>;
  save_app_settings: AppSettings;
  cancel_metadata_search: { search_request_id: string };
  hikarinagi_account: Record<string, never>;
  begin_hikarinagi_login: Record<string, never>;
  poll_hikarinagi_login: { flow_id: string };
  cancel_hikarinagi_login: { flow_id: string };
  logout_hikarinagi: Record<string, never>;
  get_metadata_sources: Record<string, never>;
  save_metadata_sources: { sources: { provider: string; enabled: boolean }[] };
  get_vndb_settings: Record<string, never>;
  save_vndb_settings: { api_token: string | null; clear_api_token: boolean };
  test_vndb_connection: Record<string, never>;
  sync_account_play_data: {
    provider: 'bangumi' | 'hikarinagi';
    confirmed: boolean;
  };
  begin_import_batch: Record<string, never>;
  cancel_import_batch: { batch_id: string };
  prepare_import_metadata: PrepareImportRequest;
  discard_import_metadata: { preparation_ids: string[] };
  import_prepared_game: CommitImportRequest;
  get_translation_settings: Record<string, never>;
  get_hikarinagi_settings: Record<string, never>;
  get_hikarinagi_rates: { game_id: string; refresh: boolean };
  get_hikarinagi_review: { game_id: string; account_id: number };
  submit_hikarinagi_review: SubmitHikarinagiReview;
  save_hikarinagi_settings: SaveHikarinagiSettings;
  test_hikarinagi_connection: Record<string, never>;
  save_translation_settings: SaveTranslationSettings;
  test_translation: Record<string, never>;
  health_check: undefined;
  scan_roots: ScanRootsRequest;
  get_scan_task: { task_id: string };
  list_games: GameQuery;
  get_game: { game_id: string };
  launch_game: { install_id: string; options: { user_initiated: true } };
  list_save_snapshots: { game_id: string };
  restore_save_snapshot: {
    snapshot_id: string;
    preview_id: string;
    confirmation_token: string;
  };
  search_metadata: {
    manual?: boolean;
    query: string;
    providers: string[];
    cache?: boolean;
    batch_id?: string;
  };
  confirm_metadata_match: {
    manual?: boolean;
    title_hint?: string;
    game_id: string;
    provider: string;
    remote_id: string;
  };
  unbind_metadata: {
    game_id: string;
    provider: 'vndb' | 'bangumi' | 'hikarinagi';
    confirmed: true;
  };
  get_home_summary: { dashboard_window?: import('./home').HomeWindow };
  get_recommendations: { limit: number; excluded_game_ids: string[] };
  list_collections: Record<string, never>;
  reorder_collections: { collection_ids: string[] };
  save_collection: SaveCollectionRequest;
  set_collection_members: { collection_id: string; game_ids: string[] };
  set_recommendation_preference: {
    game_id: string;
    preference: 'not_interested' | 'snoozed' | 'none';
    expires_at: string | null;
  };
  list_recommendation_preferences: Record<string, never>;
  backend_status: Record<string, never>;
  export_database: { directory: string };
  preview_database_import: { path: string };
  confirm_database_import: { confirmation_token: string; confirmed: true };
  cancel_database_import: Record<string, never>;
  database_transfer_status: Record<string, never>;
  import_game: ImportGameRequest;
  preview_import: PreviewImportRequest;
  update_game: UpdateGameRequest;
  update_game_metadata: {
    game_id: string;
    changes: import('./detailWorkspace').MetadataChanges;
    expected: import('./detailWorkspace').MetadataChanges;
  };
  set_metadata_lock: { game_id: string; locked: boolean };
  list_game_processes: { install_id: string };
  select_game_process: {
    install_id: string;
    pid: number;
    created_at_ticks: string;
  };
  export_screenshots: {
    game_id: string;
    screenshot_ids: string[];
    batch: boolean;
  };
  delete_screenshots: {
    game_id: string;
    screenshot_ids: string[];
    confirmed: boolean;
  };
  scan_screenshots: { install_id: string };
  list_screenshots: { game_id: string; include_spoilers: boolean };
  update_screenshot: {
    game_id: string;
    screenshot_id: string;
    title: string;
    captured_at: string | null;
    is_spoiler: boolean;
  };
  list_notes: { game_id: string };
  save_note: {
    id: string | null;
    game_id: string;
    title: string;
    content_markdown: string;
    is_spoiler: boolean;
  };
  delete_note: { id: string; confirmed: true };
  replace_game_tags: { game_id: string; tag_names: string[] };
  list_external_sources: { game_id: string };
  bind_external_source: {
    game_id: string;
    install_id: string | null;
    provider: string;
    value: string;
  };
  open_external_source: { game_id: string; url: string };
  remove_game: { id: string; confirmed: true };
  get_installation: { install_id: string };
  configure_installation: ConfigureInstallationRequest;
  control_scan_task: { task_id: string; action: 'pause' | 'resume' | 'cancel' };
  get_collection: { collection_id: string };
  delete_collection: { id: string; confirmed: true };
  bangumi_account: Record<string, never>;
  login_bangumi: { access_token: string };
  logout_bangumi: Record<string, never>;
  get_bangumi_cover_status: { game_id: string };
  retry_bangumi_cover: { game_id: string; query: string | null };
  list_play_sessions: {
    game_id: string | null;
    page: number;
    page_size: number;
  };
  correct_play_session: {
    session_id: string;
    expected_duration_seconds: number;
    duration_seconds: number;
    reason: string;
    confirmed: true;
  };
  get_playtime_stats: { game_id: string | null; days: number };
  get_activity_snapshot: ActivityQuery;
  detect_save_paths: { install_id: string };
  list_save_profiles: { game_id: string };
  configure_save_profile: ConfigureSaveProfile;
  delete_save_profile: { profile_id: string; confirmed: true };
  create_save_snapshot: {
    profile_id: string;
    label: string | null;
    note: string | null;
  };
  preview_save_restore: { snapshot_id: string };
  delete_save_snapshot: { snapshot_id: string; confirmed: true };
}
export interface CommandResults {
  hikarifield_account: HikariFieldAccount;
  login_hikarifield: HikariFieldAccount;
  logout_hikarifield: HikariFieldAccount;
  sync_hikarifield: { owned: number; imported: number };
  get_hikarifield_settings: HikariFieldSettings;
  set_hikarifield_path: HikariFieldSettings;
  start_hikarifield_download: HikariFieldDownload;
  list_hikarifield_downloads: HikariFieldDownload[];
  cancel_hikarifield_download: boolean;

  check_app_update: AppUpdateStatus;
  get_app_update_status: AppUpdateStatus;
  download_app_update: AppUpdateStatus;
  install_app_update: AppUpdateStatus;
  open_app_update_release: boolean;
  cache_remote_image: string;
  open_bangumi_login: boolean;
  start_metadata_refresh: MetadataRefreshStatus;
  get_metadata_refresh: MetadataRefreshStatus;
  cancel_metadata_refresh: boolean;
  get_backup_settings: BackupConfigView;
  save_backup_settings: BackupConfigView;
  create_application_backup: ApplicationBackup;
  list_application_backups: ApplicationBackup[];
  delete_application_backup: { trash_path: string };
  preview_application_restore: ApplicationRestorePreview;
  confirm_application_restore: boolean;
  cancel_application_restore: boolean;
  get_application_backup_status: BackupStatus;
  test_webdav: boolean;
  list_webdav_backups: { file_name: string }[];
  upload_webdav_backup: boolean;
  download_webdav_backup: ApplicationBackup;

  get_app_settings: AppSettings;
  save_app_settings: AppSettings;
  cancel_metadata_search: boolean;
  hikarinagi_account: HikarinagiAccount;
  begin_hikarinagi_login: BeginHikarinagiLogin;
  poll_hikarinagi_login: HikarinagiLoginFlow;
  cancel_hikarinagi_login: boolean;
  logout_hikarinagi: HikarinagiAccount;
  get_metadata_sources: { sources: { provider: string; enabled: boolean }[] };
  save_metadata_sources: { sources: { provider: string; enabled: boolean }[] };
  get_vndb_settings: VndbSettings;
  save_vndb_settings: VndbSettings;
  test_vndb_connection: VndbConnection;
  sync_account_play_data: AccountSyncReport;
  begin_import_batch: {
    batch_id: string;
    sources: { sources: { provider: string; enabled: boolean }[] };
  };
  cancel_import_batch: boolean;
  prepare_import_metadata: ImportPreparation;
  discard_import_metadata: boolean;
  import_prepared_game: GameDetail;
  get_translation_settings: TranslationSettings;
  get_hikarinagi_settings: HikarinagiSettings;
  get_hikarinagi_rates: { wall: HikarinagiRatesWall | null };
  get_hikarinagi_review: { remote_id: string; review: HikarinagiReview | null };
  submit_hikarinagi_review: {
    review: HikarinagiReview;
    cache_warning: string | null;
  };
  save_hikarinagi_settings: HikarinagiSettings;
  test_hikarinagi_connection: boolean;
  save_translation_settings: TranslationSettings;
  test_translation: { translated_text: string };
  health_check: HealthStatus;
  scan_roots: ScanTask;
  get_scan_task: ScanReport;
  list_games: Paginated<GameSummary>;
  get_game: GameDetail;
  launch_game: LaunchSession;
  list_save_snapshots: SaveSnapshot[];
  restore_save_snapshot: RestoreResult;
  search_metadata: MetadataCandidate[];
  confirm_metadata_match: MatchResult;
  unbind_metadata: boolean;
  get_home_summary: HomeSummary;
  get_recommendations: Recommendation[];
  list_collections: CollectionSummary[];
  reorder_collections: boolean;
  save_collection: CollectionDetail;
  set_collection_members: CollectionDetail;
  set_recommendation_preference: PreferenceResult;
  list_recommendation_preferences: RecommendationPreferenceEntry[];
  backend_status: BackendStatus;
  export_database: DatabaseExportResult;
  preview_database_import: DatabaseImportPreview;
  confirm_database_import: DatabaseTransferStatus;
  cancel_database_import: DatabaseTransferStatus;
  database_transfer_status: DatabaseTransferStatus;
  import_game: GameDetail;
  preview_import: ImportPreviewReport;
  update_game: GameDetail;
  update_game_metadata: GameDetail;
  set_metadata_lock: GameDetail;
  list_game_processes: {
    active_session: boolean;
    items: import('./detailWorkspace').RunningProcess[];
  };
  select_game_process: import('./detailWorkspace').RunningProcess;
  export_screenshots: {
    cancelled: boolean;
    completed: number;
    failures: string[];
  };
  delete_screenshots: {
    cancelled: boolean;
    completed: number;
    failures: string[];
  };
  scan_screenshots: number;
  list_screenshots: Screenshot[];
  update_screenshot: Screenshot;
  list_notes: Note[];
  save_note: Note;
  delete_note: boolean;
  replace_game_tags: import('./domain').Tag[];
  remove_game: boolean;
  get_installation: InstallationDetails;
  configure_installation: InstallationDetails;
  control_scan_task: ScanReport;
  get_collection: CollectionDetail;
  delete_collection: boolean;
  bangumi_account: BangumiAccount;
  login_bangumi: BangumiAccount;
  logout_bangumi: BangumiAccount;
  get_bangumi_cover_status: BangumiCoverStatus;
  retry_bangumi_cover: BangumiCoverStatus;
  list_play_sessions: Paginated<PlaySession>;
  correct_play_session: PlaySession;
  get_playtime_stats: PlaytimeStats;
  get_activity_snapshot: ActivitySnapshot;
  detect_save_paths: string[];
  list_save_profiles: SaveProfile[];
  configure_save_profile: SaveProfile;
  delete_save_profile: boolean;
  create_save_snapshot: SaveSnapshot;
  preview_save_restore: RestorePreview;
  delete_save_snapshot: boolean;
  list_external_sources: ExternalSource[];
  bind_external_source: ExternalSource[];
  open_external_source: boolean;
}
export const IMPLEMENTED_COMMANDS = [
  'hikarifield_account',
  'login_hikarifield',
  'logout_hikarifield',
  'sync_hikarifield',
  'get_hikarifield_settings',
  'set_hikarifield_path',
  'start_hikarifield_download',
  'list_hikarifield_downloads',
  'cancel_hikarifield_download',
  'cache_remote_image',
  'open_bangumi_login',
  'start_metadata_refresh',
  'get_metadata_refresh',
  'cancel_metadata_refresh',
  'get_backup_settings',
  'save_backup_settings',
  'create_application_backup',
  'list_application_backups',
  'delete_application_backup',
  'preview_application_restore',
  'confirm_application_restore',
  'cancel_application_restore',
  'get_application_backup_status',
  'test_webdav',
  'list_webdav_backups',
  'upload_webdav_backup',
  'download_webdav_backup',

  'health_check',
  'scan_roots',
  'get_scan_task',
  'list_games',
  'get_game',
  'launch_game',
  'search_metadata',
  'cancel_metadata_search',
  'confirm_metadata_match',
  'begin_import_batch',
  'cancel_import_batch',
  'prepare_import_metadata',
  'discard_import_metadata',
  'import_prepared_game',
  'get_translation_settings',
  'get_hikarinagi_settings',
  'get_hikarinagi_rates',
  'get_hikarinagi_review',
  'submit_hikarinagi_review',
  'save_hikarinagi_settings',
  'test_hikarinagi_connection',
  'save_translation_settings',
  'test_translation',
  'unbind_metadata',
  'get_home_summary',
  'get_recommendations',
  'list_collections',
  'reorder_collections',
  'save_collection',
  'set_collection_members',
  'set_recommendation_preference',
  'list_recommendation_preferences',
  'backend_status',
  'export_database',
  'preview_database_import',
  'confirm_database_import',
  'cancel_database_import',
  'database_transfer_status',

  'import_game',
  'preview_import',
  'update_game',
  'update_game_metadata',
  'set_metadata_lock',
  'list_game_processes',
  'select_game_process',
  'remove_game',
  'get_installation',
  'configure_installation',
  'control_scan_task',
  'get_collection',
  'delete_collection',
  'bangumi_account',
  'login_bangumi',
  'logout_bangumi',
  'get_bangumi_cover_status',
  'retry_bangumi_cover',
  'list_play_sessions',
  'correct_play_session',
  'get_playtime_stats',
  'get_activity_snapshot',
  'detect_save_paths',
  'list_save_profiles',
  'configure_save_profile',
  'delete_save_profile',
  'list_save_snapshots',
  'create_save_snapshot',
  'preview_save_restore',
  'restore_save_snapshot',
  'delete_save_snapshot',
  'export_screenshots',
  'delete_screenshots',
  'scan_screenshots',
  'list_screenshots',
  'update_screenshot',
  'list_notes',
  'save_note',
  'delete_note',
  'replace_game_tags',
  'list_external_sources',
  'bind_external_source',
  'open_external_source',
  'hikarinagi_account',
  'begin_hikarinagi_login',
  'poll_hikarinagi_login',
  'cancel_hikarinagi_login',
  'logout_hikarinagi',
  'get_metadata_sources',
  'save_metadata_sources',
  'get_vndb_settings',
  'save_vndb_settings',
  'test_vndb_connection',
  'sync_account_play_data',
  'get_app_settings',
  'save_app_settings',
  'check_app_update',
  'get_app_update_status',
  'download_app_update',
  'install_app_update',
  'open_app_update_release',
] as const satisfies readonly CommandName[];
export type CommandName = keyof CommandPayloads;
