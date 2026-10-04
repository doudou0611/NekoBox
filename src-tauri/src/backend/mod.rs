//! Production services. No preview fixtures and no frontend-controlled SQL or shell.
pub mod account_sync;
pub mod activity;
pub mod app_settings;
pub mod application_backup;
pub mod backup_crypto;
pub mod bangumi;
pub mod bangumi_account;
mod credential;
pub mod detail_metadata;
pub mod hikarinagi;
pub mod hikarinagi_account;
mod hikarinagi_app;
pub mod hikarinagi_rates;
pub mod hikarinagi_review;
pub mod hikarinagi_settings;
mod http_retry;
pub mod import_metadata;
mod launch;
pub mod metadata_refresh;
pub mod metadata_search;
pub mod metadata_sources;
pub(crate) mod metadata_text;
pub mod network;
pub mod pe;
pub mod personal;
#[cfg(any(windows, test))]
mod play_clock;
pub mod playtime;
pub mod process_monitor;
pub mod process_selection;
pub mod saves;
pub mod scanner;
pub mod screenshot_actions;
pub mod sources;
pub mod thumbnails;
pub mod transfer;
pub mod translation;
pub mod types;
pub mod vndb;
pub mod vndb_settings;
pub mod webdav;

use crate::{database::Database, domain::protocol::ErrorCode};
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
use types::BackendStatus;

#[derive(Debug, Clone)]
pub struct ServiceError(pub ErrorCode, pub &'static str);
pub type Result<T> = std::result::Result<T, ServiceError>;
impl From<crate::database::DatabaseError> for ServiceError {
    fn from(_: crate::database::DatabaseError) -> Self {
        Self(
            ErrorCode::DatabaseError,
            "数据库操作失败，请检查便携数据目录及数据库版本。",
        )
    }
}
impl From<rusqlite::Error> for ServiceError {
    fn from(_: rusqlite::Error) -> Self {
        Self(ErrorCode::DatabaseError, "数据库操作失败，请检查数据目录。")
    }
}
pub fn invalid(message: &'static str) -> ServiceError {
    ServiceError(ErrorCode::InvalidRequest, message)
}
pub fn permission(message: &'static str) -> ServiceError {
    ServiceError(ErrorCode::PermissionDenied, message)
}
pub fn missing() -> ServiceError {
    ServiceError(ErrorCode::NotFound, "记录不存在。")
}
pub fn id() -> String {
    uuid::Uuid::new_v4().to_string()
}
pub fn now() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}
pub fn enum_text(value: &impl serde::Serialize) -> String {
    serde_json::to_value(value)
        .expect("enum serialization")
        .as_str()
        .expect("enum string")
        .to_owned()
}
pub fn absolute_directory(value: &str) -> Result<PathBuf> {
    let path = Path::new(value);
    if !path.is_absolute() || path.to_string_lossy().contains('\0') {
        return Err(ServiceError(
            ErrorCode::PathInvalid,
            "请输入完整的绝对目录路径。",
        ));
    }
    let canonical = path
        .canonicalize()
        .map_err(|_| ServiceError(ErrorCode::PathInvalid, "目录不存在或无法访问。"))?;
    if !canonical.is_dir() {
        return Err(ServiceError(ErrorCode::PathInvalid, "路径必须是目录。"));
    }
    Ok(canonical)
}
pub fn path_text(path: &Path) -> Result<String> {
    path.to_str()
        .map(str::to_owned)
        .ok_or(ServiceError(ErrorCode::PathInvalid, "路径编码不受支持。"))
}

/// Return a Chinese display label for a scraped source tag.
///
/// Providers do not expose a stable localized tag field. Keep the display
/// contract deterministic and offline: Chinese labels pass through, while the
/// common English VNDB/Bangumi labels are mapped locally. Unknown non-Chinese
/// labels are omitted instead of leaking English into the UI.
pub fn localized_source_tag(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    if metadata_text::is_chinese_description(value) {
        return Some(value.to_owned());
    }
    let key = value
        .to_ascii_lowercase()
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                ' '
            }
        })
        .collect::<String>();
    let translated = match key.split_whitespace().collect::<Vec<_>>().as_slice() {
        ["high", "school", "student", "protagonist"] => "高中生主角",
        ["musical", "environment"] => "音乐环境",
        ["instrumentalist", "protagonist"] => "乐器演奏主角",
        ["protagonist", "s", "childhood", "friend", "as", "a", "heroine"]
        | ["protagonist", "childhood", "friend", "as", "a", "heroine"] => "主角的青梅竹马女主",
        ["protagonist", "s", "cousin", "as", "a", "heroine"]
        | ["protagonist", "cousin", "as", "a", "heroine"] => "主角的表亲女主",
        ["ojousama", "heroine"] => "大小姐女主",
        ["singer", "heroine"] => "歌手女主",
        ["transfer", "student", "heroine"] => "转学生女主",
        ["female", "protagonist"] => "女性主角",
        ["male", "protagonist"] => "男性主角",
        ["multiple", "protagonists"] => "多主角",
        ["slice", "of", "life"] => "日常",
        ["science", "fiction"] | ["sci", "fi"] => "科幻",
        ["sexual", "content"] => "性内容",
        ["all", "ages"] => "全年龄",
        ["school", "life"] => "校园生活",
        ["little", "sister"] => "妹妹",
        ["older", "sister"] => "姐姐",
        ["school", "girl"] => "女学生",
        ["school", "boy"] => "男学生",
        ["magical", "girl"] => "魔法少女",
        ["rich", "girl"] => "大小姐",
        ["school", "club"] => "校园社团",
        ["protagonist", "childhood", "friend"] => "主角的青梅竹马",
        ["childhood", "friend"] => "青梅竹马",
        ["high", "school"] => "高中",
        ["school"] => "校园",
        ["romance"] => "恋爱",
        ["comedy"] => "喜剧",
        ["drama"] => "剧情",
        ["fantasy"] => "奇幻",
        ["mystery"] => "悬疑",
        ["action"] => "动作",
        ["horror"] => "恐怖",
        ["supernatural"] => "超自然",
        ["psychological"] => "心理",
        ["tragedy"] => "悲剧",
        ["yuri"] => "百合",
        ["yaoi"] => "耽美",
        ["music"] => "音乐",
        ["adult"] => "成人",
        ["student"] => "学生",
        ["teacher"] => "教师",
        ["maid"] => "女仆",
        ["tsundere"] => "傲娇",
        ["yandere"] => "病娇",
        ["kuudere"] => "冷娇",
        ["meganekko"] => "眼镜娘",
        ["idol"] => "偶像",
        ["singer"] => "歌手",
        ["musician"] => "音乐家",
        ["artist"] => "艺术家",
        ["writer"] => "作家",
        ["detective"] => "侦探",
        ["nurse"] => "护士",
        ["doctor"] => "医生",
        ["sister"] => "姐妹",
        ["cousin"] => "表亲",
        ["princess"] => "公主",
        ["ojousama"] => "大小姐",
        ["loli"] => "萝莉",
        ["lolicon"] => "恋童",
        ["club"] => "社团",
        _ => return None,
    };
    Some(translated.to_owned())
}

#[derive(Clone)]
pub struct Backend {
    pub db: Arc<Mutex<Database>>,
    pub translation_snapshot: Option<(translation::Settings, Option<String>)>,
    pub app_settings_snapshot: Option<app_settings::Settings>,
    pub process_controls: Arc<Mutex<process_selection::Controls>>,
    pub active_playtime: Arc<Mutex<std::collections::HashMap<String, u64>>>,
    pub backups: Arc<Mutex<application_backup::Manager>>,
    pub metadata_refresh: Arc<Mutex<metadata_refresh::Manager>>,
    pub metadata_snapshot: Option<metadata_sources::Config>,
    pub import_batch: Option<String>,
    pub metadata_supplement: bool,
    pub metadata_searches: Arc<Mutex<metadata_search::Searches>>,
    pub metadata_search_cancelled: Option<Arc<std::sync::atomic::AtomicBool>>,
    pub scans: Arc<scanner::ScanManager>,
    pub data_directory: PathBuf,
    pub launch_lock: Arc<Mutex<()>>,
    pub bangumi_lock: Arc<Mutex<()>>,
    pub translation_lock: Arc<Mutex<()>>,
    pub hikarinagi_lock: Arc<Mutex<()>>,
    pub hikarinagi_account_lock: Arc<Mutex<()>>,
    pub hikarinagi_login: Arc<Mutex<hikarinagi_account::LoginFlow>>,
    pub vndb_lock: Arc<Mutex<()>>,
    pub account_sync_lock: Arc<Mutex<()>>,
    pub cover_lock: Arc<Mutex<()>>,
    pub screenshot_lock: Arc<Mutex<()>>,
    pub saves: Arc<Mutex<saves::types::SaveManager>>,
    pub transfers: Arc<Mutex<transfer::TransferManager>>,
    pub prepared_imports: Arc<Mutex<import_metadata::PreparedImports>>,
    _data_lock: Arc<std::fs::File>,
}
impl Backend {
    pub fn unbind_metadata(
        &self,
        request: &crate::domain::requests::UnbindMetadataRequest,
    ) -> Result<()> {
        if !request.confirmed
            || !matches!(request.provider.as_str(), "vndb" | "bangumi" | "hikarinagi")
        {
            return Err(invalid("解绑资料需要确认且来源必须是 VNDB 或 Bangumi。"));
        }
        let _guard = self
            .launch_lock
            .lock()
            .map_err(|_| ServiceError(ErrorCode::InternalError, "数据库服务需要重新启动。"))?;
        let mut db = self.database()?;
        let backups = self.data_directory.join("database-backups");
        std::fs::create_dir_all(&backups).map_err(|_| {
            ServiceError(
                ErrorCode::PermissionDenied,
                "无法创建数据库安全快照，已保留资料。",
            )
        })?;
        db.snapshot_to(&backups.join(format!("before-unbind-{}.sqlite3", id())))?;
        db.unbind_metadata(&request.game_id, &request.provider)
    }
    pub fn remove_record(&self, id: &str, collection: bool) -> Result<bool> {
        let _launch = self
            .launch_lock
            .lock()
            .map_err(|_| ServiceError(ErrorCode::InternalError, "启动服务需要重新启动。"))?;
        let mut db = self.database()?;
        if db
            .setting::<serde_json::Value>("saves.restore.pending")?
            .is_some()
        {
            return Err(invalid("存档恢复未完成，请重新启动完成回滚后再移除记录。"));
        }
        if collection {
            db.get_collection(id)?;
        } else {
            db.get_game(id)?;
            if db.has_open_session(id)? {
                return Err(ServiceError(
                    ErrorCode::Conflict,
                    "游戏正在运行，结束后再移除库记录。",
                ));
            }
        }
        let backups = self.data_directory.join("database-backups");
        std::fs::create_dir_all(&backups).map_err(|_| {
            ServiceError(
                ErrorCode::PermissionDenied,
                "无法创建数据库安全快照，已保留原记录。",
            )
        })?;
        db.snapshot_to(&backups.join(format!("before-remove-{}.sqlite3", self::id())))?;
        if collection {
            db.delete_collection_record(id)
        } else {
            db.remove_game_record(id)
        }
    }
    pub fn open(data_directory: PathBuf) -> Result<Self> {
        std::fs::create_dir_all(&data_directory).map_err(|_| {
            ServiceError(
                ErrorCode::PermissionDenied,
                "无法创建 exe 旁 data 目录，请将程序放在可写目录。",
            )
        })?;
        let data_directory = data_directory
            .canonicalize()
            .map_err(|_| ServiceError(ErrorCode::PathInvalid, "无法访问便携数据目录。"))?;
        let lock = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(data_directory.join("app.lock"))
            .map_err(|_| {
                ServiceError(
                    ErrorCode::PermissionDenied,
                    "便携数据目录无法写入，请移动程序到可写目录。",
                )
            })?;
        lock.try_lock().map_err(|_| {
            ServiceError(
                ErrorCode::Conflict,
                "此便携数据目录已被另一实例使用，或系统不支持文件锁。",
            )
        })?;
        let mut db = Database::open(data_directory.join("galgame-manager.sqlite3"))?;
        #[cfg(not(test))]
        translation::pin_existing_endpoint(&data_directory, &db)?;
        transfer::apply_pending(&mut db, &data_directory)?;
        db.recover_interrupted()?;
        let backend = Self {
            db: Arc::new(Mutex::new(db)),
            scans: Arc::new(scanner::ScanManager::default()),
            data_directory,
            launch_lock: Arc::new(Mutex::new(())),
            bangumi_lock: Arc::new(Mutex::new(())),
            translation_lock: Arc::new(Mutex::new(())),
            hikarinagi_lock: Arc::new(Mutex::new(())),
            hikarinagi_account_lock: Arc::new(Mutex::new(())),
            hikarinagi_login: Arc::new(Mutex::new(hikarinagi_account::LoginFlow::default())),
            vndb_lock: Arc::new(Mutex::new(())),
            account_sync_lock: Arc::new(Mutex::new(())),
            cover_lock: Arc::new(Mutex::new(())),
            screenshot_lock: Arc::new(Mutex::new(())),
            saves: Arc::new(Mutex::new(saves::types::SaveManager::default())),
            transfers: Arc::new(Mutex::new(transfer::TransferManager::default())),
            translation_snapshot: None,
            app_settings_snapshot: None,
            process_controls: Arc::new(Mutex::new(process_selection::Controls::new())),
            active_playtime: Arc::new(Mutex::new(std::collections::HashMap::new())),
            backups: Arc::new(Mutex::new(application_backup::Manager::default())),
            metadata_refresh: Arc::new(Mutex::new(metadata_refresh::Manager::default())),
            metadata_snapshot: None,
            import_batch: None,
            metadata_supplement: false,
            metadata_searches: Arc::new(Mutex::new(metadata_search::Searches::default())),
            metadata_search_cancelled: None,
            prepared_imports: Arc::new(Mutex::new(import_metadata::PreparedImports::default())),
            _data_lock: Arc::new(lock),
        };
        backend.recover_save_restore()?;
        application_backup::recover(&backend)?;
        #[cfg(not(test))]
        network::configure(app_settings::get(&backend)?);
        Ok(backend)
    }
    pub fn database(&self) -> Result<std::sync::MutexGuard<'_, Database>> {
        self.ensure_search_active()?;
        let db = self
            .db
            .lock()
            .map_err(|_| ServiceError(ErrorCode::InternalError, "数据库服务需要重新启动。"))?;
        self.ensure_search_active()?;
        Ok(db)
    }
    pub fn status(&self) -> Result<BackendStatus> {
        let db = self.database()?;
        Ok(BackendStatus {
            data_directory: path_text(&self.data_directory)?,
            schema_version: db.schema_version()?,
            portable: true,
            last_scan_task_id: db.setting("scan.last_task")?,
        })
    }
}
/// Startup failures remain queryable; a corrupt/unwritable DB never becomes a fake empty library.
pub struct BackendState(pub Result<Backend>);
