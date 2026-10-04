use super::*;
use crate::domain::protocol::InstallSource;
struct Fixture {
    root: PathBuf,
    backend: Option<Backend>,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("gm-database-fixture-{}", id()));
        fs::create_dir_all(&root).unwrap();
        let root = root.canonicalize().unwrap();
        let backend = Backend::open(root.join("data")).unwrap();
        Self {
            root,
            backend: Some(backend),
        }
    }
    fn b(&self) -> &Backend {
        self.backend.as_ref().unwrap()
    }
    fn add(&self, title: &str) -> String {
        self.b()
            .database()
            .unwrap()
            .import_installation(
                self.root.join(title).to_str().unwrap(),
                title,
                None,
                InstallSource::Manual,
                &[],
                "fixture",
            )
            .unwrap()
    }
    fn export(&self) -> ExportResult {
        let directory = self.root.join("备份 目录");
        fs::create_dir_all(&directory).unwrap();
        self.b()
            .export_database(ExportRequest {
                directory: path_text(&directory).unwrap(),
            })
            .unwrap()
    }
    fn preview(&self, path: &str) -> ImportPreview {
        self.b()
            .preview_database_import(PreviewRequest { path: path.into() })
            .unwrap()
    }
    fn confirm(&self, p: &ImportPreview) {
        self.b()
            .confirm_database_import(ConfirmRequest {
                confirmation_token: p.confirmation_token.clone(),
                confirmed: true,
            })
            .unwrap();
    }
    fn restart(&mut self) {
        drop(self.backend.take());
        self.backend = Some(Backend::open(self.root.join("data")).unwrap());
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        drop(self.backend.take());
        let _ = fs::remove_dir_all(&self.root);
    }
}
#[test]
fn export_is_consistent_and_import_waits_for_restart_with_safety_backup() {
    let mut f = Fixture::new();
    let first = f.add("作品 含空格");
    let export = f.export();
    let exported_bytes = fs::read(&export.path).unwrap();
    assert_eq!(export.sha256, hash(&exported_bytes));
    assert_eq!(export.size_bytes, exported_bytes.len() as u64);
    let second = f.add("新增作品");
    fs::write(f.root.join("original-save.dat"), b"never modify").unwrap();
    let preview = f.preview(&export.path);
    assert_eq!(preview.counts.games, 1);
    assert_eq!(preview.counts.installations, 1);
    f.confirm(&preview);
    assert!(f.b().database().unwrap().get_game(&second).is_ok());
    f.restart();
    assert!(f.b().database().unwrap().get_game(&first).is_ok());
    assert!(f.b().database().unwrap().get_game(&second).is_err());
    assert!(!f.b().database_transfer_status().unwrap().pending_restart);
    let backup = fs::read_dir(f.root.join("data/database-backups"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    assert!(Database::verified_snapshot(backup.as_path())
        .unwrap()
        .get_game(&second)
        .is_ok());
    assert_eq!(
        fs::read(f.root.join("original-save.dat")).unwrap(),
        b"never modify"
    );
    assert_eq!(fs::read(&export.path).unwrap(), exported_bytes);
}
#[test]
fn import_rejects_missing_wrong_expired_confirmation_and_changed_copy() {
    let f = Fixture::new();
    let export = f.export();
    let p = f.preview(&export.path);
    for request in [
        ConfirmRequest {
            confirmation_token: p.confirmation_token.clone(),
            confirmed: false,
        },
        ConfirmRequest {
            confirmation_token: "wrong".into(),
            confirmed: true,
        },
    ] {
        assert!(f.b().confirm_database_import(request).is_err());
    }
    f.b()
        .transfers
        .lock()
        .unwrap()
        .preview
        .as_mut()
        .unwrap()
        .deadline = Instant::now() - Duration::from_secs(1);
    assert!(f
        .b()
        .confirm_database_import(ConfirmRequest {
            confirmation_token: p.confirmation_token,
            confirmed: true
        })
        .is_err());
    let p = f.preview(&export.path);
    let name = f
        .b()
        .transfers
        .lock()
        .unwrap()
        .preview
        .as_ref()
        .unwrap()
        .pending
        .file_name
        .clone();
    fs::write(f.root.join("data/database-imports").join(name), b"changed").unwrap();
    assert!(f
        .b()
        .confirm_database_import(ConfirmRequest {
            confirmation_token: p.confirmation_token,
            confirmed: true
        })
        .is_err());
    assert!(!f.b().database_transfer_status().unwrap().pending_restart);
}
#[test]
fn damaged_future_foreign_key_and_altered_schema_snapshots_are_refused() {
    let f = Fixture::new();
    f.add("original");
    for sql in [
        "UPDATE schema_migrations SET version=999",
        "CREATE TRIGGER unexpected AFTER INSERT ON games BEGIN DELETE FROM notes; END",
        "PRAGMA foreign_keys=OFF; INSERT INTO tags(id,name) VALUES('tag','orphan'); INSERT INTO game_tags(id,game_id,tag_id) VALUES('rel','missing','tag')",
    ] {
        let export = f.export();
        let source = rusqlite::Connection::open(&export.path).unwrap();
        source.execute_batch(sql).unwrap(); drop(source);
        assert!(f.b().preview_database_import(PreviewRequest { path: export.path }).is_err());
    }
    let bad = f.root.join("bad.sqlite3");
    fs::write(&bad, b"SQLite format 3\0damaged").unwrap();
    assert!(f
        .b()
        .preview_database_import(PreviewRequest {
            path: path_text(&bad).unwrap()
        })
        .is_err());
    assert_eq!(
        f.b().database().unwrap().transfer_counts().unwrap().games,
        1
    );
    assert_eq!(
        fs::read_dir(f.root.join("data/database-imports"))
            .unwrap()
            .count(),
        0
    );
}
#[test]
fn cancellation_and_failed_startup_keep_original_library_usable() {
    let mut f = Fixture::new();
    let export = f.export();
    let game = f.add("original");
    let p = f.preview(&export.path);
    f.confirm(&p);
    f.b().cancel_database_import().unwrap();
    f.restart();
    assert!(f.b().database().unwrap().get_game(&game).is_ok());
    let p = f.preview(&export.path);
    f.confirm(&p);
    // A safety snapshot that cannot be created must prevent replacement.
    fs::write(f.root.join("data/database-backups"), b"blocked").unwrap();
    f.restart();
    let status = f.b().database_transfer_status().unwrap();
    assert!(!status.pending_restart);
    assert!(status.last_import_error.is_some());
    assert!(f.b().database().unwrap().get_game(&game).is_ok());
}
#[test]
fn imported_recovery_jobs_are_discarded_and_corrupt_pending_copy_is_not_applied() {
    let mut f = Fixture::new();
    f.b()
        .database()
        .unwrap()
        .put_setting(
            "saves.restore.pending",
            &serde_json::json!({"unsafe":"fixture"}),
        )
        .unwrap();
    let export = f.export();
    f.b()
        .database()
        .unwrap()
        .delete_setting("saves.restore.pending")
        .unwrap();
    let game = f.add("original");
    let p = f.preview(&export.path);
    let pending = f
        .b()
        .transfers
        .lock()
        .unwrap()
        .preview
        .as_ref()
        .unwrap()
        .pending
        .clone();
    let staged = checked_path(&f.b().data_directory, &pending).unwrap();
    let candidate = Database::verified_snapshot(&staged).unwrap();
    assert!(candidate
        .setting::<serde_json::Value>("saves.restore.pending")
        .unwrap()
        .is_none());
    drop(candidate);
    f.confirm(&p);
    fs::write(staged, b"corrupt after confirmation").unwrap();
    f.restart();
    assert!(f.b().database().unwrap().get_game(&game).is_ok());
    assert!(f
        .b()
        .database_transfer_status()
        .unwrap()
        .last_import_error
        .is_some());
}
#[test]
fn pending_local_save_restore_prevents_database_replacement() {
    let f = Fixture::new();
    let export = f.export();
    let game = f.add("original");
    let preview = f.preview(&export.path);
    f.confirm(&preview);
    let mut db = f.b().database().unwrap();
    db.put_setting(
        "saves.restore.pending",
        &serde_json::json!({"local":"must recover first"}),
    )
    .unwrap();
    apply_pending(&mut db, &f.b().data_directory).unwrap();
    assert!(db.get_game(&game).is_ok());
    assert!(db
        .setting::<serde_json::Value>("saves.restore.pending")
        .unwrap()
        .is_some());
    assert!(db.setting::<Pending>(PENDING).unwrap().is_none());
}
#[cfg(unix)]
#[test]
fn snapshot_and_internal_directory_links_are_rejected() {
    let f = Fixture::new();
    let export = f.export();
    let link = f.root.join("linked.sqlite3");
    std::os::unix::fs::symlink(&export.path, &link).unwrap();
    assert!(f
        .b()
        .preview_database_import(PreviewRequest {
            path: path_text(&link).unwrap()
        })
        .is_err());
    let external = f.root.join("outside");
    fs::create_dir(&external).unwrap();
    std::os::unix::fs::symlink(&external, f.root.join("data/database-imports")).unwrap();
    assert!(f
        .b()
        .preview_database_import(PreviewRequest { path: export.path })
        .is_err());
    assert_eq!(fs::read_dir(external).unwrap().count(), 0);
}
#[test]
fn oversized_and_live_wal_databases_are_rejected_without_allocating_or_copying() {
    let f = Fixture::new();
    let large = f.root.join("large.sqlite3");
    fs::File::create(&large)
        .unwrap()
        .set_len(MAX_BYTES + 1)
        .unwrap();
    assert!(f
        .b()
        .preview_database_import(PreviewRequest {
            path: path_text(&large).unwrap()
        })
        .is_err());
    let export = f.export();
    fs::write(format!("{}-wal", export.path), b"live wal").unwrap();
    assert!(f
        .b()
        .preview_database_import(PreviewRequest { path: export.path })
        .is_err());
    assert!(!f.root.join("data/database-imports").exists());
}

#[test]
fn import_preserves_existing_recovery_tasks() {
    for key in [
        "saves.restore.pending",
        "backup.restore.pending",
        "backup.restore.journal",
        "database.import.pending",
    ] {
        let f = Fixture::new();
        let game = f.add("Current game");
        let export = f.export();
        let p = f.preview(&export.path);
        f.b().database().unwrap().put_setting(key, &true).unwrap();
        assert!(f
            .b()
            .confirm_database_import(ConfirmRequest {
                confirmation_token: p.confirmation_token,
                confirmed: true
            })
            .is_err());
        assert!(f.b().database().unwrap().get_game(&game).is_ok());
        assert_eq!(
            f.b().database().unwrap().setting::<bool>(key).unwrap(),
            Some(true)
        );
    }
    let f = Fixture::new();
    f.add("Old");
    let export = f.export();
    let p = f.preview(&export.path);
    f.confirm(&p);
    let new_game = f.add("Current");
    let mut db = f.b().database().unwrap();
    db.put_setting("backup.restore.pending", &true).unwrap();
    apply_pending(&mut db, &f.b().data_directory).unwrap();
    assert!(db.get_game(&new_game).is_ok());
    assert_eq!(
        db.setting::<bool>("backup.restore.pending").unwrap(),
        Some(true)
    );
    assert!(db.setting::<Pending>(PENDING).unwrap().is_none());
}

#[test]
fn snapshot_rejects_invalid_preferences_before_scheduling_import() {
    let f = Fixture::new();
    let current = f.add("Current");
    let export = f.export();
    {
        let db = Database::open(&export.path).unwrap();
        db.put_setting(
            "app.preferences",
            &serde_json::json!({"ui_refresh_seconds": 0}),
        )
        .unwrap();
    }
    assert!(f
        .b()
        .preview_database_import(PreviewRequest { path: export.path })
        .is_err());
    assert!(f.b().database().unwrap().get_game(&current).is_ok());
    assert!(!f.b().database_transfer_status().unwrap().pending_restart);
    let compatible = f.export();
    {
        let db = Database::open(&compatible.path).unwrap();
        db.put_setting("app.preferences", &serde_json::json!({}))
            .unwrap();
    }
    assert!(Database::verified_snapshot(Path::new(&compatible.path)).is_ok());
}

#[test]
fn snapshot_rejects_malformed_typed_settings_and_unreferenced_smart_rules() {
    let f = Fixture::new();
    for (key, value) in [
        (
            "app.preferences",
            serde_json::json!({"ui_refresh_seconds": "bad"}),
        ),
        ("metadata.locked.fixture", serde_json::json!(1)),
        ("metadata.priority.fixture", serde_json::json!(true)),
        (
            "metadata.hikarinagi",
            serde_json::json!({"method": "unknown"}),
        ),
        ("launch.tools.fixture", serde_json::json!([true, "bad"])),
    ] {
        let export = f.export();
        {
            let db = Database::open(&export.path).unwrap();
            db.put_setting(key, &value).unwrap();
        }
        assert!(
            Database::verified_snapshot(Path::new(&export.path)).is_err(),
            "{key}"
        );
    }
    let export = f.export();
    let connection = rusqlite::Connection::open(&export.path).unwrap();
    connection
        .execute(
            "INSERT INTO smart_filters(id,name,query_json) VALUES('invalid','Invalid','{}')",
            [],
        )
        .unwrap();
    drop(connection);
    assert!(Database::verified_snapshot(Path::new(&export.path)).is_err());
}
