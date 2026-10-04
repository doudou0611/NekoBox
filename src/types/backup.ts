export const BACKUP_CATEGORIES = [
  {
    value: 'metadata',
    label: '元数据',
    description: '名称、简介、刮削字段与来源',
  },
  { value: 'covers', label: '封面', description: '已缓存的作品和分组封面' },
  { value: 'playtime', label: '游玩记录', description: '会话、时长与修正记录' },
  {
    value: 'groups',
    label: '游戏分组',
    description: '普通分组、智能规则与成员',
  },
  {
    value: 'personal',
    label: '个人资料',
    description: '笔记、用户标签、评分与收藏',
  },
  { value: 'settings', label: '应用设置', description: '偏好、来源和启动配置' },
  {
    value: 'save_archives',
    label: '存档 ZIP 快照',
    description: '软件管理的存档备份',
  },
  {
    value: 'current_saves',
    label: '当前实际存档',
    description: '配置过的存档目录，需关闭游戏',
  },
  {
    value: 'screenshots',
    label: '截图附件',
    description: '原图随包保存，可独立恢复',
  },
  {
    value: 'credentials',
    label: '登录与 API 凭证',
    description: '使用你的密码加密整个备份包',
  },
] as const;
export interface BackupConfig {
  categories: string[];
  directory: string;
  automatic: boolean;
  on_startup: boolean;
  on_game_exit: boolean;
  interval_minutes: number;
  retention: number;
  upload_local: boolean;
  webdav_url: string;
  webdav_directory: string;
  webdav_username: string;
}
export interface BackupConfigView extends BackupConfig {
  has_webdav_password: boolean;
  has_backup_password: boolean;
}
export interface SaveBackupConfig extends BackupConfig {
  webdav_password: string | null;
  backup_password: string | null;
  confirmed_cleanup: boolean;
}
export interface ApplicationBackup {
  backup_id: string;
  path: string;
  created_at: string;
  reason: string;
  size_bytes: number;
  sha256: string;
  categories: string[];
  encrypted: boolean;
}
export interface BackupStatus {
  running: boolean;
  phase: string;
  message: string;
  last_backup: ApplicationBackup | null;
  pending_restore: boolean;
  cloud_status: string;
  last_upload_at: string | null;
}
export interface ApplicationRestorePreview {
  preview_id: string;
  confirmation_token: string;
  created_at: string;
  categories: string[];
  game_count: number;
  attachment_count: number;
  size_bytes: number;
  save_paths: Record<string, string>;
}
export interface MetadataRefreshStatus {
  status: string;
  total: number;
  processed: number;
  succeeded: number;
  skipped: number;
  failed: number;
  current: string | null;
  messages: string[];
}
