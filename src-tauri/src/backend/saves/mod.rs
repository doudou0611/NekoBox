pub(crate) mod files;
pub mod types;
use self::{files::*, types::*};
use super::{id, now, path_text, Backend, Result, ServiceError};
use crate::domain::{
    models::{RestoreResult, SaveSnapshot},
    requests::RestoreSaveSnapshotRequest,
};
use std::{
    fs,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

const JOURNAL: &str = "saves.restore.pending";
fn safe_id(value: &str) -> Result<()> {
    uuid::Uuid::parse_str(value).map_err(|_| super::invalid("存档记录标识无效。"))?;
    Ok(())
}
fn archive_path(b: &Backend, s: &SaveSnapshot) -> Result<PathBuf> {
    safe_id(&s.id)?;
    safe_id(&s.save_profile_id)?;
    Ok(b.data_directory
        .join("save-backups")
        .join(&s.save_profile_id)
        .join(format!("{}.zip", s.id)))
}
fn idle(b: &Backend, game: &str) -> Result<()> {
    if b.database()?.has_open_session(game)? {
        return Err(conflict(
            "游戏正在运行，请结束游戏后再配置、备份或恢复存档。",
        ));
    }
    Ok(())
}
fn unjournaled(b: &Backend) -> Result<()> {
    if b.database()?.setting::<RestoreJournal>(JOURNAL)?.is_some() {
        return Err(conflict("存在未完成的存档恢复，请重新启动软件完成回滚。"));
    }
    Ok(())
}
fn checked_source(b: &Backend, p: &SaveProfile) -> Result<PathBuf> {
    let root = directory(&p.source_path)?;
    let installation = b.database()?.installation(&p.install_id)?;
    let install = super::absolute_directory(&installation.absolute_path)?;
    if install.starts_with(&root)
        || root.starts_with(&b.data_directory)
        || b.data_directory.starts_with(&root)
    {
        return Err(conflict(
            "请选择独立的存档子目录，不可选择整个游戏目录或应用数据目录。",
        ));
    }
    Ok(root)
}
fn record_error(b: &Backend, profile: &str, error: Option<&ServiceError>) -> Result<()> {
    let key = format!("saves.error.{profile}");
    if let Some(error) = error {
        b.database()?.put_setting(&key, &error.1)?;
    } else {
        b.database()?.delete_setting(&key)?;
    }
    Ok(())
}
fn emit_backup(app: Option<&tauri::AppHandle>, request: &str, snapshot: &SaveSnapshot) {
    use tauri::Emitter;
    if let Some(app) = app {
        let _ = app.emit(
            "save:backup-created",
            crate::domain::EventEnvelope {
                request_id: request.to_owned(),
                occurred_at: now(),
                payload: snapshot,
            },
        );
    }
}
fn snapshot_locked(
    b: &Backend,
    p: &SaveProfile,
    reason: &str,
    label: Option<String>,
    note: Option<String>,
) -> Result<SaveSnapshot> {
    let source = checked_source(b, p)?;
    let current = manifest(&source)?;
    let mut snapshot = SaveSnapshot {
        id: id(),
        save_profile_id: p.id.clone(),
        game_id: p.game_id.clone(),
        install_id: p.install_id.clone(),
        created_at: now(),
        size_bytes: 0,
        sha256: String::new(),
        label,
        note,
        file_count: current.len() as u64,
        creation_reason: reason.into(),
    };
    let destination = archive_path(b, &snapshot)?;
    make_directory(destination.parent().unwrap())?;
    let temporary = destination.with_extension("partial");
    let write = (|| {
        write_archive(&source, &temporary, &current)?;
        let stamp = digest(&temporary, MAX_TOTAL + 16 * 1024 * 1024)?;
        if read_archive(&temporary, &stamp.sha256, None)? != current {
            return Err(conflict("快照内容与存档不一致，备份未保存。"));
        }
        snapshot.size_bytes = stamp.size_bytes;
        snapshot.sha256 = stamp.sha256;
        rename(&temporary, &destination)?;
        b.database()?.store_save_snapshot(&snapshot)?;
        record_error(b, &p.id, None)?;
        Ok(())
    })();
    if let Err(error) = write {
        let _ = fs::remove_file(&temporary);
        // Never remove a registered snapshot, even if a later status write failed.
        if b.database()?.save_snapshot(&snapshot.id).is_err() {
            let _ = fs::remove_file(&destination);
        }
        return Err(error);
    }
    Ok(snapshot)
}
impl Backend {
    fn save_operation<T>(
        &self,
        operation: impl FnOnce(&mut SaveManager) -> Result<T>,
    ) -> Result<T> {
        let _launch = self
            .launch_lock
            .lock()
            .map_err(|_| conflict("启动服务需要重新启动。"))?;
        let mut saves = self
            .saves
            .lock()
            .map_err(|_| conflict("存档服务需要重新启动。"))?;
        unjournaled(self)?;
        operation(&mut saves)
    }
    pub fn detect_save_paths(&self, install_id: &str) -> Result<Vec<String>> {
        let installation = self.database()?.installation(install_id)?;
        let root = super::absolute_directory(&installation.absolute_path)?;
        let mut paths = vec![];
        for name in ["Save", "save"] {
            let candidate = root.join(name);
            if candidate.exists() {
                let directory = directory(&path_text(&candidate)?)?;
                let text = path_text(&directory)?;
                if !paths.contains(&text) {
                    paths.push(text);
                }
            }
        }
        Ok(paths)
    }
    pub fn list_save_profiles(&self, game_id: &str) -> Result<Vec<SaveProfile>> {
        let mut profiles = self.database()?.save_profiles(game_id)?;
        for profile in &mut profiles {
            profile.source_available = checked_source(self, profile).is_ok();
        }
        Ok(profiles)
    }
    pub fn configure_save_profile(&self, q: ConfigureProfile) -> Result<SaveProfile> {
        self.save_operation(|manager| {
            let installation = self.database()?.installation(&q.install_id)?;
            idle(self, &installation.game_id)?;
            if !(1..=1000).contains(&q.retention_count) {
                return Err(super::invalid("建议保留数量需要在 1～1000 之间。"));
            }
            let existing =
                q.id.as_ref()
                    .map(|id| self.database()?.save_profile(id))
                    .transpose()?;
            if existing
                .as_ref()
                .is_some_and(|p| p.install_id != q.install_id)
            {
                return Err(conflict("存档配置不能转移至另一安装版本。"));
            }
            let mut profile = SaveProfile {
                id: existing.as_ref().map(|p| p.id.clone()).unwrap_or_else(id),
                game_id: installation.game_id,
                install_id: q.install_id,
                source_path: q.source_path,
                backup_before_launch: q.backup_before_launch,
                backup_after_exit: q.backup_after_exit,
                retention_count: q.retention_count,
                source_available: true,
                last_error: None,
            };
            profile.source_path = path_text(&checked_source(self, &profile)?)?;
            if self
                .database()?
                .save_profiles(&profile.game_id)?
                .iter()
                .any(|p| {
                    p.id != profile.id
                        && p.install_id == profile.install_id
                        && p.source_path == profile.source_path
                })
            {
                return Err(conflict("此安装版本已保存该存档目录，请选择已有配置。"));
            }
            if existing
                .as_ref()
                .is_some_and(|p| p.source_path != profile.source_path)
                && self
                    .database()?
                    .save_snapshots(&profile.game_id)?
                    .iter()
                    .any(|s| s.save_profile_id == profile.id)
            {
                return Err(conflict("已有快照的配置不能改为另一存档目录，请新建配置。"));
            }
            self.database()?.store_save_profile(&profile)?;
            record_error(self, &profile.id, None)?;
            manager.previews.retain(|_, v| v.profile.id != profile.id);
            Ok(profile)
        })
    }
    pub fn create_save_snapshot(
        &self,
        q: CreateSnapshot,
        request: &str,
        app: Option<&tauri::AppHandle>,
    ) -> Result<SaveSnapshot> {
        self.save_operation(|_| {
            let p = self.database()?.save_profile(&q.profile_id)?;
            idle(self, &p.game_id)?;
            if q.label.as_ref().is_some_and(|s| s.chars().count() > 80)
                || q.note.as_ref().is_some_and(|s| s.chars().count() > 1000)
            {
                return Err(super::invalid("快照名称或备注过长。"));
            }
            let result = snapshot_locked(self, &p, "manual", q.label, q.note);
            record_error(self, &p.id, result.as_ref().err())?;
            if let Ok(snapshot) = &result {
                emit_backup(app, request, snapshot);
            }
            result
        })
    }
    pub fn delete_save_profile(&self, q: DeleteProfile) -> Result<bool> {
        if !q.confirmed {
            return Err(super::invalid("删除存档配置需要再次确认。"));
        }
        self.save_operation(|manager| {
            let profile = self.database()?.save_profile(&q.profile_id)?;
            idle(self, &profile.game_id)?;
            self.database()?.forget_save_profile(&profile.id)?;
            manager.previews.retain(|_, v| v.profile.id != profile.id);
            Ok(true)
        })
    }
    pub fn preview_save_restore(&self, id: &str) -> Result<RestorePreview> {
        self.save_operation(|manager| {
            let snapshot = self.database()?.save_snapshot(id)?;
            let profile = self.database()?.save_profile(&snapshot.save_profile_id)?;
            idle(self, &profile.game_id)?;
            let source = checked_source(self, &profile)?;
            let current = manifest(&source)?;
            let archive = read_archive(&archive_path(self, &snapshot)?, &snapshot.sha256, None)?;
            let mut changes = vec![];
            let mut added = 0;
            let mut modified = 0;
            for (path, stamp) in &archive {
                let change = match current.get(path) {
                    None => {
                        added += 1;
                        "added"
                    }
                    Some(value) if value != stamp => {
                        modified += 1;
                        "modified"
                    }
                    _ => continue,
                };
                if changes.len() < 200 {
                    changes.push(FileChange {
                        path: path.clone(),
                        change: change.into(),
                    });
                }
            }
            let mut deleted = 0;
            for path in current.keys().filter(|path| !archive.contains_key(*path)) {
                deleted += 1;
                if changes.len() < 200 {
                    changes.push(FileChange {
                        path: path.clone(),
                        change: "deleted".into(),
                    });
                }
            }
            let preview = RestorePreview {
                preview_id: super::id(),
                confirmation_token: super::id(),
                snapshot_id: snapshot.id,
                source_path: profile.source_path.clone(),
                expires_at: (chrono::Utc::now() + chrono::Duration::minutes(5)).to_rfc3339(),
                added_files: added,
                modified_files: modified,
                deleted_files: deleted,
                preserved_files: 0,
                truncated: added + modified + deleted > changes.len() as u64,
                changes,
            };
            manager.previews.retain(|_, v| v.expires > Instant::now());
            if manager.previews.len() >= 32 {
                manager.previews.clear();
            }
            manager.previews.insert(
                preview.preview_id.clone(),
                PendingPreview {
                    preview: preview.clone(),
                    profile,
                    current,
                    archive,
                    expires: Instant::now() + Duration::from_secs(300),
                },
            );
            Ok(preview)
        })
    }
    pub fn restore_save_snapshot(
        &self,
        q: RestoreSaveSnapshotRequest,
        request: &str,
        app: Option<&tauri::AppHandle>,
    ) -> Result<RestoreResult> {
        self.save_operation(|manager| {
            let pending = manager
                .previews
                .remove(&q.preview_id)
                .ok_or_else(|| conflict("恢复预览已失效，请重新预览。"))?;
            if pending.expires < Instant::now()
                || pending.preview.confirmation_token != q.confirmation_token
                || pending.preview.snapshot_id != q.snapshot_id
            {
                return Err(conflict("恢复确认与预览不匹配，请重新预览。"));
            }
            let snapshot = self.database()?.save_snapshot(&q.snapshot_id)?;
            let profile = self.database()?.save_profile(&snapshot.save_profile_id)?;
            idle(self, &profile.game_id)?;
            let source = checked_source(self, &profile)?;
            if profile.source_path != pending.profile.source_path
                || profile.id != pending.profile.id
                || manifest(&source)? != pending.current
            {
                return Err(conflict("存档或配置在预览后发生变化，请重新预览。"));
            }
            let archive_path = archive_path(self, &snapshot)?;
            if read_archive(&archive_path, &snapshot.sha256, None)? != pending.archive {
                return Err(conflict("快照已变化，请重新预览。"));
            }
            let safety = snapshot_locked(
                self,
                &profile,
                "safety_before_restore",
                Some("恢复前安全备份".into()),
                None,
            )?;
            emit_backup(app, request, &safety);
            let transaction = RestoreJournal {
                profile_id: profile.id.clone(),
                transaction_id: id(),
            };
            let (stage, original) = transaction_paths(&source, &transaction)?;
            fs::create_dir(&stage).map_err(|_| conflict("无法创建恢复临时目录，原存档未修改。"))?;
            let prepare = (|| {
                read_archive(&archive_path, &snapshot.sha256, Some(&stage))?;
                if manifest(&stage)? != pending.archive {
                    return Err(conflict(
                        "恢复临时目录校验失败，可能存在文件名冲突；原存档未修改。",
                    ));
                }
                if manifest(&source)? != pending.current {
                    return Err(conflict("存档在恢复准备阶段发生变化，原存档未修改。"));
                }
                {
                    let db = self.database()?;
                    db.ensure_recovery_idle_except(None)?;
                    db.put_setting(JOURNAL, &transaction)?;
                }
                ancestors(&source)?;
                if let Err(error) = swap_directories(&source, &stage, &original) {
                    if source.exists() && !original.exists() {
                        self.database()?.delete_setting(JOURNAL)?;
                    }
                    return Err(error);
                }
                if let Err(error) = self.database()?.delete_setting(JOURNAL) {
                    rename(&source, &stage)?;
                    rename(&original, &source)?;
                    return Err(error);
                }
                Ok(())
            })();
            if let Err(error) = prepare {
                // A pending journal owns every recovery path: never clean it prematurely.
                if self
                    .database()?
                    .setting::<RestoreJournal>(JOURNAL)?
                    .is_none()
                {
                    let _ = fs::remove_dir_all(&stage);
                }
                record_error(self, &profile.id, Some(&error))?;
                return Err(error);
            }
            // Current source is already restored and the verified safety ZIP is retained.
            let _ = fs::remove_dir_all(&original);
            record_error(self, &profile.id, None)?;
            Ok(RestoreResult {
                snapshot_id: snapshot.id,
                safety_backup_id: safety.id,
                added_files: pending.preview.added_files,
                modified_files: pending.preview.modified_files,
                deleted_files: pending.preview.deleted_files,
            })
        })
    }
    pub fn delete_save_snapshot(&self, q: DeleteSnapshot) -> Result<bool> {
        if !q.confirmed {
            return Err(super::invalid("删除快照需要再次确认。"));
        }
        self.save_operation(|manager| {
            let snapshot = self.database()?.save_snapshot(&q.snapshot_id)?;
            let path = archive_path(self, &snapshot)?;
            ancestors(&path)?;
            let trash = self.data_directory.join("save-backups").join(".trash");
            make_directory(&trash)?;
            let target = trash.join(format!("{}.zip", snapshot.id));
            if target.exists() {
                return Err(conflict("回收目录中已有同名快照，删除已停止。"));
            }
            rename(&path, &target)?;
            if let Err(error) = self.database()?.forget_save_snapshot(&snapshot.id) {
                rename(&target, &path)?;
                return Err(error);
            }
            manager
                .previews
                .retain(|_, v| v.preview.snapshot_id != snapshot.id);
            Ok(true)
        })
    }
    /// Caller holds launch_lock; do not re-enter save_operation.
    pub fn automatic_save_backup(
        &self,
        install_id: &str,
        before: bool,
        request: &str,
        app: Option<&tauri::AppHandle>,
    ) -> Result<()> {
        let _saves = self
            .saves
            .lock()
            .map_err(|_| conflict("存档服务需要重新启动。"))?;
        unjournaled(self)?;
        let install = self.database()?.installation(install_id)?;
        let profiles = self.database()?.save_profiles(&install.game_id)?;
        let mut first_error = None;
        for profile in profiles.into_iter().filter(|p| {
            p.install_id == install_id
                && if before {
                    p.backup_before_launch
                } else {
                    p.backup_after_exit
                }
        }) {
            let result = snapshot_locked(
                self,
                &profile,
                if before {
                    "before_launch"
                } else {
                    "after_exit"
                },
                None,
                None,
            );
            record_error(self, &profile.id, result.as_ref().err())?;
            if let Ok(snapshot) = &result {
                emit_backup(app, request, snapshot);
            }
            if let Err(error) = result {
                first_error.get_or_insert(error);
            }
        }
        first_error.map_or(Ok(()), Err)
    }
    pub fn recover_save_restore(&self) -> Result<()> {
        let journal: Option<RestoreJournal> = self.database()?.setting(JOURNAL)?;
        let Some(journal) = journal else {
            return Ok(());
        };
        let profile = self.database()?.save_profile(&journal.profile_id)?;
        let source = Path::new(&profile.source_path);
        ancestors(
            source
                .parent()
                .ok_or_else(|| conflict("恢复日志中的目录无效。"))?,
        )?;
        let (stage, original) = transaction_paths(source, &journal)?;
        if original.exists() {
            ancestors(&original)?;
            if source.exists() {
                ancestors(source)?;
                if stage.exists() {
                    return Err(conflict(
                        "恢复副本冲突，已保留全部文件，请手动检查存档目录。",
                    ));
                }
                rename(source, &stage)?;
            }
            rename(&original, source)?;
        }
        // Preserve staged files for inspection, including any changes made after a crash.
        self.database()?.delete_setting(JOURNAL)?;
        record_error(
            self,
            &profile.id,
            Some(&conflict(
                "上次恢复被中断，已回滚原存档；临时副本保留在存档目录旁。",
            )),
        )?;
        Ok(())
    }
    pub fn skipped_exit_save_backup(&self, install_id: &str) -> Result<Option<String>> {
        let install = self.database()?.installation(install_id)?;
        let profiles = self.database()?.save_profiles(&install.game_id)?;
        let error = conflict("进程监控中断，未执行退出后备份；关闭游戏后可手动备份。");
        let mut skipped = false;
        for profile in profiles
            .into_iter()
            .filter(|p| p.install_id == install_id && p.backup_after_exit)
        {
            record_error(self, &profile.id, Some(&error))?;
            skipped = true;
        }
        Ok(skipped.then(|| error.1.to_owned()))
    }
}
fn swap_directories(source: &Path, stage: &Path, original: &Path) -> Result<()> {
    if original.exists() {
        return Err(conflict("恢复安全目录发生冲突，原存档未修改。"));
    }
    rename(source, original)?;
    if let Err(error) = rename(stage, source) {
        rename(original, source)?;
        return Err(error);
    }
    Ok(())
}
fn transaction_paths(source: &Path, journal: &RestoreJournal) -> Result<(PathBuf, PathBuf)> {
    safe_id(&journal.transaction_id)?;
    let parent = source.parent().ok_or_else(|| conflict("存档目录无效。"))?;
    Ok((
        parent.join(format!(".gm-restore-{}", journal.transaction_id)),
        parent.join(format!(".gm-original-{}", journal.transaction_id)),
    ))
}

#[cfg(test)]
mod tests;
