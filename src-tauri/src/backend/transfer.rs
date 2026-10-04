use super::*;
use crate::database::transfer::DatabaseCounts;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    time::{Duration, Instant},
};
const MAX_BYTES: u64 = 128 * 1024 * 1024;
const PENDING: &str = "database.import.pending";

#[derive(Deserialize)]
pub struct ExportRequest {
    pub directory: String,
}
#[derive(Deserialize)]
pub struct PreviewRequest {
    pub path: String,
}
#[derive(Deserialize)]
pub struct ConfirmRequest {
    pub confirmation_token: String,
    pub confirmed: bool,
}
#[derive(Debug, Serialize)]
pub struct ExportResult {
    pub path: String,
    pub sha256: String,
    pub size_bytes: u64,
}
#[derive(Debug, Clone, Serialize)]
pub struct ImportPreview {
    pub confirmation_token: String,
    pub counts: DatabaseCounts,
    pub sha256: String,
    pub expires_at: String,
}
#[derive(Debug, Serialize)]
pub struct TransferStatus {
    pub pending_restart: bool,
    pub last_import_error: Option<String>,
}
#[derive(Clone, Serialize, Deserialize)]
struct Pending {
    file_name: String,
    sha256: String,
}
struct Prepared {
    pending: Pending,
    preview: ImportPreview,
    deadline: Instant,
    copy: StagedCopy,
}
struct StagedCopy {
    path: PathBuf,
    keep: bool,
}
impl Drop for StagedCopy {
    fn drop(&mut self) {
        if !self.keep {
            let _ = fs::remove_file(&self.path);
        }
    }
}
#[derive(Default)]
pub struct TransferManager {
    preview: Option<Prepared>,
}

fn directory(data: &Path, name: &str) -> Result<PathBuf> {
    let path = data.join(name);
    if fs::symlink_metadata(&path).is_ok_and(|m| scanner::linked(&m)) {
        return Err(permission("数据库备份目录不能是链接。"));
    }
    fs::create_dir_all(&path).map_err(|_| permission("无法创建数据库备份目录。"))?;
    let path = path
        .canonicalize()
        .map_err(|_| permission("无法访问数据库备份目录。"))?;
    if path.parent() != Some(data) {
        return Err(permission("数据库备份目录越界。"));
    }
    Ok(path)
}
fn read(path: &Path) -> Result<Vec<u8>> {
    let meta = fs::symlink_metadata(path).map_err(|_| invalid("无法读取数据库快照文件。"))?;
    if !meta.is_file() || scanner::linked(&meta) || meta.len() > MAX_BYTES {
        return Err(invalid("请选择不超过 128 MiB 的普通 SQLite 快照文件。"));
    }
    let mut bytes = Vec::new();
    fs::File::open(path)
        .map_err(|_| permission("数据库快照无法读取。"))?
        .take(MAX_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| permission("数据库快照读取失败。"))?;
    if bytes.len() as u64 > MAX_BYTES || !bytes.starts_with(b"SQLite format 3\0") {
        return Err(invalid("文件不是有效的 SQLite 数据库快照。"));
    }
    Ok(bytes)
}
fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn checked_path(data: &Path, pending: &Pending) -> Result<PathBuf> {
    let name = pending
        .file_name
        .strip_suffix(".sqlite3")
        .ok_or_else(|| invalid("导入任务文件名无效。"))?;
    uuid::Uuid::parse_str(name).map_err(|_| invalid("导入任务标识无效。"))?;
    if name.len() != 36 || pending.sha256.len() != 64 {
        return Err(invalid("导入任务标识无效。"));
    }
    let path = directory(data, "database-imports")?.join(&pending.file_name);
    if hash(&read(&path)?) != pending.sha256 {
        return Err(invalid("待导入文件已改变，拒绝替换当前库。"));
    }
    Ok(path)
}
impl Backend {
    pub fn export_database(&self, request: ExportRequest) -> Result<ExportResult> {
        let _gate = self
            .transfers
            .lock()
            .map_err(|_| invalid("数据库迁移服务忙。"))?;
        let directory = absolute_directory(&request.directory)?;
        let path = directory.join(format!("GalgameManager-{}.sqlite3", id()));
        self.database()?.snapshot_to(&path)?;
        let bytes = read(&path)?;
        Ok(ExportResult {
            path: path_text(&path)?,
            sha256: hash(&bytes),
            size_bytes: bytes.len() as u64,
        })
    }
    pub fn preview_database_import(&self, request: PreviewRequest) -> Result<ImportPreview> {
        let mut gate = self
            .transfers
            .lock()
            .map_err(|_| invalid("数据库迁移服务忙。"))?;
        if self.database()?.setting::<Pending>(PENDING)?.is_some() {
            return Err(invalid("已有等待重启的导入，请先取消。"));
        }
        let source = Path::new(&request.path);
        if !source.is_absolute() {
            return Err(invalid("请选择完整的数据库文件路径。"));
        }
        let mut wal_name = source.as_os_str().to_owned();
        wal_name.push("-wal");
        if fs::metadata(PathBuf::from(wal_name)).is_ok_and(|m| m.len() > 0) {
            return Err(invalid("请选择导出的快照，不要选择正在使用的 WAL 数据库。"));
        }
        let bytes = read(source)?;
        let name = format!("{}.sqlite3", id());
        let path = directory(&self.data_directory, "database-imports")?.join(&name);
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|_| permission("无法准备导入文件。"))?;
        let copy = StagedCopy {
            path: path.clone(),
            keep: false,
        };
        let saved = file.write_all(&bytes).and_then(|_| file.sync_all());
        drop(file);
        saved.map_err(|_| permission("无法保存导入文件。"))?;
        let verified = Database::verified_snapshot(&path)?;
        drop(verified);
        let mut candidate = Database::open(&path)?;
        candidate.sanitize_import()?;
        let counts = candidate.transfer_counts()?;
        drop(candidate);
        let sha256 = hash(&read(&path)?);
        let preview = ImportPreview {
            confirmation_token: id(),
            counts,
            sha256: sha256.clone(),
            expires_at: (chrono::Utc::now() + chrono::Duration::minutes(5)).to_rfc3339(),
        };
        gate.preview = Some(Prepared {
            pending: Pending {
                file_name: name,
                sha256,
            },
            preview: preview.clone(),
            deadline: Instant::now() + Duration::from_secs(300),
            copy,
        });
        Ok(preview)
    }
    pub fn confirm_database_import(&self, request: ConfirmRequest) -> Result<TransferStatus> {
        if !request.confirmed {
            return Err(invalid("导入数据库需要确认。"));
        }
        let mut gate = self
            .transfers
            .lock()
            .map_err(|_| invalid("数据库迁移服务忙。"))?;
        let prepared = gate
            .preview
            .as_ref()
            .ok_or_else(|| invalid("请重新选择并预览数据库。"))?;
        if prepared.preview.confirmation_token != request.confirmation_token
            || Instant::now() > prepared.deadline
        {
            return Err(invalid("确认凭据错误或已过期，请重新预览。"));
        }
        checked_path(&self.data_directory, &prepared.pending)?;
        {
            let db = self.database()?;
            db.ensure_recovery_idle_except(None)?;
            db.put_setting(PENDING, &prepared.pending)?;
        }
        let mut prepared = gate
            .preview
            .take()
            .expect("preview was checked under mutex");
        prepared.copy.keep = true;
        self.database_transfer_status()
    }
    pub fn database_transfer_status(&self) -> Result<TransferStatus> {
        let db = self.database()?;
        Ok(TransferStatus {
            pending_restart: db.setting::<Pending>(PENDING)?.is_some(),
            last_import_error: db.setting("database.import.error")?,
        })
    }
    pub fn cancel_database_import(&self) -> Result<TransferStatus> {
        let mut gate = self
            .transfers
            .lock()
            .map_err(|_| invalid("数据库迁移服务忙。"))?;
        let pending = self.database()?.setting::<Pending>(PENDING)?;
        self.database()?.delete_setting(PENDING)?;
        if let Some(pending) = pending {
            if let Ok(path) = checked_path(&self.data_directory, &pending) {
                let _ = fs::remove_file(path);
            }
        }
        gate.preview = None;
        self.database_transfer_status()
    }
}
pub fn apply_pending(db: &mut Database, data: &Path) -> Result<()> {
    let Some(pending) = db.setting::<Pending>(PENDING)? else {
        return Ok(());
    };
    let result = (|| -> Result<()> {
        db.ensure_recovery_idle_except(Some(PENDING))?;
        let path = checked_path(data, &pending)?;
        let source = Database::verified_snapshot(&path)?;
        let backup =
            directory(data, "database-backups")?.join(format!("before-import-{}.sqlite3", id()));
        db.snapshot_to(&backup)?;
        db.restore_verified_snapshot(&source)?;
        Ok(())
    })();
    if let Err(error) = result {
        db.delete_setting(PENDING)?;
        db.put_setting(
            "database.import.error",
            &format!("导入失败，原数据库已保留：{}", error.1),
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
