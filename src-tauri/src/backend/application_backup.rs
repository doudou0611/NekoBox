//! Versioned application packages; selective data, bounded attachments and restart recovery.
use super::*;
use super::{
    credential::{NativeVault, Vault},
    saves::files,
};
use crate::database::application_backup::{selected, Data, CATEGORIES};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, HashMap},
    fs,
    io::{Cursor, Read, Write},
    time::{Duration, Instant},
};
use zip::{write::SimpleFileOptions, ZipArchive, ZipWriter};
pub const KEY: &str = "backup.config";
const PENDING: &str = "backup.restore.pending";
const JOURNAL: &str = "backup.restore.journal";
const LIMIT: u64 = 2 * 1024 * 1024 * 1024;
const SERVICES: [&str; 8] = [
    "dev.galgame.manager.bangumi",
    "dev.galgame.manager.translation",
    "dev.galgame.manager.translation.endpoint",
    "dev.galgame.manager.hikarinagi",
    "dev.galgame.manager.hikarinagi.account",
    "dev.galgame.manager.vndb",
    "dev.galgame.manager.webdav",
    "dev.galgame.manager.backup.password",
];
#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub categories: Vec<String>,
    pub directory: String,
    pub automatic: bool,
    pub on_startup: bool,
    pub on_game_exit: bool,
    pub interval_minutes: u32,
    pub retention: u32,
    pub upload_local: bool,
    pub webdav_url: String,
    pub webdav_directory: String,
    pub webdav_username: String,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            categories: CATEGORIES.iter().map(|s| s.to_string()).collect(),
            directory: String::new(),
            automatic: false,
            on_startup: false,
            on_game_exit: false,
            interval_minutes: 0,
            retention: 10,
            upload_local: false,
            webdav_url: String::new(),
            webdav_directory: String::new(),
            webdav_username: String::new(),
        }
    }
}
#[derive(Serialize)]
pub struct ConfigView {
    #[serde(flatten)]
    pub config: Config,
    pub has_webdav_password: bool,
    pub has_backup_password: bool,
}
#[derive(Deserialize)]
pub struct SaveConfig {
    #[serde(flatten)]
    pub config: Config,
    pub webdav_password: Option<String>,
    pub backup_password: Option<String>,
    #[serde(default)]
    pub confirmed_cleanup: bool,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub version: u32,
    pub schema_version: u32,
    pub app_version: String,
    pub backup_id: String,
    pub created_at: String,
    pub reason: String,
    pub categories: Vec<String>,
    pub files: BTreeMap<String, saves::types::FileStamp>,
    pub current_save_paths: BTreeMap<String, String>,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Backup {
    pub backup_id: String,
    pub path: String,
    pub created_at: String,
    pub reason: String,
    pub size_bytes: u64,
    pub sha256: String,
    pub categories: Vec<String>,
    pub encrypted: bool,
}
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Status {
    pub running: bool,
    pub phase: String,
    pub message: String,
    pub last_backup: Option<Backup>,
    pub pending_restore: bool,
    pub cloud_status: String,
    pub last_upload_at: Option<String>,
}
#[derive(Default)]
pub struct Manager {
    pub state: Status,
    pub previews: HashMap<String, PreviewState>,
    automatic_queue: std::collections::VecDeque<&'static str>,
    automatic_worker: bool,
}
pub struct PreviewState {
    pub preview: RestorePreview,
    pub sha256: String,
    pub bytes: Vec<u8>,
    pub password: String,
    pub expires: Instant,
}
#[derive(Deserialize)]
pub struct CreateRequest {
    pub categories: Vec<String>,
    pub directory: String,
    pub password: Option<String>,
}
#[derive(Deserialize)]
pub struct DeleteRequest {
    pub backup_id: String,
    pub path: String,
    pub confirmed: bool,
}
#[derive(Serialize)]
pub struct DeleteResult {
    pub trash_path: String,
}
#[derive(Deserialize)]
pub struct PreviewRequest {
    pub path: String,
    pub password: Option<String>,
}
#[derive(Clone, Serialize)]
pub struct RestorePreview {
    pub preview_id: String,
    pub confirmation_token: String,
    pub created_at: String,
    pub categories: Vec<String>,
    pub game_count: usize,
    pub attachment_count: usize,
    pub size_bytes: u64,
    pub save_paths: BTreeMap<String, String>,
}
#[derive(Deserialize)]
pub struct ConfirmRequest {
    pub preview_id: String,
    pub confirmation_token: String,
    pub confirmed: bool,
    pub categories: Vec<String>,
    pub save_destinations: BTreeMap<String, String>,
}
#[derive(Clone, Serialize, Deserialize)]
struct Pending {
    file: String,
    sha256: String,
    categories: Vec<String>,
    save_destinations: BTreeMap<String, String>,
    current_manifests: BTreeMap<String, saves::types::Manifest>,
}
#[derive(Clone, Serialize, Deserialize)]
struct Swap {
    target: String,
    stage: String,
    original: String,
}
#[derive(Clone, Serialize, Deserialize)]
struct Journal {
    committed: bool,
    #[serde(default)]
    credentials: bool,
    swaps: Vec<Swap>,
    safety_file: String,
}
pub struct Package {
    pub manifest: Manifest,
    pub data: Data,
    pub attachments: BTreeMap<String, Vec<u8>>,
    pub credentials: BTreeMap<String, Option<String>>,
}
pub(crate) fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn io(_: impl std::fmt::Display) -> ServiceError {
    ServiceError(
        ErrorCode::PermissionDenied,
        "备份文件无法访问或写入，请检查权限和磁盘空间。",
    )
}
fn categories(values: &[String]) -> Result<()> {
    if values.is_empty()
        || values.len() > 10
        || values
            .iter()
            .any(|s| !CATEGORIES.contains(&s.as_str()) && s != "credentials")
        || values
            .iter()
            .collect::<std::collections::HashSet<_>>()
            .len()
            != values.len()
    {
        return Err(invalid("请选择有效且不重复的备份类别。"));
    }
    Ok(())
}
fn bounded(path: &Path) -> Result<Vec<u8>> {
    files::ancestors(path)?;
    if fs::metadata(path).map_err(io)?.len() > LIMIT {
        return Err(invalid("备份包超过 2 GB 限制。"));
    }
    fs::read(path).map_err(io)
}
fn managed_directory(b: &Backend) -> Result<PathBuf> {
    let p = b.data_directory.join("application-backups");
    fs::create_dir_all(&p).map_err(io)?;
    files::ancestors(&p)?;
    Ok(p)
}
fn default_directory(b: &Backend, c: &Config) -> Result<PathBuf> {
    if c.directory.is_empty() {
        managed_directory(b)
    } else {
        absolute_directory(&c.directory)
    }
}
fn credential_values(b: &Backend) -> Result<BTreeMap<String, Option<String>>> {
    SERVICES
        .into_iter()
        .map(|s| Ok((s.into(), NativeVault::new(b, s)?.read()?)))
        .collect()
}
fn write_credentials(b: &Backend, values: &BTreeMap<String, Option<String>>) -> Result<()> {
    if values.keys().any(|s| !SERVICES.contains(&s.as_str())) {
        return Err(invalid("备份包含未知凭据。"));
    }
    for (s, v) in values {
        let vault = NativeVault::new(b, s)?;
        if let Some(v) = v {
            vault.write(v)?
        } else {
            vault.delete()?
        }
    }
    Ok(())
}
pub fn get_config(b: &Backend) -> Result<ConfigView> {
    let config = b.database()?.setting(KEY)?.unwrap_or_default();
    Ok(ConfigView {
        config,
        has_webdav_password: NativeVault::new(b, "dev.galgame.manager.webdav")?
            .read()?
            .is_some(),
        has_backup_password: NativeVault::new(b, "dev.galgame.manager.backup.password")?
            .read()?
            .is_some(),
    })
}
pub fn save_config(b: &Backend, q: SaveConfig) -> Result<ConfigView> {
    categories(&q.config.categories)?;
    if !(1..=100).contains(&q.config.retention)
        || q.config.interval_minutes > 10080
        || q.config.automatic && !q.confirmed_cleanup
    {
        return Err(invalid("自动备份保留数量或清理确认无效。"));
    }
    if q.config.automatic
        && !q.config.on_startup
        && !q.config.on_game_exit
        && q.config.interval_minutes == 0
    {
        return Err(invalid("请为自动备份至少选择一个触发时机。"));
    }
    if q.config.upload_local && q.config.webdav_url.is_empty() {
        return Err(invalid("同步本地备份前请配置 WebDAV 地址。"));
    }
    default_directory(b, &q.config)?;
    if !q.config.webdav_url.is_empty() {
        super::webdav::base(&q.config)?;
    }
    if q.config.automatic
        && selected(&q.config.categories, "credentials")
        && q.backup_password
            .as_deref()
            .filter(|p| p.len() >= 8)
            .is_none()
        && NativeVault::new(b, "dev.galgame.manager.backup.password")?
            .read()?
            .is_none()
    {
        return Err(invalid("自动备份凭证需要先设置加密密码。"));
    }
    let old = b.database()?.setting::<Config>(KEY)?.unwrap_or_default();
    if (old.webdav_url != q.config.webdav_url || old.webdav_username != q.config.webdav_username)
        && q.webdav_password.is_none()
        && NativeVault::new(b, "dev.galgame.manager.webdav")?
            .read()?
            .is_some()
    {
        return Err(invalid(
            "更换 WebDAV 地址时请输入对应密码，避免向新地址发送旧凭据。",
        ));
    }
    if q.backup_password
        .as_ref()
        .is_some_and(|v| !v.is_empty() && !(8..=1024).contains(&v.len()))
    {
        return Err(invalid("备份密码需要 8～1024 字节。"));
    }
    if q.webdav_password.as_ref().is_some_and(|v| v.len() > 8192) {
        return Err(invalid("WebDAV 密码过长。"));
    }
    let webdav=q.webdav_password.map(|password|if password.is_empty(){String::new()}else{serde_json::json!({"url":q.config.webdav_url,"username":q.config.webdav_username,"password":password}).to_string()});
    let updates = [
        ("dev.galgame.manager.webdav", webdav),
        ("dev.galgame.manager.backup.password", q.backup_password),
    ];
    let mut previous = Vec::new();
    let operation = (|| -> Result<()> {
        for (service, value) in updates {
            if let Some(value) = value {
                let vault = NativeVault::new(b, service)?;
                previous.push((service, vault.read()?));
                if value.is_empty() {
                    vault.delete()?;
                } else {
                    vault.write(&value)?;
                }
            }
        }
        b.database()?.put_setting(KEY, &q.config)?;
        Ok(())
    })();
    if let Err(error) = operation {
        for (service, value) in previous.into_iter().rev() {
            let vault = NativeVault::new(b, service)?;
            if let Some(v) = value {
                vault.write(&v)?;
            } else {
                vault.delete()?;
            }
        }
        return Err(error);
    }
    get_config(b)
}
fn add(entries: &mut BTreeMap<String, Vec<u8>>, name: String, bytes: Vec<u8>) -> Result<()> {
    if !files::valid_name(&name) || entries.contains_key(&name) {
        return Err(invalid("备份附件路径无效或重复。"));
    }
    if entries.values().map(|v| v.len() as u64).sum::<u64>() + bytes.len() as u64 > LIMIT {
        return Err(invalid("所选内容超过 2 GB 限制。"));
    }
    entries.insert(name, bytes);
    Ok(())
}
fn capture(b: &Backend, chosen: Vec<String>, reason: &str) -> Result<Package> {
    categories(&chosen)?;
    let mut data = b.database()?.export_application_data(&chosen)?;
    let mut attachments = BTreeMap::new();
    let mut current_save_paths = BTreeMap::new();
    // Only referenced content is captured, never the entire application directory.
    if selected(&chosen, "covers") {
        let mut paths = std::collections::BTreeSet::new();
        for table in ["games", "collections"] {
            for r in data.tables.get(table).into_iter().flatten() {
                if let Some(p) = r.get("cover_path").and_then(serde_json::Value::as_str) {
                    if !p.is_empty() {
                        paths.insert(p.to_owned());
                    }
                }
            }
        }
        for r in data.tables.get("metadata_records").into_iter().flatten() {
            if r.get("field_name").and_then(serde_json::Value::as_str) == Some("cover_path") {
                if let Some(p) = r
                    .get("value_json")
                    .and_then(serde_json::Value::as_str)
                    .and_then(|s| serde_json::from_str::<String>(s).ok())
                {
                    paths.insert(p);
                }
            }
        }
        for path in data.collection_covers.values().flatten() {
            paths.insert(path.clone());
        }
        for p in paths {
            import_metadata::validate_cover(b, &p)?;
            let path = b.data_directory.join(&p);
            add(&mut attachments, p, bounded(&path)?)?;
        }
    }
    if selected(&chosen, "screenshots") {
        for r in data.tables.get_mut("screenshots").into_iter().flatten() {
            let raw = r
                .get("absolute_path")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| invalid("截图路径无效。"))?;
            let path = Path::new(raw);
            let ext = path
                .extension()
                .and_then(|s| s.to_str())
                .filter(|e| {
                    ["png", "jpg", "jpeg", "webp", "gif"].contains(&e.to_ascii_lowercase().as_str())
                })
                .ok_or_else(|| invalid("截图类型无效。"))?;
            let bytes = bounded(path)?;
            if bytes.len() > 64 * 1024 * 1024 {
                return Err(invalid("截图超过 64 MB。"));
            }
            image::load_from_memory(&bytes).map_err(|_| invalid("截图内容无效。"))?;
            let name = format!(
                "backup-screenshots/{}.{}",
                hash(&bytes),
                ext.to_ascii_lowercase()
            );
            attachments.entry(name.clone()).or_insert(bytes);
            r.insert("absolute_path".into(), serde_json::Value::String(name));
            r.insert("thumbnail_path".into(), serde_json::Value::Null);
        }
    }
    if selected(&chosen, "save_archives") {
        for r in data.tables.get("save_snapshots").into_iter().flatten() {
            let path = r
                .get("archive_path")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| invalid("快照路径无效。"))?;
            let relative = Path::new(path)
                .strip_prefix(&b.data_directory)
                .map_err(|_| invalid("快照不在软件管理目录中。"))?
                .to_str()
                .ok_or_else(|| invalid("快照路径编码无效。"))?
                .replace('\\', "/");
            if !relative.starts_with("save-backups/") {
                return Err(invalid("快照路径边界无效。"));
            }
            let bytes = bounded(Path::new(path))?;
            if r.get("sha256").and_then(serde_json::Value::as_str) != Some(hash(&bytes).as_str()) {
                return Err(invalid("存档快照已改变。"));
            }
            add(&mut attachments, relative, bytes)?;
        }
    }
    if selected(&chosen, "current_saves") {
        for r in data.tables.get("save_profiles").into_iter().flatten() {
            let profile = r["id"].as_str().ok_or_else(|| invalid("存档配置无效。"))?;
            let source = r["source_path"]
                .as_str()
                .ok_or_else(|| invalid("存档路径无效。"))?;
            if b.database()?
                .has_open_session(r["game_id"].as_str().unwrap_or_default())?
            {
                return Err(invalid("实际存档备份请先关闭游戏。"));
            }
            let root = files::directory(source)?;
            if root.starts_with(&b.data_directory) || b.data_directory.starts_with(&root) {
                return Err(invalid("存档目录与软件数据目录重叠。"));
            }
            let before = files::manifest(&root)?;
            for (name, stamp) in &before {
                let bytes = bounded(&root.join(name))?;
                if stamp.sha256 != hash(&bytes) {
                    return Err(invalid("备份期间存档发生改变。"));
                }
                add(
                    &mut attachments,
                    format!("current-saves/{profile}/{name}"),
                    bytes,
                )?;
            }
            if files::manifest(&root)? != before {
                return Err(invalid("备份期间存档发生改变。"));
            }
            current_save_paths.insert(profile.into(), source.into());
        }
    }
    let credentials = if selected(&chosen, "credentials") {
        credential_values(b)?
    } else {
        BTreeMap::new()
    };
    let manifest = Manifest {
        version: 1,
        schema_version: crate::database::SCHEMA_VERSION,
        app_version: env!("CARGO_PKG_VERSION").into(),
        backup_id: id(),
        created_at: now(),
        reason: reason.into(),
        categories: chosen,
        files: BTreeMap::new(),
        current_save_paths,
    };
    Ok(Package {
        manifest,
        data,
        attachments,
        credentials,
    })
}
pub fn encode(mut pack: Package, password: Option<&str>) -> Result<Vec<u8>> {
    let sensitive = selected(&pack.manifest.categories, "credentials");
    if sensitive && password.is_none_or(|p| p.len() < 8) {
        return Err(invalid("凭证备份需要至少 8 字节的加密密码。"));
    }
    add(
        &mut pack.attachments,
        "data.json".into(),
        serde_json::to_vec(&pack.data).map_err(io)?,
    )?;
    if sensitive {
        add(
            &mut pack.attachments,
            "credentials.json".into(),
            serde_json::to_vec(&pack.credentials).map_err(io)?,
        )?;
    }
    pack.manifest.files = pack
        .attachments
        .iter()
        .map(|(n, b)| {
            (
                n.clone(),
                saves::types::FileStamp {
                    size_bytes: b.len() as u64,
                    sha256: hash(b),
                },
            )
        })
        .collect();
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    zip.start_file("manifest.json", options).map_err(io)?;
    zip.write_all(&serde_json::to_vec(&pack.manifest).map_err(io)?)
        .map_err(io)?;
    for (name, bytes) in pack.attachments {
        zip.start_file(name, options).map_err(io)?;
        zip.write_all(&bytes).map_err(io)?;
    }
    let bytes = zip.finish().map_err(io)?.into_inner();
    if sensitive {
        backup_crypto::seal(bytes, password.unwrap())
    } else {
        Ok(bytes)
    }
}
pub fn decode(bytes: &[u8], password: Option<&str>) -> Result<Package> {
    if bytes.len() as u64 > LIMIT {
        return Err(invalid("备份包大小超限。"));
    }
    let plain = backup_crypto::open(bytes, password.unwrap_or_default())?;
    let mut zip =
        ZipArchive::new(Cursor::new(plain)).map_err(|_| invalid("不是有效的软件备份包。"))?;
    if zip.len() > 20000 {
        return Err(invalid("备份条目过多。"));
    }
    let mut entries = BTreeMap::new();
    let mut total = 0u64;
    let mut seen = std::collections::HashSet::new();
    for i in 0..zip.len() {
        let mut f = zip.by_index(i).map_err(io)?;
        let name = f.name().to_owned();
        if f.is_dir()
            || f.is_symlink()
            || !files::valid_name(&name)
            || !seen.insert(name.to_lowercase())
            || f.size() > LIMIT
            || total.saturating_add(f.size()) > LIMIT
        {
            return Err(invalid("备份包含不安全或过大的条目。"));
        }
        let expected = f.size();
        let mut value = Vec::new();
        f.by_ref()
            .take(expected + 1)
            .read_to_end(&mut value)
            .map_err(io)?;
        if value.len() as u64 != expected {
            return Err(invalid("备份条目大小不匹配。"));
        }
        total += expected;
        entries.insert(name, value);
    }
    let manifest: Manifest = serde_json::from_slice(
        &entries
            .remove("manifest.json")
            .ok_or_else(|| invalid("缺少备份清单。"))?,
    )
    .map_err(|_| invalid("备份清单无效。"))?;
    if manifest.version != 1
        || manifest.schema_version != crate::database::SCHEMA_VERSION
        || uuid::Uuid::parse_str(&manifest.backup_id).is_err()
        || chrono::DateTime::parse_from_rfc3339(&manifest.created_at).is_err()
    {
        return Err(invalid("备份版本或标识不兼容。"));
    }
    categories(&manifest.categories)?;
    if selected(&manifest.categories, "credentials") && !backup_crypto::encrypted(bytes) {
        return Err(invalid("含凭据的包必须加密。"));
    }
    if entries.len() != manifest.files.len() {
        return Err(invalid("备份附件清单不一致。"));
    }
    for (name, stamp) in &manifest.files {
        if !entries
            .get(name)
            .is_some_and(|b| b.len() as u64 == stamp.size_bytes && hash(b) == stamp.sha256)
        {
            return Err(invalid("备份完整性校验失败。"));
        }
        let allowed = name == "data.json"
            || name == "credentials.json" && selected(&manifest.categories, "credentials")
            || name.starts_with("covers/") && selected(&manifest.categories, "covers")
            || name.starts_with("backup-screenshots/")
                && selected(&manifest.categories, "screenshots")
            || name.starts_with("save-backups/") && selected(&manifest.categories, "save_archives")
            || name.starts_with("current-saves/")
                && selected(&manifest.categories, "current_saves");
        if !allowed {
            return Err(invalid("附件超出所选类别。"));
        }
    }
    let data: Data = serde_json::from_slice(
        &entries
            .remove("data.json")
            .ok_or_else(|| invalid("缺少备份数据。"))?,
    )
    .map_err(|_| invalid("备份记录无效。"))?;
    let credentials: BTreeMap<String, Option<String>> = entries
        .remove("credentials.json")
        .map(|b| serde_json::from_slice(&b))
        .transpose()
        .map_err(|_| invalid("凭据数据无效。"))?
        .unwrap_or_default();
    if selected(&manifest.categories, "credentials")
        && (credentials.len() != SERVICES.len()
            || SERVICES.iter().any(|s| !credentials.contains_key(*s)))
    {
        return Err(invalid("凭证类别清单不完整。"));
    }
    validate_package_references(&manifest, &data, &entries)?;
    let mut check = Database::in_memory()?;
    check.restore_application_data(&data, &manifest.categories)?;
    Ok(Package {
        manifest,
        data,
        attachments: entries,
        credentials,
    })
}
pub(crate) fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path.parent().ok_or_else(|| invalid("目标路径无效。"))?;
    files::ancestors(parent)?;
    let tmp = parent.join(format!(".{}.partial", id()));
    let result: Result<()> = (|| {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp)
            .map_err(io)?;
        file.write_all(bytes).map_err(io)?;
        file.sync_all().map_err(io)?;
        if path.exists() {
            return Err(invalid("目标文件已经存在，未覆盖。"));
        }
        fs::rename(&tmp, path).map_err(io)
    })();
    if result.is_err() {
        let _ = fs::remove_file(tmp);
    }
    result
}
pub fn create(b: &Backend, q: CreateRequest) -> Result<Backup> {
    create_with(b, q, "manual")
}
fn create_with(b: &Backend, q: CreateRequest, reason: &str) -> Result<Backup> {
    {
        let mut m = b.backups.lock().map_err(|_| invalid("备份服务不可用。"))?;
        if m.state.running {
            return Err(invalid("已有备份或云任务正在进行。"));
        }
        m.state.running = true;
        m.state.phase = "packing".into();
        m.state.message = "正在打包所选内容".into();
    }
    let result: Result<Backup> = (|| {
        let dir = if q.directory.is_empty() {
            let c: Config = b.database()?.setting(KEY)?.unwrap_or_default();
            default_directory(b, &c)?
        } else {
            absolute_directory(&q.directory)?
        };
        if selected(&q.categories, "credentials")
            && !q
                .password
                .as_deref()
                .is_some_and(|v| (8..=1024).contains(&v.len()))
        {
            return Err(invalid("备份凭证需要 8～1024 字节的加密密码。"));
        }
        let pack = capture(b, q.categories, reason)?;
        let manifest = pack.manifest.clone();
        let bytes = encode(pack, q.password.as_deref())?;
        decode(&bytes, q.password.as_deref())?;
        let path = dir.join(format!("GalgameManager-{}.gmbak", manifest.backup_id));
        atomic_write(&path, &bytes)?;
        let info = Backup {
            backup_id: manifest.backup_id,
            path: path_text(&path)?,
            created_at: manifest.created_at,
            reason: reason.into(),
            size_bytes: bytes.len() as u64,
            sha256: hash(&bytes),
            categories: manifest.categories,
            encrypted: backup_crypto::encrypted(&bytes),
        };
        let mut history: Vec<Backup> = b.database()?.setting("backup.history")?.unwrap_or_default();
        history.insert(0, info.clone());
        history.truncate(10000);
        b.database()?.put_setting("backup.history", &history)?;
        Ok(info)
    })();
    if let Ok(mut m) = b.backups.lock() {
        m.state.running = false;
        m.state.phase = "idle".into();
        match &result {
            Ok(v) => {
                m.state.last_backup = Some(v.clone());
                m.state.message = "本地备份已完成".into();
            }
            Err(e) => m.state.message = e.1.into(),
        }
    }
    if let Ok(info) = &result {
        let config: Config = b.database()?.setting(KEY)?.unwrap_or_default();
        if reason != "manual" {
            if let Err(error) = prune(b, &config) {
                if let Ok(mut manager) = b.backups.lock() {
                    manager.state.message = format!("本地备份已完成，旧包清理失败：{}", error.1);
                }
            }
        }
        if config.upload_local {
            let _ = super::webdav::upload(
                b,
                super::webdav::LocalRequest {
                    backup_id: info.backup_id.clone(),
                },
            );
        }
    }
    result
}
pub fn list(b: &Backend) -> Result<Vec<Backup>> {
    let history: Vec<Backup> = b.database()?.setting("backup.history")?.unwrap_or_default();
    Ok(history
        .into_iter()
        .filter(|v| Path::new(&v.path).is_file())
        .collect())
}
pub fn delete(b: &Backend, q: DeleteRequest) -> Result<DeleteResult> {
    if !q.confirmed {
        return Err(invalid("删除备份需要确认。"));
    }
    // Keep history changes and file moves exclusive with create/upload/download/prune.
    let mut manager = b.backups.lock().map_err(|_| invalid("备份服务不可用。"))?;
    if manager.state.running {
        return Err(invalid("已有备份或云任务正在进行，请完成后再删除。"));
    }
    if b.database()?.setting::<Pending>(PENDING)?.is_some()
        || b.database()?.setting::<Journal>(JOURNAL)?.is_some()
    {
        return Err(invalid("恢复任务尚未完成，请先取消待恢复或完成恢复。"));
    }
    let mut history: Vec<Backup> = b.database()?.setting("backup.history")?.unwrap_or_default();
    let index = history
        .iter()
        .position(|v| v.backup_id == q.backup_id && v.path == q.path)
        .ok_or_else(missing)?;
    let item = &history[index];
    let sha256 = item.sha256.clone();
    let path = Path::new(&item.path);
    if !path.is_absolute()
        || uuid::Uuid::parse_str(&item.backup_id).is_err()
        || path.file_name().and_then(|v| v.to_str())
            != Some(format!("GalgameManager-{}.gmbak", item.backup_id).as_str())
    {
        return Err(invalid("备份路径与登记记录不匹配，未删除。"));
    }
    files::ancestors(path)?;
    if !fs::metadata(path).map_err(io)?.is_file() {
        return Err(invalid("备份路径不是普通文件，未删除。"));
    }
    let stamp = files::digest_reader(&mut fs::File::open(path).map_err(io)?, LIMIT)?;
    if stamp.sha256 != item.sha256 {
        return Err(invalid("备份文件已改变，未删除，请检查原文件。"));
    }
    // Same-volume rename preserves encrypted bytes and needs no password or extra copy.
    let trash_root = path
        .parent()
        .ok_or_else(|| invalid("备份路径无效。"))?
        .join(".trash");
    fs::create_dir_all(&trash_root).map_err(io)?;
    files::ancestors(&trash_root)?;
    let trash_dir = trash_root.join(id());
    fs::create_dir(&trash_dir).map_err(io)?;
    let trash = trash_dir.join(path.file_name().ok_or_else(|| invalid("备份名称无效。"))?);
    let trash_path = path_text(&trash)?;
    let database = b.database()?;
    fs::rename(path, &trash).map_err(io)?;
    let original = item.path.clone();
    history.remove(index);
    if let Err(error) = database.put_setting("backup.history", &history) {
        if fs::rename(&trash, &original).is_err() {
            return Err(invalid(
                "备份列表更新失败，文件保留在原备份目录的 .trash 中。",
            ));
        }
        return Err(error);
    }
    if manager
        .state
        .last_backup
        .as_ref()
        .is_some_and(|v| v.path == original)
    {
        manager.state.last_backup = history
            .iter()
            .find(|v| Path::new(&v.path).is_file())
            .cloned();
    }
    manager.previews.retain(|_, v| v.sha256 != sha256);
    manager.state.message = "本地备份已移入回收目录，云端备份保留".into();
    Ok(DeleteResult { trash_path })
}
pub fn status(b: &Backend) -> Result<Status> {
    let mut state = b
        .backups
        .lock()
        .map_err(|_| invalid("备份服务不可用。"))?
        .state
        .clone();
    state.pending_restore = b.database()?.setting::<Pending>(PENDING)?.is_some();
    if state.message.is_empty() {
        state.message = b
            .database()?
            .setting::<String>("backup.restore.result")?
            .unwrap_or_default();
    }
    Ok(state)
}
fn prune(b: &Backend, c: &Config) -> Result<()> {
    let manager = b.backups.lock().map_err(|_| invalid("备份服务不可用。"))?;
    if manager.state.running {
        return Ok(());
    }
    let mut history: Vec<Backup> = b.database()?.setting("backup.history")?.unwrap_or_default();
    let mut count = 0;
    let mut removed = std::collections::HashSet::new();
    for item in &history {
        if !["startup", "game_exit", "interval"].contains(&item.reason.as_str()) {
            continue;
        }
        let directory = default_directory(b, c)?;
        if Path::new(&item.path).parent() != Some(directory.as_path()) {
            continue;
        }
        if c.upload_local
            && b.database()?
                .setting::<String>(&format!("backup.uploaded.{}", item.backup_id))?
                .is_none()
        {
            continue;
        }
        count += 1;
        if count <= c.retention {
            continue;
        }
        let path = Path::new(&item.path);
        if path.file_name().and_then(|s| s.to_str())
            != Some(format!("GalgameManager-{}.gmbak", item.backup_id).as_str())
        {
            continue;
        }
        if let Ok(bytes) = bounded(path) {
            if hash(&bytes) == item.sha256 {
                fs::remove_file(path).map_err(io)?;
                removed.insert(item.backup_id.clone());
            }
        }
    }
    history.retain(|v| !removed.contains(&v.backup_id));
    b.database()?.put_setting("backup.history", &history)
}
pub fn preview(b: &Backend, q: PreviewRequest) -> Result<RestorePreview> {
    if !Path::new(&q.path).is_absolute() {
        return Err(invalid("请选择绝对备份路径。"));
    }
    let bytes = bounded(Path::new(&q.path))?;
    let pack = decode(&bytes, q.password.as_deref())?;
    let preview = RestorePreview {
        preview_id: id(),
        confirmation_token: id(),
        created_at: pack.manifest.created_at,
        categories: pack.manifest.categories,
        game_count: pack.data.tables.get("games").map_or(0, Vec::len),
        attachment_count: pack.attachments.len(),
        size_bytes: bytes.len() as u64,
        save_paths: pack.manifest.current_save_paths,
    };
    let mut m = b.backups.lock().map_err(|_| invalid("备份服务不可用。"))?;
    m.previews.retain(|_, v| v.expires > Instant::now());
    if m.previews.len() > 8 {
        return Err(invalid("恢复预览过多。"));
    }
    m.previews.insert(
        preview.preview_id.clone(),
        PreviewState {
            preview: preview.clone(),
            sha256: hash(&bytes),
            bytes,
            password: q.password.unwrap_or_default(),
            expires: Instant::now() + Duration::from_secs(600),
        },
    );
    Ok(preview)
}
fn validate_save_target(b: &Backend, root: &Path, data: &Data) -> Result<()> {
    if root.starts_with(&b.data_directory) || b.data_directory.starts_with(root) {
        return Err(invalid("存档恢复目录不能与软件数据目录重叠。"));
    }
    let mut paths = b.database()?.installation_paths()?;
    paths.extend(
        data.tables
            .get("game_installations")
            .into_iter()
            .flatten()
            .filter_map(|row| row.get("absolute_path")?.as_str().map(str::to_owned)),
    );
    if paths.iter().any(|raw| {
        let path = Path::new(raw);
        path.canonicalize()
            .unwrap_or_else(|_| path.to_path_buf())
            .starts_with(root)
    }) {
        return Err(invalid("不能把整个游戏安装目录作为存档恢复目标。"));
    }
    Ok(())
}
pub fn confirm(b: &Backend, q: ConfirmRequest) -> Result<bool> {
    categories(&q.categories)?;
    if !q.confirmed {
        return Err(invalid("恢复需要确认。"));
    }
    let mut gate = b.backups.lock().map_err(|_| invalid("备份服务不可用。"))?;
    b.database()?.ensure_recovery_idle_except(None)?;
    let state = gate
        .previews
        .remove(&q.preview_id)
        .ok_or_else(|| invalid("恢复预览已失效。"))?;
    if state.expires < Instant::now()
        || state.preview.confirmation_token != q.confirmation_token
        || q.categories
            .iter()
            .any(|c| !state.preview.categories.contains(c))
    {
        return Err(invalid("恢复确认与预览不匹配。"));
    }
    let pack = decode(&state.bytes, Some(&state.password))?;
    let mut current_manifests = BTreeMap::new();
    let mut destinations = BTreeMap::new();
    if selected(&q.categories, "current_saves") {
        for profile in state.preview.save_paths.keys() {
            let raw = q
                .save_destinations
                .get(profile)
                .ok_or_else(|| invalid("请为每个实际存档确认恢复目录。"))?;
            let root = files::directory(raw)?;
            validate_save_target(b, &root, &pack.data)?;
            current_manifests.insert(profile.clone(), files::manifest(&root)?);
            destinations.insert(profile.clone(), path_text(&root)?);
        }
        let values = destinations.values().map(Path::new).collect::<Vec<_>>();
        if values.iter().enumerate().any(|(i, a)| {
            values
                .iter()
                .enumerate()
                .any(|(j, b)| i != j && (a.starts_with(b) || b.starts_with(a)))
        }) {
            return Err(invalid("实际存档恢复目录不能相互重叠。"));
        }
    }
    let dir = b.data_directory.join("application-restores");
    fs::create_dir_all(&dir).map_err(io)?;
    files::ancestors(&dir)?;
    let name = format!("{}.gmbak", id());
    atomic_write(&dir.join(&name), &state.bytes)?;
    if backup_crypto::encrypted(&state.bytes) {
        NativeVault::new(b, "dev.galgame.manager.restore.password")?.write(&state.password)?;
    }
    let pending = Pending {
        file: name,
        sha256: hash(&state.bytes),
        categories: q.categories,
        save_destinations: destinations,
        current_manifests,
    };
    {
        let db = b.database()?;
        db.ensure_recovery_idle_except(None)?;
        db.put_setting(PENDING, &pending)?;
    }
    Ok(true)
}
pub fn cancel_restore(b: &Backend) -> Result<bool> {
    b.database()?.delete_setting(PENDING)?;
    let _ = NativeVault::new(b, "dev.galgame.manager.restore.password")?.delete();
    Ok(true)
}
fn restore_assets(b: &Backend, pack: &mut Package, chosen: &[String]) -> Result<()> {
    for (name, bytes) in &pack.attachments {
        if name.starts_with("current-saves/") {
            continue;
        }
        let category = if name.starts_with("covers/") {
            "covers"
        } else if name.starts_with("backup-screenshots/") {
            "screenshots"
        } else {
            "save_archives"
        };
        if !selected(chosen, category) {
            continue;
        }
        if category == "covers" || category == "screenshots" {
            let filename = Path::new(name)
                .file_stem()
                .and_then(|s| s.to_str())
                .ok_or_else(|| invalid("图片文件名无效。"))?;
            if filename != hash(bytes) || bytes.len() > 64 * 1024 * 1024 {
                return Err(invalid("图片哈希或大小无效。"));
            }
            image::load_from_memory(bytes).map_err(|_| invalid("图片内容无效。"))?;
        }
        let path = b.data_directory.join(name);
        fs::create_dir_all(path.parent().unwrap()).map_err(io)?;
        files::ancestors(path.parent().unwrap())?;
        if path.exists() {
            if bounded(&path)? != *bytes {
                return Err(invalid("已有附件与备份不同，未覆盖。"));
            }
        } else {
            atomic_write(&path, bytes)?;
        }
    }
    if selected(chosen, "screenshots") {
        for row in pack
            .data
            .tables
            .get_mut("screenshots")
            .into_iter()
            .flatten()
        {
            let relative = row["absolute_path"]
                .as_str()
                .ok_or_else(|| invalid("截图路径无效。"))?;
            if !relative.starts_with("backup-screenshots/")
                || !pack.attachments.contains_key(relative)
            {
                return Err(invalid("截图附件缺失。"));
            }
            let path = b.data_directory.join(relative);
            let absolute = path_text(&path)?;
            let thumbnail = super::thumbnails::generate(&b.data_directory, &path)?;
            row.insert("absolute_path".into(), serde_json::Value::String(absolute));
            row.insert(
                "thumbnail_path".into(),
                serde_json::Value::String(thumbnail),
            );
        }
    }
    if selected(chosen, "save_archives") {
        for row in pack
            .data
            .tables
            .get_mut("save_snapshots")
            .into_iter()
            .flatten()
        {
            let profile = row["profile_id"]
                .as_str()
                .ok_or_else(|| invalid("存档归属无效。"))?;
            let snapshot = row["id"]
                .as_str()
                .ok_or_else(|| invalid("存档标识无效。"))?;
            uuid::Uuid::parse_str(profile)
                .and_then(|_| uuid::Uuid::parse_str(snapshot))
                .map_err(|_| invalid("存档标识不是合法 UUID。"))?;
            let relative = format!("save-backups/{profile}/{snapshot}.zip");
            if !pack.attachments.contains_key(&relative) {
                return Err(invalid("存档快照附件缺失。"));
            }
            row.insert(
                "archive_path".into(),
                serde_json::Value::String(path_text(&b.data_directory.join(relative))?),
            );
        }
    }
    Ok(())
}
fn journal_path(b: &Backend, name: &str) -> Result<PathBuf> {
    if !name.starts_with("safety-") || !name.ends_with(".sqlite3") || name.contains(['/', '\\']) {
        return Err(invalid("恢复安全快照路径无效。"));
    }
    Ok(b.data_directory.join("application-restores").join(name))
}
fn rollback(b: &Backend, j: &Journal) -> Result<()> {
    for swap in j.swaps.iter().rev() {
        let target = Path::new(&swap.target);
        let original = Path::new(&swap.original);
        let stage = Path::new(&swap.stage);
        let parent = target.parent().ok_or_else(|| invalid("恢复目录无效。"))?;
        if original.parent() != Some(parent)
            || stage.parent() != Some(parent)
            || !original
                .file_name()
                .is_some_and(|n| n.to_string_lossy().starts_with(".gm-original-"))
            || !stage
                .file_name()
                .is_some_and(|n| n.to_string_lossy().starts_with(".gm-restore-"))
        {
            return Err(invalid("恢复日志目录边界无效。"));
        }
        if original.exists() {
            files::ancestors(original)?;
            if target.exists() {
                files::ancestors(target)?;
                fs::remove_dir_all(target).map_err(io)?;
            }
            fs::rename(original, target).map_err(io)?;
        }
        if stage.exists() {
            files::ancestors(stage)?;
            fs::remove_dir_all(stage).map_err(io)?;
        }
    }
    let path = journal_path(b, &j.safety_file)?;
    let db = Database::verified_snapshot(&path)?;
    b.database()?.restore_verified_snapshot(&db)?;
    if j.credentials {
        if let Some(old) = NativeVault::new(b, "dev.galgame.manager.restore.credentials")?.read()? {
            let old = serde_json::from_str(&old).map_err(|_| invalid("凭据回滚状态无效。"))?;
            write_credentials(b, &old)?;
        }
    }
    b.database()?.delete_setting(JOURNAL)?;
    Ok(())
}
pub fn recover(b: &Backend) -> Result<()> {
    let existing: Option<Journal> = b.database()?.setting(JOURNAL)?;
    if let Some(journal) = existing {
        if !journal.committed {
            rollback(b, &journal)?;
        } else {
            cleanup_swaps(&journal)?;
            b.database()?.delete_setting(JOURNAL)?;
            b.database()?.delete_setting(PENDING)?;
        }
    }
    let pending: Option<Pending> = b.database()?.setting(PENDING)?;
    let Some(pending) = pending else {
        return Ok(());
    };
    let outcome = (|| -> Result<()> {
        if pending.file.contains(['/', '\\'])
            || !pending.file.ends_with(".gmbak")
            || uuid::Uuid::parse_str(pending.file.trim_end_matches(".gmbak")).is_err()
        {
            return Err(invalid("待恢复包标识无效。"));
        }
        let bytes = bounded(
            &b.data_directory
                .join("application-restores")
                .join(&pending.file),
        )?;
        if hash(&bytes) != pending.sha256 {
            return Err(invalid("待恢复包已改变。"));
        }
        let password = if backup_crypto::encrypted(&bytes) {
            NativeVault::new(b, "dev.galgame.manager.restore.password")?
                .read()?
                .ok_or_else(|| invalid("无法读取恢复密码。"))?
        } else {
            String::new()
        };
        let mut pack = decode(&bytes, Some(&password))?;
        let safety_file = format!("safety-{}.sqlite3", id());
        b.database()?.snapshot_to(&journal_path(b, &safety_file)?)?;
        let mut journal = Journal {
            committed: false,
            credentials: selected(&pending.categories, "credentials"),
            swaps: Vec::new(),
            safety_file,
        };
        if selected(&pending.categories, "credentials") {
            let old = credential_values(b)?;
            NativeVault::new(b, "dev.galgame.manager.restore.credentials")?
                .write(&serde_json::to_string(&old).map_err(io)?)?;
        }
        // Foreign package settings cannot become a local recovery journal.
        b.database()?.put_setting(JOURNAL, &journal)?;
        let apply = (|| -> Result<()> {
            if selected(&pending.categories, "current_saves") {
                for (profile, destination) in &pending.save_destinations {
                    let root = files::directory(destination)?;
                    validate_save_target(b, &root, &pack.data)?;
                    let expected = pending
                        .current_manifests
                        .get(profile)
                        .ok_or_else(|| invalid("缺少存档确认状态。"))?;
                    if files::manifest(&root)? != *expected {
                        return Err(invalid("确认后实际存档发生变化，恢复已停止。"));
                    }
                    let parent = root.parent().ok_or_else(|| invalid("存档目标无效。"))?;
                    let transaction = id();
                    let stage = parent.join(format!(".gm-restore-{transaction}"));
                    let original = parent.join(format!(".gm-original-{transaction}"));
                    fs::create_dir(&stage).map_err(io)?;
                    let safety = b
                        .data_directory
                        .join("application-restores")
                        .join(format!("save-safety-{transaction}.zip"));
                    files::write_archive(&root, &safety, expected)?;
                    let prefix = format!("current-saves/{profile}/");
                    for (name, bytes) in &pack.attachments {
                        if let Some(relative) = name.strip_prefix(&prefix) {
                            if !files::valid_name(relative) {
                                return Err(invalid("存档相对路径无效。"));
                            }
                            let path = stage.join(relative);
                            fs::create_dir_all(path.parent().unwrap()).map_err(io)?;
                            files::ancestors(path.parent().unwrap())?;
                            let mut file = fs::File::create(path).map_err(io)?;
                            file.write_all(bytes).map_err(io)?;
                            file.sync_all().map_err(io)?;
                        }
                    }
                    // Recheck immediately before replacing this explicitly approved directory.
                    if files::manifest(&root)? != *expected {
                        return Err(invalid("实际存档在恢复准备期间发生变化。"));
                    }
                    journal.swaps.push(Swap {
                        target: path_text(&root)?,
                        stage: path_text(&stage)?,
                        original: path_text(&original)?,
                    });
                    b.database()?.put_setting(JOURNAL, &journal)?;
                    fs::rename(&root, &original).map_err(io)?;
                    fs::rename(&stage, &root).map_err(io)?;
                }
            }
            restore_assets(b, &mut pack, &pending.categories)?;
            if selected(&pending.categories, "credentials") {
                write_credentials(b, &pack.credentials)?;
            }
            let mut data = pack.data.subset(&pending.categories);
            if selected(&pending.categories, "current_saves") {
                for row in data.tables.get_mut("save_profiles").into_iter().flatten() {
                    if let Some(path) = row
                        .get("id")
                        .and_then(serde_json::Value::as_str)
                        .and_then(|profile| pending.save_destinations.get(profile))
                    {
                        row.insert(
                            "source_path".into(),
                            serde_json::Value::String(path.clone()),
                        );
                    }
                }
            }
            b.database()?
                .restore_application_data(&data, &pending.categories)?;
            // The same transaction marks the journal committed (see repository).
            Ok(())
        })();
        if let Err(error) = apply {
            rollback(b, &journal)?;
            return Err(error);
        }
        cleanup_swaps(&journal)?;
        b.database()?.delete_setting(JOURNAL)?;
        b.database()?.delete_setting(PENDING)?;
        let _ = NativeVault::new(b, "dev.galgame.manager.restore.password")?.delete();
        let _ = NativeVault::new(b, "dev.galgame.manager.restore.credentials")?.delete();
        b.database()?.put_setting(
            "backup.restore.result",
            &"所选内容已恢复，恢复前安全快照已保留。",
        )?;
        Ok(())
    })();
    if let Err(error) = outcome {
        b.database()?
            .put_setting("backup.restore.result", &error.1)?;
        b.database()?.delete_setting(PENDING)?;
        return Ok(());
    }
    Ok(())
}
fn cleanup_swaps(j: &Journal) -> Result<()> {
    for s in &j.swaps {
        let original = Path::new(&s.original);
        let target = Path::new(&s.target);
        let stage = Path::new(&s.stage);
        if original.parent() != target.parent()
            || stage.parent() != target.parent()
            || !original
                .file_name()
                .is_some_and(|n| n.to_string_lossy().starts_with(".gm-original-"))
            || !stage
                .file_name()
                .is_some_and(|n| n.to_string_lossy().starts_with(".gm-restore-"))
        {
            return Err(invalid("恢复清理目录边界无效。"));
        }
        if original.exists() {
            files::ancestors(original)?;
            fs::remove_dir_all(original).map_err(io)?;
        }
    }
    Ok(())
}
pub fn trigger(b: &Backend, reason: &'static str) {
    let Ok(mut manager) = b.backups.lock() else {
        return;
    };
    if !manager.automatic_queue.contains(&reason) {
        manager.automatic_queue.push_back(reason);
    }
    if manager.automatic_worker {
        return;
    }
    manager.automatic_worker = true;
    drop(manager);
    let b = b.clone();
    std::thread::spawn(move || loop {
        let reason = {
            let Ok(mut manager) = b.backups.lock() else {
                return;
            };
            if manager.state.running {
                drop(manager);
                std::thread::sleep(Duration::from_millis(500));
                continue;
            }
            let Some(reason) = manager.automatic_queue.pop_front() else {
                manager.automatic_worker = false;
                return;
            };
            reason
        };
        let result = (|| -> Result<()> {
            let config: Config = b.database()?.setting(KEY)?.unwrap_or_default();
            if !config.automatic
                || reason == "startup" && !config.on_startup
                || reason == "game_exit" && !config.on_game_exit
                || reason == "interval" && config.interval_minutes == 0
            {
                return Ok(());
            }
            b.database()?.put_setting("backup.last_attempt", &now())?;
            let password = if selected(&config.categories, "credentials") {
                NativeVault::new(&b, "dev.galgame.manager.backup.password")?.read()?
            } else {
                None
            };
            create_with(
                &b,
                CreateRequest {
                    categories: config.categories,
                    directory: config.directory,
                    password,
                },
                reason,
            )?;
            b.database()?.put_setting("backup.last_auto", &now())?;
            Ok(())
        })();
        if let Err(error) = result {
            if let Ok(mut manager) = b.backups.lock() {
                if error.1 == "已有备份或云任务正在进行。" {
                    if !manager.automatic_queue.contains(&reason) {
                        manager.automatic_queue.push_front(reason);
                    }
                } else {
                    manager.state.message = error.1.into();
                }
            }
        }
    });
}
pub fn scheduler(b: &Backend) {
    trigger(b, "startup");
    let b = b.clone();
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_secs(15));
        let state = (|| -> Result<bool> {
            let config: Config = b.database()?.setting(KEY)?.unwrap_or_default();
            if !config.automatic || config.interval_minutes == 0 {
                return Ok(false);
            }
            let last: Option<String> = b.database()?.setting("backup.last_attempt")?;
            let elapsed = last
                .and_then(|v| chrono::DateTime::parse_from_rfc3339(&v).ok())
                .map(|v| (chrono::Utc::now() - v.with_timezone(&chrono::Utc)).num_seconds())
                .unwrap_or(i64::MAX);
            Ok(elapsed >= i64::from(config.interval_minutes) * 60)
        })();
        if matches!(state, Ok(true)) {
            trigger(&b, "interval");
        }
    });
}

fn validate_package_references(
    manifest: &Manifest,
    data: &Data,
    attachments: &BTreeMap<String, Vec<u8>>,
) -> Result<()> {
    if data
        .collection_covers
        .values()
        .flatten()
        .any(|p| !attachments.contains_key(p))
    {
        return Err(invalid("分组封面附件缺失。"));
    }
    for table in ["games", "collections"] {
        for row in data.tables.get(table).into_iter().flatten() {
            if let Some(path) = row.get("cover_path").and_then(serde_json::Value::as_str) {
                if !attachments.contains_key(path) {
                    return Err(invalid("封面附件缺失。"));
                }
            }
        }
    }
    for row in data.tables.get("screenshots").into_iter().flatten() {
        if !row
            .get("absolute_path")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|p| p.starts_with("backup-screenshots/") && attachments.contains_key(p))
        {
            return Err(invalid("截图附件缺失或不受管理。"));
        }
    }
    for row in data.tables.get("save_snapshots").into_iter().flatten() {
        let profile = row["profile_id"]
            .as_str()
            .ok_or_else(|| invalid("存档归属无效。"))?;
        let snapshot = row["id"]
            .as_str()
            .ok_or_else(|| invalid("存档 ID 无效。"))?;
        uuid::Uuid::parse_str(profile)
            .and_then(|_| uuid::Uuid::parse_str(snapshot))
            .map_err(|_| invalid("存档 ID 无效。"))?;
        let bytes = attachments
            .get(&format!("save-backups/{profile}/{snapshot}.zip"))
            .ok_or_else(|| invalid("存档快照附件缺失。"))?;
        if row["sha256"].as_str() != Some(hash(bytes).as_str())
            || row["size_bytes"].as_u64() != Some(bytes.len() as u64)
        {
            return Err(invalid("存档快照校验失败。"));
        }
        let mut zip =
            ZipArchive::new(Cursor::new(bytes)).map_err(|_| invalid("存档 ZIP 格式无效。"))?;
        if zip.len() > 10000 {
            return Err(invalid("存档 ZIP 条目过多。"));
        }
        let mut names = std::collections::HashSet::new();
        let mut total = 0u64;
        for i in 0..zip.len() {
            let mut entry = zip.by_index(i).map_err(io)?;
            if entry.is_dir()
                || entry.is_symlink()
                || !files::valid_name(entry.name())
                || !names.insert(entry.name().to_lowercase())
            {
                return Err(invalid("存档 ZIP 含不安全条目。"));
            }
            let stamp = files::digest_reader(&mut entry, 128 * 1024 * 1024)?;
            total += stamp.size_bytes;
            if total > files::MAX_TOTAL {
                return Err(invalid("存档 ZIP 大小超限。"));
            }
        }
    }
    for (name, bytes) in attachments {
        if name.starts_with("covers/") || name.starts_with("backup-screenshots/") {
            if Path::new(name).file_stem().and_then(|s| s.to_str()) != Some(hash(bytes).as_str())
                || bytes.len() > 64 * 1024 * 1024
            {
                return Err(invalid("图片哈希或大小无效。"));
            }
            image::load_from_memory(bytes).map_err(|_| invalid("图片内容无效。"))?;
        }
        if let Some(relative) = name.strip_prefix("current-saves/") {
            let Some((profile, _)) = relative.split_once('/') else {
                return Err(invalid("实际存档附件归属无效。"));
            };
            if !manifest.current_save_paths.contains_key(profile) {
                return Err(invalid("实际存档附件缺少配置归属。"));
            }
        }
    }
    if manifest.current_save_paths.keys().any(|id| {
        !data
            .tables
            .get("save_profiles")
            .into_iter()
            .flatten()
            .any(|r| r["id"].as_str() == Some(id.as_str()))
    }) {
        return Err(invalid("实际存档配置不存在。"));
    }
    for row in data.tables.get("settings").into_iter().flatten() {
        let Some(key) = row["key"].as_str() else {
            return Err(invalid("设置键无效。"));
        };
        let raw = row["value_json"]
            .as_str()
            .ok_or_else(|| invalid("设置数据类型无效。"))?;
        validate_persistent_setting(key, raw)?;
    }
    Ok(())
}

pub(crate) fn validate_persistent_setting(key: &str, raw: &str) -> Result<()> {
    match key {
        "app.preferences" => serde_json::from_str::<app_settings::Settings>(raw)
            .map_err(|_| invalid("应用偏好无效。"))?
            .validate()?,
        "metadata.translation" => translation::validate(
            &serde_json::from_str(raw).map_err(|_| invalid("翻译设置无效。"))?,
        )?,
        "metadata.sources" => serde_json::from_str::<metadata_sources::Config>(raw)
            .map_err(|_| invalid("来源设置无效。"))?
            .validate()?,
        "backup.config" => {
            let c: Config = serde_json::from_str(raw).map_err(|_| invalid("备份配置无效。"))?;
            categories(&c.categories)?;
            if !(1..=100).contains(&c.retention) || c.interval_minutes > 10080 {
                return Err(invalid("备份配置参数无效。"));
            }
            if !c.webdav_url.is_empty() {
                super::webdav::base(&c)?;
            }
        }
        "metadata.hikarinagi" => {
            serde_json::from_str::<super::hikarinagi_settings::Settings>(raw)
                .map_err(|_| invalid("Hikarinagi 来源设置无效。"))?;
        }
        key if key.starts_with("metadata.locked.") => {
            serde_json::from_str::<bool>(raw).map_err(|_| invalid("资料冻结状态无效。"))?;
        }
        key if key.starts_with("metadata.priority.") => {
            serde_json::from_str::<Vec<String>>(raw)
                .map_err(|_| invalid("资料来源优先级无效。"))?;
        }
        key if key.starts_with("metadata.generated_title.") => {
            serde_json::from_str::<String>(raw).map_err(|_| invalid("生成标题状态无效。"))?;
        }
        key if key.starts_with("launch.tools.") => {
            serde_json::from_str::<(Option<bool>, Option<bool>)>(raw)
                .map_err(|_| invalid("启动工具配置无效。"))?;
        }
        key if key.starts_with("playtime.corrections.") => {
            serde_json::from_str::<Vec<super::playtime::Correction>>(raw)
                .map_err(|_| invalid("游玩修正记录无效。"))?;
        }
        _ => {}
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn root() -> PathBuf {
        let p = std::env::temp_dir().join(format!("gm-app-backup-{}", id()));
        fs::create_dir_all(&p).unwrap();
        p.canonicalize().unwrap()
    }
    fn test_backup(b: &Backend, root: &Path) -> Backup {
        create(
            b,
            CreateRequest {
                categories: vec!["metadata".into()],
                directory: root.to_str().unwrap().into(),
                password: None,
            },
        )
        .unwrap()
    }
    fn deletion(item: &Backup, confirmed: bool) -> DeleteRequest {
        DeleteRequest {
            backup_id: item.backup_id.clone(),
            path: item.path.clone(),
            confirmed,
        }
    }
    #[test]
    fn delete_rolls_back_the_file_move_when_history_cannot_be_updated() {
        let root = root();
        let b = Backend::open(root.join("data")).unwrap();
        let item = test_backup(&b, &root);
        let original = fs::read(&item.path).unwrap();
        let connection =
            rusqlite::Connection::open(b.data_directory.join("galgame-manager.sqlite3")).unwrap();
        connection.execute_batch("CREATE TRIGGER reject_backup_deletion BEFORE UPDATE ON settings WHEN OLD.key = 'backup.history' BEGIN SELECT RAISE(ABORT, 'fixture history failure'); END;").unwrap();
        assert!(delete(&b, deletion(&item, true)).is_err());
        assert_eq!(fs::read(&item.path).unwrap(), original);
        assert_eq!(list(&b).unwrap().len(), 1);
        assert_eq!(status(&b).unwrap().last_backup.unwrap().path, item.path);
        connection
            .execute_batch("DROP TRIGGER reject_backup_deletion;")
            .unwrap();
        delete(&b, deletion(&item, true)).unwrap();
        assert!(list(&b).unwrap().is_empty());
        drop(connection);
        drop(b);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn delete_preserves_encrypted_bytes_without_requesting_the_password() {
        let root = root();
        let b = Backend::open(root.join("data")).unwrap();
        let mut item = test_backup(&b, &root);
        let original = fs::read(&item.path).unwrap();
        let encrypted = backup_crypto::seal(original.clone(), "fixture-password").unwrap();
        fs::write(&item.path, &encrypted).unwrap();
        item.sha256 = hash(&encrypted);
        item.encrypted = true;
        item.size_bytes = encrypted.len() as u64;
        b.database()
            .unwrap()
            .put_setting("backup.history", &vec![item.clone()])
            .unwrap();
        let result = delete(&b, deletion(&item, true)).unwrap();
        let bytes = fs::read(result.trash_path).unwrap();
        assert_eq!(bytes, encrypted);
        assert_eq!(
            backup_crypto::open(&bytes, "fixture-password").unwrap(),
            original
        );
        drop(b);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn delete_requires_confirmation_and_moves_only_the_selected_copy() {
        let root = root();
        let b = Backend::open(root.join("data")).unwrap();
        let item = test_backup(&b, &root);
        let original = fs::read(&item.path).unwrap();
        let cloud_dir = root.join("cloud-copy");
        fs::create_dir(&cloud_dir).unwrap();
        let mut copy = item.clone();
        copy.path = path_text(&cloud_dir.join(Path::new(&item.path).file_name().unwrap())).unwrap();
        fs::write(&copy.path, &original).unwrap();
        copy.reason = "cloud_download".into();
        b.database()
            .unwrap()
            .put_setting("backup.history", &vec![item.clone(), copy.clone()])
            .unwrap();
        b.database()
            .unwrap()
            .put_setting(&format!("backup.uploaded.{}", item.backup_id), &"uploaded")
            .unwrap();
        assert!(delete(&b, deletion(&item, false)).is_err());
        let mut unknown = deletion(&item, true);
        unknown.path = root.join("unregistered.gmbak").to_str().unwrap().into();
        assert!(delete(&b, unknown).is_err());
        assert_eq!(list(&b).unwrap().len(), 2);
        let result = delete(&b, deletion(&item, true)).unwrap();
        assert!(!Path::new(&item.path).exists());
        assert_eq!(fs::read(&result.trash_path).unwrap(), original);
        assert!(Path::new(&result.trash_path).starts_with(root.join(".trash")));
        assert_eq!(fs::read(&copy.path).unwrap(), original);
        assert_eq!(list(&b).unwrap()[0].path, copy.path);
        assert_eq!(status(&b).unwrap().last_backup.unwrap().path, copy.path);
        assert!(b
            .database()
            .unwrap()
            .setting::<String>(&format!("backup.uploaded.{}", item.backup_id))
            .unwrap()
            .is_some());
        assert!(delete(&b, deletion(&item, true)).is_err());
        drop(b);
        let b = Backend::open(root.join("data")).unwrap();
        assert_eq!(list(&b).unwrap().len(), 1);
        drop(b);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn delete_protects_running_tasks_pending_restore_and_restore_journal() {
        let root = root();
        let b = Backend::open(root.join("data")).unwrap();
        let item = test_backup(&b, &root);
        b.backups.lock().unwrap().state.running = true;
        assert!(delete(&b, deletion(&item, true)).is_err());
        b.backups.lock().unwrap().state.running = false;
        let preview = preview(
            &b,
            PreviewRequest {
                path: item.path.clone(),
                password: None,
            },
        )
        .unwrap();
        confirm(
            &b,
            ConfirmRequest {
                preview_id: preview.preview_id,
                confirmation_token: preview.confirmation_token,
                confirmed: true,
                categories: vec!["metadata".into()],
                save_destinations: BTreeMap::new(),
            },
        )
        .unwrap();
        assert!(delete(&b, deletion(&item, true)).is_err());
        cancel_restore(&b).unwrap();
        b.database()
            .unwrap()
            .put_setting(
                JOURNAL,
                &Journal {
                    committed: false,
                    credentials: false,
                    swaps: vec![],
                    safety_file: String::new(),
                },
            )
            .unwrap();
        assert!(delete(&b, deletion(&item, true)).is_err());
        b.database().unwrap().delete_setting(JOURNAL).unwrap();
        assert!(Path::new(&item.path).exists());
        assert_eq!(list(&b).unwrap().len(), 1);
        delete(&b, deletion(&item, true)).unwrap();
        assert!(status(&b).unwrap().last_backup.is_none());
        drop(b);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn delete_invalidates_unconfirmed_preview_and_rejects_changed_files() {
        let root = root();
        let b = Backend::open(root.join("data")).unwrap();
        let item = test_backup(&b, &root);
        let preview = preview(
            &b,
            PreviewRequest {
                path: item.path.clone(),
                password: None,
            },
        )
        .unwrap();
        let original = fs::read(&item.path).unwrap();
        fs::write(&item.path, b"changed").unwrap();
        assert!(delete(&b, deletion(&item, true)).is_err());
        assert_eq!(fs::read(&item.path).unwrap(), b"changed");
        assert_eq!(list(&b).unwrap().len(), 1);
        fs::write(&item.path, original).unwrap();
        delete(&b, deletion(&item, true)).unwrap();
        assert!(confirm(
            &b,
            ConfirmRequest {
                preview_id: preview.preview_id,
                confirmation_token: preview.confirmation_token,
                confirmed: true,
                categories: vec!["metadata".into()],
                save_destinations: BTreeMap::new(),
            }
        )
        .is_err());
        drop(b);
        fs::remove_dir_all(root).unwrap();
    }
    #[cfg(unix)]
    #[test]
    fn delete_rejects_linked_backup_and_linked_trash_directory() {
        let root = root();
        let b = Backend::open(root.join("data")).unwrap();
        let item = test_backup(&b, &root);
        let target = root.join("keep.gmbak");
        fs::rename(&item.path, &target).unwrap();
        std::os::unix::fs::symlink(&target, &item.path).unwrap();
        assert!(delete(&b, deletion(&item, true)).is_err());
        assert!(target.exists());
        fs::remove_file(&item.path).unwrap();
        fs::rename(&target, &item.path).unwrap();
        let elsewhere = root.join("elsewhere");
        fs::create_dir(&elsewhere).unwrap();
        std::os::unix::fs::symlink(&elsewhere, root.join(".trash")).unwrap();
        assert!(delete(&b, deletion(&item, true)).is_err());
        assert!(Path::new(&item.path).exists());
        assert_eq!(fs::read_dir(elsewhere).unwrap().count(), 0);
        drop(b);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn real_package_roundtrip_restores_selected_records_after_restart() {
        let root = root();
        let b = Backend::open(root.join("data")).unwrap();
        let game = b
            .database()
            .unwrap()
            .import_installation(
                "C:/missing/game",
                "Test game",
                None,
                crate::domain::protocol::InstallSource::Manual,
                &[],
                "fixture",
            )
            .unwrap();
        let q = CreateRequest {
            categories: vec!["metadata".into(), "personal".into(), "playtime".into()],
            directory: root.to_str().unwrap().into(),
            password: None,
        };
        let backup = create(&b, q).unwrap();
        assert_eq!(
            hash(&bounded(Path::new(&backup.path)).unwrap()),
            backup.sha256
        );
        let preview = preview(
            &b,
            PreviewRequest {
                path: backup.path,
                password: None,
            },
        )
        .unwrap();
        confirm(
            &b,
            ConfirmRequest {
                preview_id: preview.preview_id,
                confirmation_token: preview.confirmation_token,
                confirmed: true,
                categories: vec!["metadata".into()],
                save_destinations: BTreeMap::new(),
            },
        )
        .unwrap();
        b.database().unwrap().remove_game_record(&game).unwrap();
        drop(b);
        let b = Backend::open(root.join("data")).unwrap();
        assert_eq!(
            b.database().unwrap().get_game(&game).unwrap().summary.title,
            "Test game"
        );
        assert!(!status(&b).unwrap().pending_restore);
        drop(b);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn actual_saves_and_screenshots_restore_without_the_original_game_files() {
        let root = root();
        let game_root = root.join("game");
        let save_root = root.join("save");
        fs::create_dir_all(&game_root).unwrap();
        fs::create_dir_all(&save_root).unwrap();
        fs::write(save_root.join("slot.dat"), b"original save").unwrap();
        let image = game_root.join("screenshot.png");
        image::RgbImage::new(4, 4).save(&image).unwrap();
        let image_bytes = fs::read(&image).unwrap();
        let b = Backend::open(root.join("data")).unwrap();
        let game = b
            .database()
            .unwrap()
            .import_installation(
                game_root.to_str().unwrap(),
                "Game",
                None,
                crate::domain::protocol::InstallSource::Manual,
                &[],
                "fixture",
            )
            .unwrap();
        let install = b
            .database()
            .unwrap()
            .get_game(&game)
            .unwrap()
            .summary
            .installations[0]
            .id
            .clone();
        b.database()
            .unwrap()
            .upsert_screenshot(&game, &install, image.to_str().unwrap(), None)
            .unwrap();
        let profile = b
            .configure_save_profile(saves::types::ConfigureProfile {
                id: None,
                install_id: install,
                source_path: save_root.to_str().unwrap().into(),
                backup_before_launch: false,
                backup_after_exit: false,
                retention_count: 10,
            })
            .unwrap();
        let chosen = vec![
            "metadata".into(),
            "screenshots".into(),
            "current_saves".into(),
        ];
        let backup = create(
            &b,
            CreateRequest {
                categories: chosen.clone(),
                directory: root.to_str().unwrap().into(),
                password: None,
            },
        )
        .unwrap();
        fs::write(save_root.join("slot.dat"), b"new save").unwrap();
        fs::write(save_root.join("extra.dat"), b"extra").unwrap();
        let p = preview(
            &b,
            PreviewRequest {
                path: backup.path,
                password: None,
            },
        )
        .unwrap();
        confirm(
            &b,
            ConfirmRequest {
                preview_id: p.preview_id,
                confirmation_token: p.confirmation_token,
                confirmed: true,
                categories: chosen,
                save_destinations: BTreeMap::from([(
                    profile.id,
                    save_root.to_str().unwrap().into(),
                )]),
            },
        )
        .unwrap();
        fs::remove_dir_all(&game_root).unwrap();
        drop(b);
        let b = Backend::open(root.join("data")).unwrap();
        assert_eq!(
            fs::read(save_root.join("slot.dat")).unwrap(),
            b"original save"
        );
        assert!(!save_root.join("extra.dat").exists());
        assert!(!status(&b).unwrap().pending_restore);
        let data = b
            .database()
            .unwrap()
            .export_application_data(&["screenshots".into()])
            .unwrap();
        let restored = data.tables["screenshots"][0]["absolute_path"]
            .as_str()
            .unwrap();
        assert_eq!(fs::read(restored).unwrap(), image_bytes);
        assert!(fs::read_dir(root.join("data/application-restores"))
            .unwrap()
            .any(|e| e
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with("save-safety-")));
        drop(b);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn interrupted_external_swap_rolls_back_at_next_start() {
        let root = root();
        let b = Backend::open(root.join("data")).unwrap();
        fs::create_dir_all(root.join("data/application-restores")).unwrap();
        let safety_file = format!("safety-{}.sqlite3", id());
        b.database()
            .unwrap()
            .snapshot_to(&journal_path(&b, &safety_file).unwrap())
            .unwrap();
        let target = root.join("save");
        let original = root.join(format!(".gm-original-{}", id()));
        let stage = root.join(format!(".gm-restore-{}", id()));
        fs::create_dir(&target).unwrap();
        fs::create_dir(&original).unwrap();
        fs::write(target.join("slot.dat"), b"partially restored").unwrap();
        fs::write(original.join("slot.dat"), b"before restore").unwrap();
        b.database()
            .unwrap()
            .put_setting(
                JOURNAL,
                &Journal {
                    committed: false,
                    credentials: false,
                    safety_file,
                    swaps: vec![Swap {
                        target: target.to_str().unwrap().into(),
                        original: original.to_str().unwrap().into(),
                        stage: stage.to_str().unwrap().into(),
                    }],
                },
            )
            .unwrap();
        drop(b);
        let b = Backend::open(root.join("data")).unwrap();
        assert_eq!(
            fs::read(target.join("slot.dat")).unwrap(),
            b"before restore"
        );
        assert!(!original.exists());
        assert!(b
            .database()
            .unwrap()
            .setting::<Journal>(JOURNAL)
            .unwrap()
            .is_none());
        drop(b);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn malformed_future_and_traversal_packages_never_restore() {
        let root = root();
        let b = Backend::open(root.join("data")).unwrap();
        let pack = capture(&b, vec!["metadata".into()], "manual").unwrap();
        let bytes = encode(pack, None).unwrap();
        assert!(decode(&bytes, None).is_ok());
        assert!(decode(&bytes[..bytes.len() / 2], None).is_err());
        let mut archive = ZipWriter::new(Cursor::new(Vec::new()));
        archive
            .start_file("../outside", SimpleFileOptions::default())
            .unwrap();
        archive.write_all(b"evil").unwrap();
        assert!(decode(&archive.finish().unwrap().into_inner(), None).is_err());
        let mut pack = capture(&b, vec!["metadata".into()], "manual").unwrap();
        pack.manifest.version = 999;
        assert!(decode(&encode(pack, None).unwrap(), None).is_err());
        assert!(!root.join("outside").exists());
        drop(b);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn automatic_retention_keeps_manual_packages_and_previous_valid_backup() {
        let root = root();
        let b = Backend::open(root.join("data")).unwrap();
        let config = Config {
            directory: root.to_str().unwrap().into(),
            categories: vec!["metadata".into()],
            retention: 1,
            ..Config::default()
        };
        b.database().unwrap().put_setting(KEY, &config).unwrap();
        let manual = create_with(
            &b,
            CreateRequest {
                categories: config.categories.clone(),
                directory: config.directory.clone(),
                password: None,
            },
            "manual",
        )
        .unwrap();
        let first = create_with(
            &b,
            CreateRequest {
                categories: config.categories.clone(),
                directory: config.directory.clone(),
                password: None,
            },
            "startup",
        )
        .unwrap();
        assert!(Path::new(&first.path).exists());
        assert!(create_with(
            &b,
            CreateRequest {
                categories: vec!["credentials".into()],
                directory: config.directory.clone(),
                password: None
            },
            "startup"
        )
        .is_err());
        assert!(Path::new(&first.path).exists());
        let next = create_with(
            &b,
            CreateRequest {
                categories: config.categories,
                directory: config.directory,
                password: None,
            },
            "startup",
        )
        .unwrap();
        assert!(Path::new(&manual.path).exists());
        assert!(!Path::new(&first.path).exists());
        assert!(Path::new(&next.path).exists());
        drop(b);
        fs::remove_dir_all(root).unwrap();
    }
}

#[cfg(test)]
mod review_target_tests {
    use super::*;
    #[test]
    fn restoration_protects_locked_and_packaged_installations_and_allows_save_subdirectory() {
        let root = std::env::temp_dir().join(format!("gm-target-{}", id()));
        fs::create_dir_all(root.join("game/Save")).unwrap();
        let root = root.canonicalize().unwrap();
        let b = Backend::open(root.join("data")).unwrap();
        let game = b
            .database()
            .unwrap()
            .import_installation(
                root.join("game").to_str().unwrap(),
                "Game",
                None,
                crate::domain::protocol::InstallSource::Manual,
                &[],
                "fixture",
            )
            .unwrap();
        let data = b
            .database()
            .unwrap()
            .export_application_data(&["current_saves".into()])
            .unwrap();
        for locked in [false, true] {
            b.database()
                .unwrap()
                .put_setting(&format!("metadata.locked.{game}"), &locked)
                .unwrap();
            assert!(validate_save_target(&b, &root.join("game"), &data).is_err());
            assert!(validate_save_target(&b, &root, &data).is_err());
            assert!(validate_save_target(&b, &root.join("game/Save"), &data).is_ok());
        }
        b.database().unwrap().remove_game_record(&game).unwrap();
        assert!(validate_save_target(&b, &root.join("game"), &data).is_err());
        drop(b);
        fs::remove_dir_all(root).unwrap();
    }
}
