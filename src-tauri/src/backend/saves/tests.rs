use super::*;
use crate::{
    backend::saves::files,
    domain::{models::LaunchSession, protocol::InstallSource, ErrorCode},
};
use std::io::Write;

struct Fixture {
    root: PathBuf,
    backend: Option<Backend>,
    game: String,
    install: String,
    source: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("gm-save-fixture-{}", id()));
        fs::create_dir_all(&root).unwrap();
        let root = root.canonicalize().unwrap();
        let install_path = root.join("作品含空格");
        let source = install_path.join("Save");
        fs::create_dir_all(source.join("路线")).unwrap();
        fs::write(source.join("路线/slot.dat"), b"original save").unwrap();
        fs::write(source.join("system.dat"), b"settings").unwrap();
        let backend = Backend::open(root.join("appdata")).unwrap();
        let game = backend
            .database()
            .unwrap()
            .import_installation(
                install_path.to_str().unwrap(),
                "Fixture",
                None,
                InstallSource::Manual,
                &[],
                "fixture",
            )
            .unwrap();
        let install = backend
            .database()
            .unwrap()
            .get_game(&game)
            .unwrap()
            .summary
            .installations[0]
            .id
            .clone();
        Self {
            root,
            backend: Some(backend),
            game,
            install,
            source,
        }
    }
    fn b(&self) -> &Backend {
        self.backend.as_ref().unwrap()
    }
    fn profile(&self) -> SaveProfile {
        self.b()
            .configure_save_profile(ConfigureProfile {
                id: None,
                install_id: self.install.clone(),
                source_path: path_text(&self.source).unwrap(),
                backup_before_launch: false,
                backup_after_exit: false,
                retention_count: 10,
            })
            .unwrap()
    }
    fn backup(&self, p: &SaveProfile) -> SaveSnapshot {
        self.b()
            .create_save_snapshot(
                CreateSnapshot {
                    profile_id: p.id.clone(),
                    label: Some("Fixture snapshot".into()),
                    note: None,
                },
                "fixture",
                None,
            )
            .unwrap()
    }
    fn confirm(&self, p: &RestorePreview) -> Result<RestoreResult> {
        self.b().restore_save_snapshot(
            RestoreSaveSnapshotRequest {
                snapshot_id: p.snapshot_id.clone(),
                preview_id: p.preview_id.clone(),
                confirmation_token: p.confirmation_token.clone(),
            },
            "fixture",
            None,
        )
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        drop(self.backend.take());
        let _ = fs::remove_dir_all(&self.root);
    }
}
#[test]
fn creates_verified_zip_and_only_reads_source_files() {
    let f = Fixture::new();
    let p = f.profile();
    let before = manifest(&f.source).unwrap();
    let s = f.backup(&p);
    assert_eq!(s.file_count, 2);
    assert_eq!(s.creation_reason, "manual");
    let path = archive_path(f.b(), &s).unwrap();
    assert_eq!(digest(&path, MAX_TOTAL).unwrap().sha256, s.sha256);
    assert_eq!(read_archive(&path, &s.sha256, None).unwrap(), before);
    assert_eq!(manifest(&f.source).unwrap(), before);
    assert!(!path.with_extension("partial").exists());
}
#[test]
fn restores_exact_snapshot_and_safety_backup_preserves_extra_files() {
    let f = Fixture::new();
    let p = f.profile();
    let s = f.backup(&p);
    fs::write(f.source.join("路线/slot.dat"), b"newer progress").unwrap();
    fs::remove_file(f.source.join("system.dat")).unwrap();
    fs::write(f.source.join("extra.dat"), b"keep this").unwrap();
    let current = manifest(&f.source).unwrap();
    let preview = f.b().preview_save_restore(&s.id).unwrap();
    assert_eq!(
        (
            preview.added_files,
            preview.modified_files,
            preview.deleted_files
        ),
        (1, 1, 1)
    );
    let result = f.confirm(&preview).unwrap();
    assert_eq!(result.deleted_files, 1);
    assert_eq!(
        fs::read(f.source.join("路线/slot.dat")).unwrap(),
        b"original save"
    );
    assert!(!f.source.join("extra.dat").exists());
    assert_eq!(
        manifest(&f.source).unwrap(),
        read_archive(&archive_path(f.b(), &s).unwrap(), &s.sha256, None).unwrap()
    );
    assert_eq!(preview.preserved_files, 0);
    assert!(preview
        .changes
        .iter()
        .any(|c| c.path == "extra.dat" && c.change == "deleted"));
    let safety = f
        .b()
        .database()
        .unwrap()
        .save_snapshot(&result.safety_backup_id)
        .unwrap();
    assert_eq!(safety.creation_reason, "safety_before_restore");
    assert_eq!(
        read_archive(&archive_path(f.b(), &safety).unwrap(), &safety.sha256, None).unwrap(),
        current
    );
    assert!(f.confirm(&preview).is_err()); // One-time confirmation.
}
#[test]
fn four_slots_restore_removes_fifth_and_safety_snapshot_can_undo() {
    let f = Fixture::new();
    let p = f.profile();
    for slot in 1..=4 {
        fs::write(f.source.join(format!("slot{slot}.dat")), [slot]).unwrap();
    }
    let expected = manifest(&f.source).unwrap();
    let snapshot = f.backup(&p);
    fs::write(f.source.join("slot5.dat"), b"fifth dialogue").unwrap();
    let before = manifest(&f.source).unwrap();
    let preview = f.b().preview_save_restore(&snapshot.id).unwrap();
    assert_eq!(
        (
            preview.added_files,
            preview.modified_files,
            preview.deleted_files
        ),
        (0, 0, 1)
    );
    let result = f.confirm(&preview).unwrap();
    assert_eq!(result.deleted_files, 1);
    assert_eq!(manifest(&f.source).unwrap(), expected);
    assert!(!f.source.join("slot5.dat").exists());
    let undo = f
        .b()
        .preview_save_restore(&result.safety_backup_id)
        .unwrap();
    f.confirm(&undo).unwrap();
    assert_eq!(manifest(&f.source).unwrap(), before);
}
#[test]
fn pure_deletions_report_full_counts_when_preview_is_truncated() {
    let f = Fixture::new();
    let p = f.profile();
    let snapshot = f.backup(&p);
    for slot in 0..205 {
        fs::write(f.source.join(format!("extra-{slot}.dat")), b"extra").unwrap();
    }
    let preview = f.b().preview_save_restore(&snapshot.id).unwrap();
    assert_eq!(preview.deleted_files, 205);
    assert!(preview.truncated);
    assert_eq!(preview.changes.len(), 200);
    assert!(preview.changes.iter().all(|c| c.change == "deleted"));
    let result = f.confirm(&preview).unwrap();
    assert_eq!(result.deleted_files, 205);
    assert_eq!(manifest(&f.source).unwrap().len(), 2);
}
#[test]
fn exact_restore_handles_file_and_directory_replacements() {
    let f = Fixture::new();
    let p = f.profile();
    let expected = manifest(&f.source).unwrap();
    let snapshot = f.backup(&p);
    fs::remove_file(f.source.join("system.dat")).unwrap();
    fs::create_dir(f.source.join("system.dat")).unwrap();
    fs::write(f.source.join("system.dat/new.dat"), b"nested extra").unwrap();
    fs::remove_dir_all(f.source.join("路线")).unwrap();
    fs::write(f.source.join("路线"), b"file replacing directory").unwrap();
    let preview = f.b().preview_save_restore(&snapshot.id).unwrap();
    f.confirm(&preview).unwrap();
    assert_eq!(manifest(&f.source).unwrap(), expected);
}
#[test]
fn profile_deletion_recycles_metadata_and_retains_files_and_other_profiles() {
    let f = Fixture::new();
    let mut p = f.profile();
    p.backup_before_launch = true;
    f.b().database().unwrap().store_save_profile(&p).unwrap();
    let snapshot = f.backup(&p);
    let pending = f.b().preview_save_restore(&snapshot.id).unwrap();
    let before = manifest(&f.source).unwrap();
    let path = archive_path(f.b(), &snapshot).unwrap();
    let archive_before = fs::read(&path).unwrap();
    let second_source = f.source.parent().unwrap().join("Save2");
    fs::create_dir(&second_source).unwrap();
    fs::write(second_source.join("slot.dat"), b"other profile").unwrap();
    let other = f
        .b()
        .configure_save_profile(ConfigureProfile {
            id: None,
            install_id: f.install.clone(),
            source_path: path_text(&second_source).unwrap(),
            backup_before_launch: false,
            backup_after_exit: false,
            retention_count: 10,
        })
        .unwrap();
    let other_snapshot = f.backup(&other);
    assert!(f
        .b()
        .delete_save_profile(DeleteProfile {
            profile_id: p.id.clone(),
            confirmed: true
        })
        .unwrap());
    assert_eq!(manifest(&f.source).unwrap(), before);
    assert_eq!(fs::read(&path).unwrap(), archive_before);
    let profiles = f.b().list_save_profiles(&f.game).unwrap();
    assert_eq!(profiles.len(), 1);
    assert_eq!(profiles[0].id, other.id);
    let snapshots = f.b().database().unwrap().save_snapshots(&f.game).unwrap();
    assert_eq!(snapshots.len(), 1);
    assert_eq!(snapshots[0].id, other_snapshot.id);
    let recycled: RecycledProfile = f
        .b()
        .database()
        .unwrap()
        .setting(&format!("saves.deleted-profile.{}", p.id))
        .unwrap()
        .unwrap();
    assert_eq!(recycled.profile.id, p.id);
    assert_eq!(recycled.snapshots.len(), 1);
    assert_eq!(recycled.snapshots[0].sha256, snapshot.sha256);
    assert_eq!(recycled.backup_directory, format!("save-backups/{}", p.id));
    assert!(!recycled.deleted_at.is_empty());
    assert!(f.confirm(&pending).is_err());
    f.b()
        .automatic_save_backup(&f.install, true, "fixture", None)
        .unwrap();
    assert_eq!(
        f.b()
            .database()
            .unwrap()
            .save_snapshots(&f.game)
            .unwrap()
            .len(),
        1
    );
    let replacement = f.profile();
    assert_ne!(replacement.id, p.id);
}
#[test]
fn profile_deletion_requires_confirmation_and_blocks_running_games() {
    let f = Fixture::new();
    let p = f.profile();
    assert!(f
        .b()
        .delete_save_profile(DeleteProfile {
            profile_id: p.id.clone(),
            confirmed: false
        })
        .is_err());
    f.b()
        .database()
        .unwrap()
        .begin_session(
            &LaunchSession {
                session_id: id(),
                game_id: f.game.clone(),
                install_id: f.install.clone(),
                started_at: now(),
            },
            "fixture.exe",
        )
        .unwrap();
    assert_eq!(
        f.b()
            .delete_save_profile(DeleteProfile {
                profile_id: p.id.clone(),
                confirmed: true
            })
            .unwrap_err()
            .0,
        ErrorCode::Conflict
    );
    assert!(f.b().database().unwrap().save_profile(&p.id).is_ok());
    assert!(f
        .b()
        .database()
        .unwrap()
        .setting::<RecycledProfile>(&format!("saves.deleted-profile.{}", p.id))
        .unwrap()
        .is_none());
}
#[test]
fn profile_deletion_rolls_back_metadata_and_indexes_on_database_failure() {
    let f = Fixture::new();
    let p = f.profile();
    let snapshot = f.backup(&p);
    let pending = f.b().preview_save_restore(&snapshot.id).unwrap();
    record_error(f.b(), &p.id, Some(&conflict("fixture old error"))).unwrap();
    let connection =
        rusqlite::Connection::open(f.b().data_directory.join("galgame-manager.sqlite3")).unwrap();
    connection.execute_batch("CREATE TRIGGER reject_profile_cleanup BEFORE DELETE ON settings WHEN OLD.key LIKE 'saves.error.%' BEGIN SELECT RAISE(ABORT, 'fixture cleanup failure'); END;").unwrap();
    assert!(f
        .b()
        .delete_save_profile(DeleteProfile {
            profile_id: p.id.clone(),
            confirmed: true
        })
        .is_err());
    assert!(f.b().database().unwrap().save_profile(&p.id).is_ok());
    assert!(f
        .b()
        .database()
        .unwrap()
        .save_snapshot(&snapshot.id)
        .is_ok());
    assert!(f
        .b()
        .saves
        .lock()
        .unwrap()
        .previews
        .contains_key(&pending.preview_id));
    assert!(f
        .b()
        .database()
        .unwrap()
        .setting::<RecycledProfile>(&format!("saves.deleted-profile.{}", p.id))
        .unwrap()
        .is_none());
    assert!(f
        .b()
        .database()
        .unwrap()
        .setting::<String>(&format!("saves.error.{}", p.id))
        .unwrap()
        .is_some());
    assert_eq!(
        digest(&archive_path(f.b(), &snapshot).unwrap(), MAX_TOTAL)
            .unwrap()
            .sha256,
        snapshot.sha256
    );
}
#[test]
fn missing_source_can_be_deleted_but_unfinished_restore_cannot() {
    let f = Fixture::new();
    let p = f.profile();
    let journal = RestoreJournal {
        profile_id: p.id.clone(),
        transaction_id: id(),
    };
    f.b()
        .database()
        .unwrap()
        .put_setting(JOURNAL, &journal)
        .unwrap();
    assert!(f
        .b()
        .delete_save_profile(DeleteProfile {
            profile_id: p.id.clone(),
            confirmed: true
        })
        .is_err());
    f.b().database().unwrap().delete_setting(JOURNAL).unwrap();
    fs::rename(&f.source, f.source.with_file_name("unavailable-save")).unwrap();
    assert!(f
        .b()
        .delete_save_profile(DeleteProfile {
            profile_id: p.id.clone(),
            confirmed: true
        })
        .unwrap());
    assert!(f
        .b()
        .delete_save_profile(DeleteProfile {
            profile_id: p.id,
            confirmed: true
        })
        .is_err());
}
#[test]
fn changed_source_invalidates_preview_without_writing() {
    let f = Fixture::new();
    let p = f.profile();
    let s = f.backup(&p);
    let preview = f.b().preview_save_restore(&s.id).unwrap();
    fs::write(f.source.join("路线/slot.dat"), b"changed after preview").unwrap();
    let before = manifest(&f.source).unwrap();
    assert_eq!(f.confirm(&preview).unwrap_err().0, ErrorCode::Conflict);
    assert_eq!(manifest(&f.source).unwrap(), before);
    assert_eq!(
        f.b()
            .database()
            .unwrap()
            .save_snapshots(&f.game)
            .unwrap()
            .len(),
        1
    );
}
#[test]
fn corruption_invalidates_archive_before_restore_or_write() {
    let f = Fixture::new();
    let p = f.profile();
    let s = f.backup(&p);
    let preview = f.b().preview_save_restore(&s.id).unwrap();
    fs::write(archive_path(f.b(), &s).unwrap(), b"corrupted zip").unwrap();
    let before = manifest(&f.source).unwrap();
    assert!(f.confirm(&preview).is_err());
    assert!(f.b().preview_save_restore(&s.id).is_err());
    assert_eq!(manifest(&f.source).unwrap(), before);
}
#[test]
fn rejects_expired_and_wrong_confirmations() {
    let f = Fixture::new();
    let p = f.profile();
    let s = f.backup(&p);
    let preview = f.b().preview_save_restore(&s.id).unwrap();
    f.b()
        .saves
        .lock()
        .unwrap()
        .previews
        .get_mut(&preview.preview_id)
        .unwrap()
        .expires = Instant::now() - Duration::from_secs(1);
    assert!(f.confirm(&preview).is_err());
    let mut preview = f.b().preview_save_restore(&s.id).unwrap();
    preview.confirmation_token = id();
    assert!(f.confirm(&preview).is_err());
    assert_eq!(
        fs::read(f.source.join("路线/slot.dat")).unwrap(),
        b"original save"
    );
}
#[test]
fn running_games_block_manual_backup_configuration_and_restore() {
    let f = Fixture::new();
    let p = f.profile();
    let s = f.backup(&p);
    let preview = f.b().preview_save_restore(&s.id).unwrap();
    f.b()
        .database()
        .unwrap()
        .begin_session(
            &LaunchSession {
                session_id: id(),
                game_id: f.game.clone(),
                install_id: f.install.clone(),
                started_at: now(),
            },
            "fixture.exe",
        )
        .unwrap();
    assert_eq!(
        f.b()
            .create_save_snapshot(
                CreateSnapshot {
                    profile_id: p.id.clone(),
                    label: None,
                    note: None
                },
                "fixture",
                None
            )
            .unwrap_err()
            .0,
        ErrorCode::Conflict
    );
    assert_eq!(f.confirm(&preview).unwrap_err().0, ErrorCode::Conflict);
    assert!(f
        .b()
        .configure_save_profile(ConfigureProfile {
            id: Some(p.id),
            install_id: f.install.clone(),
            source_path: path_text(&f.source).unwrap(),
            backup_before_launch: true,
            backup_after_exit: false,
            retention_count: 10
        })
        .is_err());
}
#[test]
fn refuses_disk_game_and_appdata_roots_and_duplicate_profiles() {
    let f = Fixture::new();
    let _p = f.profile();
    for source in [
        &f.root,
        f.source.parent().unwrap(),
        &f.b().data_directory,
        &f.source,
    ] {
        assert!(f
            .b()
            .configure_save_profile(ConfigureProfile {
                id: None,
                install_id: f.install.clone(),
                source_path: path_text(source).unwrap(),
                backup_before_launch: false,
                backup_after_exit: false,
                retention_count: 10
            })
            .is_err());
    }
}
#[test]
fn automatic_backup_is_opt_in_and_missing_sources_report_failure() {
    let f = Fixture::new();
    let p = f.profile();
    assert!(f
        .b()
        .skipped_exit_save_backup(&f.install)
        .unwrap()
        .is_none());
    f.b()
        .automatic_save_backup(&f.install, true, "fixture", None)
        .unwrap();
    assert!(f
        .b()
        .database()
        .unwrap()
        .save_snapshots(&f.game)
        .unwrap()
        .is_empty());
    f.b()
        .configure_save_profile(ConfigureProfile {
            id: Some(p.id.clone()),
            install_id: f.install.clone(),
            source_path: path_text(&f.source).unwrap(),
            backup_before_launch: true,
            backup_after_exit: true,
            retention_count: 10,
        })
        .unwrap();
    f.b()
        .automatic_save_backup(&f.install, true, "fixture", None)
        .unwrap();
    f.b()
        .automatic_save_backup(&f.install, false, "fixture", None)
        .unwrap();
    assert_eq!(
        f.b()
            .database()
            .unwrap()
            .save_snapshots(&f.game)
            .unwrap()
            .len(),
        2
    );
    assert!(f
        .b()
        .skipped_exit_save_backup(&f.install)
        .unwrap()
        .is_some());
    assert!(f.b().list_save_profiles(&f.game).unwrap()[0]
        .last_error
        .is_some());
    fs::rename(&f.source, f.root.join("moved-source")).unwrap();
    assert!(f
        .b()
        .automatic_save_backup(&f.install, true, "fixture", None)
        .is_err());
    let profiles = f.b().list_save_profiles(&f.game).unwrap();
    assert!(!profiles[0].source_available);
    assert!(profiles[0].last_error.is_some());
}
#[test]
fn failed_second_directory_rename_rolls_back_original() {
    let f = Fixture::new();
    let before = manifest(&f.source).unwrap();
    let original = f.source.parent().unwrap().join("original-fixture");
    assert!(swap_directories(&f.source, &f.root.join("nonexistent-stage"), &original).is_err());
    assert_eq!(manifest(&f.source).unwrap(), before);
    assert!(!original.exists());
}
#[test]
fn restart_recovers_interrupted_swap_and_keeps_staged_copy() {
    for swapped in [false, true] {
        let mut f = Fixture::new();
        let p = f.profile();
        let journal = RestoreJournal {
            profile_id: p.id,
            transaction_id: id(),
        };
        let (stage, original) = transaction_paths(&f.source, &journal).unwrap();
        fs::create_dir(&stage).unwrap();
        fs::write(stage.join("slot.dat"), b"restored version").unwrap();
        f.b()
            .database()
            .unwrap()
            .put_setting(JOURNAL, &journal)
            .unwrap();
        fs::rename(&f.source, &original).unwrap();
        if swapped {
            fs::rename(&stage, &f.source).unwrap();
        }
        let data = f.b().data_directory.clone();
        drop(f.backend.take());
        f.backend = Some(Backend::open(data).unwrap());
        assert_eq!(
            fs::read(f.source.join("路线/slot.dat")).unwrap(),
            b"original save"
        );
        assert_eq!(
            fs::read(stage.join("slot.dat")).unwrap(),
            b"restored version"
        );
        assert!(f
            .b()
            .database()
            .unwrap()
            .setting::<RestoreJournal>(JOURNAL)
            .unwrap()
            .is_none());
    }
}
#[test]
fn deletion_requires_confirmation_and_keeps_recoverable_zip() {
    let f = Fixture::new();
    let p = f.profile();
    let s = f.backup(&p);
    assert!(f
        .b()
        .delete_save_snapshot(DeleteSnapshot {
            snapshot_id: s.id.clone(),
            confirmed: false
        })
        .is_err());
    assert!(f
        .b()
        .delete_save_snapshot(DeleteSnapshot {
            snapshot_id: s.id.clone(),
            confirmed: true
        })
        .unwrap());
    assert!(f
        .b()
        .data_directory
        .join("save-backups/.trash")
        .join(format!("{}.zip", s.id))
        .is_file());
    assert!(f
        .b()
        .database()
        .unwrap()
        .save_snapshots(&f.game)
        .unwrap()
        .is_empty());
    assert_eq!(
        fs::read(f.source.join("路线/slot.dat")).unwrap(),
        b"original save"
    );
}
#[test]
fn portable_data_move_keeps_registered_archives_readable() {
    let mut f = Fixture::new();
    let p = f.profile();
    let s = f.backup(&p);
    let old = f.b().data_directory.clone();
    drop(f.backend.take());
    let new = f.root.join("moved-data");
    fs::rename(old, &new).unwrap();
    f.backend = Some(Backend::open(new).unwrap());
    assert!(f.b().preview_save_restore(&s.id).is_ok());
}
#[test]
fn zip_slip_absolute_reserved_and_case_conflicts_are_rejected() {
    let f = Fixture::new();
    for name in [
        "../escape.dat",
        "C:/escape.dat",
        "/escape.dat",
        "a\\escape.dat",
        "CON",
        "a/../escape.dat",
        "file.",
        "NUL.txt",
        "COM¹.dat",
        "LPT².dat",
        "CONIN$",
    ] {
        assert!(!files::valid_name(name));
    }
    let path = f.root.join("hostile.zip");
    let mut writer = zip::ZipWriter::new(fs::File::create(&path).unwrap());
    writer
        .start_file("../escape.dat", zip::write::SimpleFileOptions::default())
        .unwrap();
    writer.write_all(b"escape").unwrap();
    writer.finish().unwrap();
    let sha = digest(&path, MAX_TOTAL).unwrap().sha256;
    assert!(read_archive(&path, &sha, Some(&f.source)).is_err());
    assert!(!f.root.join("escape.dat").exists());
}
#[cfg(unix)]
#[test]
fn linked_save_files_are_rejected_and_external_files_are_untouched() {
    let f = Fixture::new();
    let p = f.profile();
    let outside = f.root.join("outside.dat");
    fs::write(&outside, b"external").unwrap();
    std::os::unix::fs::symlink(&outside, f.source.join("linked.dat")).unwrap();
    assert!(f
        .b()
        .create_save_snapshot(
            CreateSnapshot {
                profile_id: p.id,
                label: None,
                note: None
            },
            "fixture",
            None
        )
        .is_err());
    assert_eq!(fs::read(outside).unwrap(), b"external");
}
#[test]
fn profiles_and_snapshots_are_scoped_to_installation() {
    let f = Fixture::new();
    let p = f.profile();
    let _ = f.backup(&p);
    let other = f.root.join("other-install");
    fs::create_dir_all(other.join("save")).unwrap();
    fs::write(other.join("save/slot.dat"), b"other version").unwrap();
    let game = f
        .b()
        .database()
        .unwrap()
        .import_installation(
            other.to_str().unwrap(),
            "Fixture",
            Some(&f.game),
            InstallSource::Manual,
            &[],
            "other",
        )
        .unwrap();
    let install = f
        .b()
        .database()
        .unwrap()
        .get_game(&game)
        .unwrap()
        .summary
        .installations
        .into_iter()
        .find(|i| i.id != f.install)
        .unwrap()
        .id;
    let other_profile = f
        .b()
        .configure_save_profile(ConfigureProfile {
            id: None,
            install_id: install.clone(),
            source_path: path_text(&other.join("save")).unwrap(),
            backup_before_launch: true,
            backup_after_exit: false,
            retention_count: 10,
        })
        .unwrap();
    f.b()
        .automatic_save_backup(&install, true, "fixture", None)
        .unwrap();
    let snapshots = f.b().database().unwrap().save_snapshots(&f.game).unwrap();
    assert_eq!(snapshots.len(), 2);
    assert_eq!(
        snapshots
            .iter()
            .filter(|s| s.save_profile_id == p.id)
            .count(),
        1
    );
    assert_eq!(
        snapshots
            .iter()
            .filter(|s| s.save_profile_id == other_profile.id)
            .count(),
        1
    );
}
