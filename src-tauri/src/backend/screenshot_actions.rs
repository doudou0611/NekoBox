//! Original-image actions: native user-selected destinations and backed-up deletion.
use super::*;
use serde::{Deserialize, Serialize};
use std::{fs, io::Write};
#[derive(Deserialize)]
pub struct Request {
    pub game_id: String,
    pub screenshot_ids: Vec<String>,
    #[serde(default)]
    pub confirmed: bool,
    #[serde(default)]
    pub batch: bool,
}
#[derive(Default, Serialize)]
pub struct Report {
    pub cancelled: bool,
    pub completed: usize,
    pub failures: Vec<String>,
}
pub(crate) fn restored_original(b: &Backend, path: &Path) -> bool {
    path.parent() == Some(b.data_directory.join("backup-screenshots").as_path())
        && path
            .file_stem()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.len() == 64 && n.bytes().all(|c| c.is_ascii_hexdigit()))
}
fn originals(b: &Backend, q: &Request) -> Result<Vec<(String, PathBuf)>> {
    if q.screenshot_ids.is_empty() || q.screenshot_ids.len() > 1000 {
        return Err(invalid("请先选择截图，最多 1000 张。"));
    }
    let detail = b.database()?.get_game(&q.game_id)?;
    let roots = detail
        .summary
        .installations
        .iter()
        .filter_map(|i| absolute_directory(&i.absolute_path).ok())
        .collect::<Vec<_>>();
    let mut result = vec![];
    let mut ids = std::collections::HashSet::new();
    for id in &q.screenshot_ids {
        if !ids.insert(id) {
            return Err(invalid("截图选择包含重复项。"));
        }
        let (raw, game) = b.database()?.screenshot_path(id)?.ok_or_else(missing)?;
        let path = Path::new(&raw);
        if game != q.game_id
            || path
                .ancestors()
                .any(|p| fs::symlink_metadata(p).is_ok_and(|m| scanner::linked(&m)))
        {
            return Err(permission("截图不属于该作品或路径包含链接，已保留原图。"));
        }
        let path = path.canonicalize().map_err(|_| missing())?;
        if (!roots.iter().any(|root| path.starts_with(root)) && !restored_original(b, &path))
            || !path.is_file()
        {
            return Err(permission("截图必须位于该作品已登记的安装目录内。"));
        }
        if !matches!(
            path.extension()
                .and_then(|s| s.to_str())
                .map(str::to_ascii_lowercase)
                .as_deref(),
            Some("jpg" | "jpeg" | "png" | "gif" | "webp")
        ) {
            return Err(invalid("仅支持图片导出与删除。"));
        }
        result.push((id.clone(), path));
    }
    Ok(result)
}
fn copy_unique(source: &Path, destination: &Path) -> Result<()> {
    // Bound memory and re-check path components before opening the original.
    if source
        .ancestors()
        .any(|p| fs::symlink_metadata(p).is_ok_and(|m| scanner::linked(&m)))
    {
        return Err(permission("图片路径发生变化，请重新扫描。"));
    }
    let bytes = thumbnails::read_bounded(source, 64 * 1024 * 1024)?;
    let parent = destination
        .parent()
        .ok_or_else(|| invalid("保存目录无效。"))?;
    let parent = absolute_directory(&path_text(parent)?)?;
    let name = destination
        .file_name()
        .ok_or_else(|| invalid("保存名称无效。"))?;
    let mut target = parent.join(name);
    for index in 0..10_000 {
        if index > 0 {
            let stem = destination
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy();
            let ext = destination
                .extension()
                .unwrap_or_default()
                .to_string_lossy();
            target = parent.join(format!("{stem} ({index}).{ext}"));
        }
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&target)
        {
            Ok(mut file) => {
                if file
                    .write_all(&bytes)
                    .and_then(|_| file.sync_all())
                    .is_err()
                {
                    let _ = fs::remove_file(&target);
                    return Err(permission("图片保存失败，未报告成功。"));
                }
                return Ok(());
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(_) => return Err(permission("无法在所选目录保存图片。")),
        }
    }
    Err(invalid("同名图片过多，请选择其他目录。"))
}
pub fn export(b: &Backend, q: Request, app: &tauri::AppHandle) -> Result<Report> {
    use tauri::Manager;
    use tauri_plugin_dialog::DialogExt;
    let _guard = b
        .screenshot_lock
        .lock()
        .map_err(|_| invalid("截图服务需要重新启动。"))?;
    let files = originals(b, &q)?;
    let mut dialog = app.dialog().file();
    if let Some(window) = app.get_webview_window("main") {
        dialog = dialog.set_parent(&window);
    }
    let selected = if q.batch {
        dialog.set_title("选择截图保存目录").blocking_pick_folder()
    } else {
        if files.len() != 1 {
            return Err(invalid("单张保存仅能选择一张图片。"));
        }
        dialog
            .set_title("保存截图原图")
            .set_file_name(files[0].1.file_name().unwrap_or_default().to_string_lossy())
            .blocking_save_file()
    };
    let Some(target) = selected else {
        return Ok(Report {
            cancelled: true,
            ..Default::default()
        });
    };
    let target = target
        .into_path()
        .map_err(|_| invalid("保存位置不是本地路径。"))?;
    export_to(&files, &target, q.batch)
}
fn export_to(files: &[(String, PathBuf)], target: &Path, batch: bool) -> Result<Report> {
    if !target.is_absolute() {
        return Err(invalid("保存位置必须是完整路径。"));
    }
    if batch {
        absolute_directory(&path_text(target)?)?;
    }
    let mut report = Report::default();
    for (_, source) in files {
        let dest = if batch {
            target.join(source.file_name().unwrap_or_default())
        } else {
            target.to_owned()
        };
        match copy_unique(source, &dest) {
            Ok(()) => report.completed += 1,
            Err(e) => report.failures.push(e.1.into()),
        }
    }
    Ok(report)
}
pub fn delete(b: &Backend, q: Request) -> Result<Report> {
    if !q.confirmed {
        return Err(invalid("删除原图前必须确认。"));
    }
    let _guard = b
        .screenshot_lock
        .lock()
        .map_err(|_| invalid("截图服务需要重新启动。"))?;
    let files = originals(b, &q)?;
    let backup = b.data_directory.join("screenshot-backups").join(id());
    fs::create_dir_all(&backup).map_err(|_| permission("无法备份原图，已取消删除。"))?;
    b.database()?.snapshot_to(&backup.join("records.sqlite3"))?;
    for (id, source) in &files {
        let copy = backup.join(format!(
            "{id}.{}",
            source.extension().unwrap_or_default().to_string_lossy()
        ));
        copy_unique(source, &copy)?;
    }
    let manifest = serde_json::to_vec(
        &files
            .iter()
            .map(|(id, p)| (id, path_text(p).unwrap_or_default()))
            .collect::<Vec<_>>(),
    )
    .map_err(|_| invalid("备份记录无效。"))?;
    fs::write(backup.join("originals.json"), manifest)
        .map_err(|_| permission("无法记录备份位置，已取消删除。"))?;
    let mut report = Report::default();
    for (id, source) in files {
        if source
            .ancestors()
            .any(|p| fs::symlink_metadata(p).is_ok_and(|m| scanner::linked(&m)))
        {
            report
                .failures
                .push("图片路径发生变化，已保留原图。".into());
            continue;
        }
        let saved = backup.join(format!(
            "{id}.{}",
            source.extension().unwrap_or_default().to_string_lossy()
        ));
        if thumbnails::read_bounded(&source, 64 * 1024 * 1024).ok()
            != thumbnails::read_bounded(&saved, 64 * 1024 * 1024).ok()
        {
            report
                .failures
                .push("原图在备份后发生变化，已保留原图。".into());
            continue;
        }
        if restored_original(b, &source)
            && b.database()?
                .screenshot_has_other_references(&path_text(&source)?, &id)?
        {
            match b.database()?.delete_screenshot_record(&id, &q.game_id) {
                Ok(()) => report.completed += 1,
                Err(e) => report.failures.push(e.1.into()),
            }
            continue;
        }
        match fs::remove_file(&source) {
            Ok(()) => match b.database()?.delete_screenshot_record(&id, &q.game_id) {
                Ok(()) => report.completed += 1,
                Err(e) => {
                    let saved = backup.join(format!(
                        "{id}.{}",
                        source.extension().unwrap_or_default().to_string_lossy()
                    ));
                    let _ = copy_unique(&saved, &source);
                    report.failures.push(e.1.into());
                }
            },
            Err(_) => report.failures.push("原图无法删除，记录已保留。".into()),
        }
    }
    Ok(report)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn original_export_renames_collisions_and_keeps_bytes() {
        let root = std::env::temp_dir().join(id());
        fs::create_dir_all(&root).unwrap();
        let original = root.join("original.png");
        fs::write(&original, b"original image bytes").unwrap();
        let target = root.join("saved.png");
        fs::write(&target, b"existing").unwrap();
        let report = export_to(
            &[("1".into(), original.canonicalize().unwrap())],
            &target,
            false,
        )
        .unwrap();
        assert_eq!(report.completed, 1);
        assert_eq!(fs::read(&target).unwrap(), b"existing");
        assert_eq!(
            fs::read(root.join("saved (1).png")).unwrap(),
            b"original image bytes"
        );
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn deletion_requires_confirmation_backs_up_original_and_removes_record() {
        let root = std::env::temp_dir().join(id());
        let b = Backend::open(root.join("data")).unwrap();
        let game_dir = root.join("game");
        fs::create_dir_all(&game_dir).unwrap();
        let source = game_dir.join("shot.png");
        fs::write(&source, b"original screenshot").unwrap();
        let directory = path_text(&game_dir.canonicalize().unwrap()).unwrap();
        let game = b
            .database()
            .unwrap()
            .import_installation(
                &directory,
                "game",
                None,
                crate::domain::protocol::InstallSource::Manual,
                &[],
                "",
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
            .upsert_screenshot(
                &game,
                &install,
                &path_text(&source.canonicalize().unwrap()).unwrap(),
                None,
            )
            .unwrap();
        let screenshot = b
            .database()
            .unwrap()
            .list_screenshots(&game, true)
            .unwrap()
            .remove(0);
        assert!(delete(
            &b,
            Request {
                game_id: game.clone(),
                screenshot_ids: vec![screenshot.id.clone()],
                confirmed: false,
                batch: false
            }
        )
        .is_err());
        assert!(source.exists());
        let result = delete(
            &b,
            Request {
                game_id: game.clone(),
                screenshot_ids: vec![screenshot.id.clone()],
                confirmed: true,
                batch: false,
            },
        )
        .unwrap();
        assert_eq!(result.completed, 1);
        assert!(result.failures.is_empty());
        assert!(!source.exists());
        assert!(b
            .database()
            .unwrap()
            .list_screenshots(&game, true)
            .unwrap()
            .is_empty());
        let backup = fs::read_dir(b.data_directory.join("screenshot-backups"))
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        assert!(backup.join("records.sqlite3").is_file());
        assert!(backup.join("originals.json").is_file());
        assert_eq!(
            fs::read(backup.join(format!("{}.png", screenshot.id))).unwrap(),
            b"original screenshot"
        );
        drop(b);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn restored_originals_can_be_exported_and_shared_files_survive_one_deletion() {
        let root = std::env::temp_dir().join(id());
        let b = Backend::open(root.join("data")).unwrap();
        let cache = b.data_directory.join("backup-screenshots");
        fs::create_dir_all(&cache).unwrap();
        let source = cache.join(format!("{}.png", "a".repeat(64)));
        fs::write(&source, b"restored screenshot").unwrap();
        let mut games = vec![];
        for title in ["one", "two"] {
            let mut db = b.database().unwrap();
            let game = db
                .import_installation(
                    &path_text(&root.join(title)).unwrap(),
                    title,
                    None,
                    crate::domain::protocol::InstallSource::Manual,
                    &[],
                    "",
                )
                .unwrap();
            let install = db.get_game(&game).unwrap().summary.installations[0]
                .id
                .clone();
            db.upsert_screenshot(&game, &install, &path_text(&source).unwrap(), None)
                .unwrap();
            let shot = db.list_screenshots(&game, true).unwrap().remove(0);
            games.push((game, shot.id));
        }
        let request = |i: usize| Request {
            game_id: games[i].0.clone(),
            screenshot_ids: vec![games[i].1.clone()],
            confirmed: true,
            batch: false,
        };
        let files = originals(&b, &request(0)).unwrap();
        let target = root.join("export.png");
        assert_eq!(export_to(&files, &target, false).unwrap().completed, 1);
        assert_eq!(fs::read(target).unwrap(), b"restored screenshot");
        let unrelated = root.join(format!("{}.png", "b".repeat(64)));
        fs::write(&unrelated, b"outside").unwrap();
        assert!(!restored_original(&b, &unrelated));
        assert_eq!(delete(&b, request(0)).unwrap().completed, 1);
        assert!(source.exists());
        assert_eq!(originals(&b, &request(1)).unwrap().len(), 1);
        assert_eq!(delete(&b, request(1)).unwrap().completed, 1);
        assert!(!source.exists());
        drop(b);
        fs::remove_dir_all(root).unwrap();
    }
}
