use super::*;
struct Fixture {
    root: PathBuf,
    backend: Backend,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(id());
        fs::create_dir_all(&root).unwrap();
        let root = root.canonicalize().unwrap();
        let backend = Backend::open(root.join("appdata")).unwrap();
        Self { root, backend }
    }
    fn game(&self, name: &str) -> PathBuf {
        let p = self.root.join(name);
        fs::create_dir_all(&p).unwrap();
        fs::write(p.join("game.exe"), pe::fixture()).unwrap();
        p
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
fn wait(b: &Backend, id: &str) -> ScanReport {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let r = b.scan_report(id).unwrap();
        if matches!(
            r.progress.status,
            TaskStatus::Completed | TaskStatus::Failed | TaskStatus::Cancelled
        ) {
            return r;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(10));
    }
}
fn request(p: &Path) -> ScanRootsRequest {
    ScanRootsRequest {
        roots: vec![p.to_str().unwrap().into()],
        follow_symlinks: false,
    }
}
#[test]
fn portable_directory_is_locked_and_status_reads_empty_library() {
    let f = Fixture::new();
    let status = f.backend.status().unwrap();
    assert!(status.portable);
    assert_eq!(status.schema_version, 1);
    assert!(status.last_scan_task_id.is_none());
    assert_eq!(
        Backend::open(f.backend.data_directory.clone())
            .err()
            .unwrap()
            .0,
        ErrorCode::Conflict
    );
}
#[test]
fn deletion_requires_a_valid_database_snapshot_and_keeps_game_files() {
    let f = Fixture::new();
    let path = f.game("game");
    let game = manual_import(
        &f.backend,
        &ImportGameRequest {
            directory: path.to_str().unwrap().into(),
            title: "备份前记录".into(),
            game_id: None,
            skip_metadata: false,
        },
    )
    .unwrap();
    let backup_folder = f.backend.data_directory.join("database-backups");
    fs::write(&backup_folder, b"blocked directory").unwrap();
    assert!(f.backend.remove_record(&game.summary.id, false).is_err());
    assert!(f
        .backend
        .database()
        .unwrap()
        .get_game(&game.summary.id)
        .is_ok());
    fs::remove_file(&backup_folder).unwrap();
    assert!(f.backend.remove_record(&game.summary.id, false).unwrap());
    assert!(path.join("game.exe").is_file());
    let snapshot = fs::read_dir(&backup_folder)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let restored = Database::open(snapshot).unwrap();
    assert_eq!(
        restored.get_game(&game.summary.id).unwrap().summary.title,
        "备份前记录"
    );
}
#[test]
fn read_only_scan_imports_deduplicates_and_ignores_helpers() {
    let f = Fixture::new();
    let game = f.game("测试游戏");
    fs::write(game.join("unins000.exe"), pe::fixture()).unwrap();
    fs::write(game.join("invalid.exe"), b"do not run").unwrap();
    f.game("node_modules");
    let task = f
        .backend
        .start_scan(&request(&f.root), "scan1", None)
        .unwrap();
    let r = wait(&f.backend, &task.task_id);
    assert_eq!(r.progress.status, TaskStatus::Completed);
    assert_eq!(r.imported, 1);
    assert_eq!(r.issue_count, 1);
    assert_eq!(fs::read(game.join("game.exe")).unwrap(), pe::fixture());
    // Wait for manager cleanup before the next task.
    let deadline = Instant::now() + Duration::from_secs(2);
    while !f.backend.scans.controls.lock().unwrap().is_empty() {
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    }
    let task = f
        .backend
        .start_scan(&request(&f.root), "scan2", None)
        .unwrap();
    let r = wait(&f.backend, &task.task_id);
    assert_eq!(r.imported, 0);
    assert_eq!(r.unchanged, 1);
}

#[test]
fn reimport_preview_marks_existing_paths_and_discovers_new_games_without_database_writes() {
    let f = Fixture::new();
    let old_path = f.game("library/old-game");
    let root = old_path.parent().unwrap();
    let request = PreviewImportRequest {
        roots: vec![path_text(root).unwrap()],
        follow_symlinks: false,
        single_directory: false,
        single_executable: None,
    };
    let first = preview_import(&f.backend, &request).unwrap();
    assert_eq!(first.items.len(), 1);
    assert!(first.items[0].existing_game_id.is_none());
    let existing = manual_import(
        &f.backend,
        &ImportGameRequest {
            directory: path_text(&old_path).unwrap(),
            title: "已整理的作品".into(),
            game_id: None,
            skip_metadata: true,
        },
    )
    .unwrap();
    let original = serde_json::to_value(&existing).unwrap();
    f.game("library/new-game-a");
    f.game("library/new-game-b");
    let next = preview_import(&f.backend, &request).unwrap();
    assert_eq!(next.items.len(), 3);
    let old = next
        .items
        .iter()
        .find(|item| item.directory == path_text(&old_path).unwrap())
        .unwrap();
    assert_eq!(
        old.existing_game_id.as_deref(),
        Some(existing.summary.id.as_str())
    );
    assert!(old.duplicate_reason.is_some());
    assert_eq!(
        next.items
            .iter()
            .filter(|item| item.existing_game_id.is_none())
            .count(),
        2
    );
    assert_eq!(
        serde_json::to_value(
            f.backend
                .database()
                .unwrap()
                .get_game(&existing.summary.id)
                .unwrap()
        )
        .unwrap(),
        original
    );
    for item in next
        .items
        .iter()
        .filter(|item| item.existing_game_id.is_none())
    {
        assert!(f
            .backend
            .database()
            .unwrap()
            .installation_conflict(&item.directory)
            .unwrap()
            .is_none());
    }
}

#[test]
fn folder_first_scan_imports_one_record_per_root_child() {
    let f = Fixture::new();
    for name in ["game-a", "game-b", "game-c"] {
        let game = f.game(name);
        fs::create_dir_all(game.join("engine")).unwrap();
        fs::write(game.join("engine/helper.exe"), pe::fixture()).unwrap();
    }
    fs::create_dir_all(f.root.join("game-without-exe")).unwrap();
    fs::write(f.root.join("game-without-exe/readme.txt"), b"local game").unwrap();
    let task = f
        .backend
        .start_scan(&request(&f.root), "folder-first", None)
        .unwrap();
    let report = wait(&f.backend, &task.task_id);
    assert_eq!(report.imported, 4);
    assert_eq!(
        f.backend
            .database()
            .unwrap()
            .list_games(&crate::domain::models::GameQuery {
                page: 1,
                page_size: 100,
                search: "".into(),
                statuses: vec![],
                sources: vec![],
                tag_ids: vec![],
                favorite: None,
                collection_id: None,
                sort: crate::domain::models::GameSort::AddedAt,
                direction: crate::domain::models::SortDirection::Desc,
                filters: Default::default(),
            })
            .unwrap()
            .total,
        4
    );
}

#[test]
fn lunabox_preview_stops_at_launcher_directory_and_marks_duplicates() {
    let f = Fixture::new();
    let root = f.root.join("library");
    let first = root.join("first-game");
    fs::create_dir_all(first.join("engine")).unwrap();
    fs::write(first.join("first-game.exe"), pe::fixture()).unwrap();
    fs::write(first.join("engine/nested.exe"), pe::fixture()).unwrap();
    fs::write(first.join("setup.exe"), pe::fixture()).unwrap();
    let second = root.join("second-game");
    fs::create_dir_all(&second).unwrap();
    fs::write(second.join("start.bat"), b"start game.exe\r\n").unwrap();
    fs::write(second.join("start.exe"), pe::fixture()).unwrap();
    let preview = preview_import(
        &f.backend,
        &PreviewImportRequest {
            roots: vec![root.to_str().unwrap().into()],
            follow_symlinks: false,
            single_directory: false,
            single_executable: None,
        },
    )
    .unwrap();
    assert_eq!(preview.items.len(), 2);
    let first_item = preview
        .items
        .iter()
        .find(|item| item.folder_name == "first-game")
        .unwrap();
    assert_eq!(first_item.executables.len(), 1);
    assert_eq!(first_item.search_name, "first-game");
    assert!(first_item.selected_executable.is_some());
    assert!(preview
        .items
        .iter()
        .find(|item| item.folder_name == "second-game")
        .unwrap()
        .executables
        .iter()
        .all(|candidate| candidate.path.ends_with(".exe")));

    manual_import(
        &f.backend,
        &ImportGameRequest {
            directory: first.to_str().unwrap().into(),
            title: "已有记录".into(),
            game_id: None,
            skip_metadata: false,
        },
    )
    .unwrap();
    let preview = preview_import(
        &f.backend,
        &PreviewImportRequest {
            roots: vec![root.to_str().unwrap().into()],
            follow_symlinks: false,
            single_directory: false,
            single_executable: None,
        },
    )
    .unwrap();
    assert!(preview
        .items
        .iter()
        .find(|item| item.folder_name == "first-game")
        .unwrap()
        .duplicate_reason
        .is_some());
}

#[test]
fn batch_preview_discovers_wrapped_games_and_preserves_install_candidates() {
    let f = Fixture::new();
    let root = f.root.join("library");
    let boundary = root.join("中文作品名");
    let inner = boundary.join("原版目录");
    fs::create_dir_all(inner.join("engine")).unwrap();
    fs::write(boundary.join("说明.txt"), b"readme").unwrap();
    fs::write(boundary.join("setup.exe"), pe::fixture()).unwrap();
    fs::write(boundary.join("broken.exe"), b"not PE").unwrap();
    fs::write(inner.join("game.exe"), pe::fixture()).unwrap();
    fs::write(inner.join("other.exe"), pe::fixture()).unwrap();
    fs::write(inner.join("engine/helper.exe"), pe::fixture()).unwrap();
    let direct = root.join("直接入口");
    fs::create_dir_all(&direct).unwrap();
    fs::write(direct.join("game.exe"), pe::fixture()).unwrap();
    let preview = preview_import(
        &f.backend,
        &PreviewImportRequest {
            roots: vec![path_text(&root).unwrap()],
            follow_symlinks: false,
            single_directory: false,
            single_executable: None,
        },
    )
    .unwrap();
    assert_eq!(preview.items.len(), 2);
    let item = preview
        .items
        .iter()
        .find(|item| item.folder_name == "中文作品名")
        .unwrap();
    assert_eq!(item.search_name, "中文作品名");
    assert_eq!(item.directory, path_text(&boundary).unwrap());
    assert_eq!(item.executables.len(), 2);
    assert!(item
        .executables
        .iter()
        .all(|exe| Path::new(&exe.path).parent() == Some(inner.as_path())));
    assert_eq!(preview.issue_count, 1);
    assert_eq!(
        f.backend
            .database()
            .unwrap()
            .home_summary()
            .unwrap()
            .game_count,
        0
    );
    let detail = manual_import(
        &f.backend,
        &ImportGameRequest {
            directory: item.directory.clone(),
            title: item.search_name.clone(),
            game_id: None,
            skip_metadata: true,
        },
    )
    .unwrap();
    let install = &detail.summary.installations[0];
    let configuration = f
        .backend
        .configure_installation(&ConfigureInstallationRequest {
            install_id: install.id.clone(),
            executable_path: item.selected_executable.clone().unwrap(),
            arguments: vec![],
            working_directory: None,
            environment: Default::default(),
            steam_app_id: None,
            main_process_name: None,
            track_after_launcher_exit: true,
            idle_timeout_minutes: None,
            use_locale_emulator: None,
            use_magpie: None,
        })
        .unwrap();
    assert_eq!(configuration.candidates.len(), 2);
    assert_eq!(configuration.executable_path, item.selected_executable);
    let single = preview_import(
        &f.backend,
        &PreviewImportRequest {
            roots: vec![path_text(&inner).unwrap()],
            follow_symlinks: false,
            single_directory: false,
            single_executable: Some(path_text(&inner.join("game.exe")).unwrap()),
        },
    )
    .unwrap();
    assert_eq!(single.items.len(), 1);
    assert_eq!(single.scanned_directories, 1);
    assert_eq!(single.items[0].executables.len(), 2);
}

#[test]
fn batch_preview_merges_sibling_launchers_and_respects_depth_limit() {
    let f = Fixture::new();
    let root = f.root.join("library");
    let boundary = root.join("两个版本");
    for edition in ["原版", "中文版"] {
        let inner = boundary.join(edition);
        fs::create_dir_all(&inner).unwrap();
        fs::write(inner.join("game.exe"), pe::fixture()).unwrap();
    }
    let mut too_deep = root.join("超深目录");
    for _ in 0..=MAX_IMPORT_DEPTH {
        too_deep = too_deep.join("nested");
    }
    fs::create_dir_all(&too_deep).unwrap();
    fs::write(too_deep.join("game.exe"), pe::fixture()).unwrap();
    let preview = preview_import(
        &f.backend,
        &PreviewImportRequest {
            roots: vec![path_text(&root).unwrap()],
            follow_symlinks: false,
            single_directory: false,
            single_executable: None,
        },
    )
    .unwrap();
    assert_eq!(preview.items.len(), 1);
    assert_eq!(preview.items[0].executables.len(), 2);
    assert!(preview
        .issues
        .iter()
        .any(|issue| issue.reason.contains("最大深度")));
}

#[test]
fn batch_preview_stops_when_entry_budget_is_exhausted() {
    let f = Fixture::new();
    let directory = f.game("bounded");
    let mut report = ImportPreviewReport {
        items: Vec::new(),
        scanned_directories: 0,
        skipped_directories: 0,
        issue_count: 0,
        issues: vec![],
    };
    let mut entries_seen = MAX_ENTRIES;
    assert!(collect_preview_executables(
        &directory,
        0,
        MAX_IMPORT_DEPTH,
        &mut report,
        &mut entries_seen
    )
    .is_err());
}

#[cfg(unix)]
#[test]
fn batch_preview_does_not_follow_links_to_external_games() {
    let f = Fixture::new();
    let root = f.root.join("library");
    let boundary = root.join("wrapped");
    let inner = boundary.join("actual");
    fs::create_dir_all(&inner).unwrap();
    fs::write(inner.join("game.exe"), pe::fixture()).unwrap();
    let external = f.game("outside");
    std::os::unix::fs::symlink(&external, boundary.join("linked-game")).unwrap();
    std::os::unix::fs::symlink(&boundary, boundary.join("cycle")).unwrap();
    let preview = preview_import(
        &f.backend,
        &PreviewImportRequest {
            roots: vec![path_text(&root).unwrap()],
            follow_symlinks: false,
            single_directory: false,
            single_executable: None,
        },
    )
    .unwrap();
    assert_eq!(preview.items.len(), 1);
    assert_eq!(preview.items[0].executables.len(), 1);
    assert_eq!(preview.issue_count, 2);
    assert_eq!(preview.skipped_directories, 2);
}

#[test]
#[ignore = "requires an explicitly supplied local game library"]
fn supplied_library_preview_is_read_only_and_finds_all_game_boundaries() {
    let f = Fixture::new();
    let root = absolute_directory(&std::env::var("GALGAME_MANAGER_TEST_LIBRARY").unwrap()).unwrap();
    let preview = preview_import(
        &f.backend,
        &PreviewImportRequest {
            roots: vec![path_text(&root).unwrap()],
            follow_symlinks: false,
            single_directory: false,
            single_executable: None,
        },
    )
    .unwrap();
    println!("{}", serde_json::to_string_pretty(&preview).unwrap());
    let boundaries = game_boundaries(&root).unwrap();
    assert_eq!(preview.items.len(), boundaries.len());
    assert!(preview
        .items
        .iter()
        .all(|item| item.search_name == item.folder_name));
    assert!(preview
        .items
        .iter()
        .all(|item| !item.executables.is_empty() && item.selected_executable.is_some()));
    assert_eq!(
        f.backend
            .database()
            .unwrap()
            .home_summary()
            .unwrap()
            .game_count,
        0
    );
}

#[test]
fn vndb_match_persists_field_provenance_and_aliases() {
    let f = Fixture::new();
    let game = manual_import(
        &f.backend,
        &ImportGameRequest {
            directory: f.game("作品").to_str().unwrap().into(),
            title: "本地作品".into(),
            game_id: None,
            skip_metadata: false,
        },
    )
    .unwrap();
    let fetched_at = now();
    f.backend
        .database()
        .unwrap()
        .apply_vndb_match_with_details(
            &game.summary.id,
            "v17",
            "Kanon",
            None,
            None,
            Some("カノン"),
            Some("Key"),
            None,
            Some("1999-06-04"),
            Some(8.08),
            &["校园".into(), "恋爱".into()],
            Some("简介"),
            None,
            &fetched_at,
        )
        .unwrap();
    let detail = f
        .backend
        .database()
        .unwrap()
        .get_game(&game.summary.id)
        .unwrap();
    assert_eq!(detail.summary.title_en.as_deref(), Some("Kanon"));
    assert_eq!(detail.summary.developer.as_deref(), Some("Key"));
    assert_eq!(detail.summary.source_tags, vec!["校园", "恋爱"]);
    assert_eq!(detail.summary.source_rating, Some(8.08));
    assert!(detail.metadata.iter().any(|field| field.provider == "vndb"));
    f.backend
        .database()
        .unwrap()
        .apply_bangumi_cover(
            &game.summary.id,
            "123",
            "https://lain.bgm.tv/pic/cover/l/example.jpg",
            &fetched_at,
        )
        .unwrap();
    let detail = f
        .backend
        .database()
        .unwrap()
        .get_game(&game.summary.id)
        .unwrap();
    assert_eq!(
        detail.summary.cover_url.as_deref(),
        Some("https://lain.bgm.tv/pic/cover/l/example.jpg")
    );
    assert!(detail
        .metadata
        .iter()
        .any(|field| field.provider == "bangumi" && field.field == "cover_url"));
    let cached = format!("covers/{}.jpg", "a".repeat(64));
    f.backend
        .database()
        .unwrap()
        .apply_cached_bangumi_cover(
            &game.summary.id,
            "123",
            "https://lain.bgm.tv/pic/cover/l/example.jpg",
            &cached,
            &fetched_at,
        )
        .unwrap();
    let detail = f
        .backend
        .database()
        .unwrap()
        .get_game(&game.summary.id)
        .unwrap();
    assert_eq!(detail.summary.cover_url.as_deref(), Some(cached.as_str()));
    let original_url = serde_json::json!("https://lain.bgm.tv/pic/cover/l/example.jpg").to_string();
    assert!(detail
        .metadata
        .iter()
        .any(|field| field.provider == "bangumi" && field.value == original_url));
    let result = bangumi::retry(
        &f.backend,
        bangumi::RetryRequest {
            game_id: game.summary.id.clone(),
            query: Some("无匹配测试".into()),
        },
    )
    .unwrap();
    assert_eq!(result.status, "no_match");
    assert!(!result.message.is_empty());
    assert_eq!(
        f.backend
            .database()
            .unwrap()
            .get_game(&game.summary.id)
            .unwrap()
            .summary
            .cover_url
            .as_deref(),
        Some(cached.as_str())
    );
}

#[test]
fn cached_exact_vndb_match_is_applied_automatically() {
    let f = Fixture::new();
    let game = manual_import(
        &f.backend,
        &ImportGameRequest {
            directory: f.game("Kanon").to_str().unwrap().into(),
            title: "Kanon".into(),
            game_id: None,
            skip_metadata: false,
        },
    )
    .unwrap();
    let response = serde_json::json!({
        "results": [{
            "id": "v17",
            "title": "Kanon",
            "alttitle": "カノン",
            "titles": [{"lang": "zh-Hans", "title": "Kanon 中文名"}],
            "developers": [{"name": "Key"}],
            "released": "1999-06-04",
            "description": "fixture",
            "image": {"url": "https://t.vndb.org/cv/00/000017.jpg"}
        }]
    });
    let response = serde_json::to_string(&response).unwrap();
    f.backend
        .database()
        .unwrap()
        .save_metadata_cache("vndb", "Kanon", Some(&response), "success", None, &now())
        .unwrap();
    // The cached VNDB response remains unused until that source is explicitly enabled.
    automatic_metadata_match(&f.backend, &game.summary.id, "Kanon").unwrap();
    assert_eq!(
        f.backend
            .database()
            .unwrap()
            .get_game(&game.summary.id)
            .unwrap()
            .summary
            .metadata_status,
        crate::domain::protocol::MetadataStatus::LocalOnly
    );
    let mut sources = metadata_sources::Config::default();
    sources.sources[2].enabled = true;
    metadata_sources::save(&f.backend, sources).unwrap();
    automatic_metadata_match(&f.backend, &game.summary.id, "Kanon").unwrap();
    let detail = f
        .backend
        .database()
        .unwrap()
        .get_game(&game.summary.id)
        .unwrap();
    assert_eq!(
        detail.summary.metadata_status,
        crate::domain::protocol::MetadataStatus::Synced
    );
    assert_eq!(detail.summary.title_en.as_deref(), Some("Kanon"));
}

#[test]
fn automatic_identity_requires_unique_confidence_but_not_language_or_cover() {
    let candidate = || MetadataCandidate {
        provider: "bangumi".into(),
        remote_id: "123".into(),
        title: "中文作品".into(),
        subtitle: None,
        cover_url: Some("https://lain.bgm.tv/pic/cover/l/fixture.jpg".into()),
        has_chinese_description: Some(true),
        confidence: 1.0,
        matched_fields: vec!["title".into()],
        explanation: "fixture".into(),
        fetched_at: now(),
        cached: true,
    };
    assert!(automatic_candidate(&[candidate()]).is_some());
    for title in ["English Title", "彼女との物語"] {
        let mut foreign = candidate();
        foreign.title = title.into();
        assert!(automatic_candidate(&[foreign]).is_some());
    }
    let mut ambiguous = candidate();
    ambiguous.remote_id = "124".into();
    ambiguous.confidence = 0.99;
    assert!(automatic_candidate(&[candidate(), ambiguous]).is_none());
    let mut no_cover = candidate();
    no_cover.cover_url = None;
    assert!(automatic_candidate(&[no_cover]).is_some());
}

#[test]
fn pending_metadata_candidates_are_exposed_in_game_and_home_status() {
    let f = Fixture::new();
    let game = manual_import(
        &f.backend,
        &ImportGameRequest {
            directory: f.game("待确认作品").to_str().unwrap().into(),
            title: "待确认作品".into(),
            game_id: None,
            skip_metadata: false,
        },
    )
    .unwrap();
    let candidate = crate::domain::models::MetadataCandidate {
        provider: "vndb".into(),
        remote_id: "v17".into(),
        title: "Kanon".into(),
        subtitle: None,
        cover_url: None,
        has_chinese_description: None,
        confidence: 0.55,
        matched_fields: vec!["search".into()],
        explanation: "需人工确认".into(),
        fetched_at: now(),
        cached: true,
    };
    f.backend
        .database()
        .unwrap()
        .put_setting(
            &format!("metadata.candidates.{}", game.summary.id),
            &vec![candidate],
        )
        .unwrap();
    let detail = f
        .backend
        .database()
        .unwrap()
        .get_game(&game.summary.id)
        .unwrap();
    assert_eq!(
        detail.summary.metadata_status,
        crate::domain::protocol::MetadataStatus::PendingConfirmation
    );
    assert_eq!(
        f.backend
            .database()
            .unwrap()
            .home_summary()
            .unwrap()
            .pending_match_count,
        1
    );
}
#[cfg(unix)]
#[test]
fn symlink_cycles_and_linked_executables_are_skipped() {
    let f = Fixture::new();
    let game = f.game("game");
    std::os::unix::fs::symlink(&f.root, game.join("loop")).unwrap();
    std::os::unix::fs::symlink(game.join("game.exe"), game.join("linked.exe")).unwrap();
    let task = f
        .backend
        .start_scan(&request(&f.root), "symlink", None)
        .unwrap();
    let r = wait(&f.backend, &task.task_id);
    assert_eq!(r.imported, 1);
    assert!(r.progress.processed < 20);
    assert_eq!(directory_candidates(&game).unwrap().len(), 1);
}
#[test]
fn depth_limit_is_reported_and_invalid_roots_never_start() {
    let f = Fixture::new();
    let mut p = f.root.clone();
    for _ in 0..34 {
        p = p.join("nested");
    }
    fs::create_dir_all(&p).unwrap();
    fs::write(p.join("game.exe"), pe::fixture()).unwrap();
    let task = f
        .backend
        .start_scan(&request(&f.root), "depth", None)
        .unwrap();
    let r = wait(&f.backend, &task.task_id);
    assert!(r.truncated);
    assert_eq!(r.imported, 0);
    assert!(r.issue_count > 0);
    assert!(f
        .backend
        .start_scan(
            &ScanRootsRequest {
                roots: vec!["relative".into()],
                follow_symlinks: false
            },
            "bad",
            None
        )
        .is_err());
}
#[test]
fn paused_scan_can_be_cancelled_without_importing() {
    let f = Fixture::new();
    f.game("game");
    let flag = Arc::new(AtomicU8::new(1));
    let b = f.backend.clone();
    let root = f.root.clone();
    let control = flag.clone();
    let join = std::thread::spawn(move || {
        let mut r = ScanReport {
            progress: TaskProgress {
                task_id: "pause".into(),
                status: TaskStatus::Queued,
                phase: "queued".into(),
                processed: 0,
                total: None,
                message: "".into(),
            },
            imported: 0,
            unchanged: 0,
            issue_count: 0,
            issues: vec![],
            truncated: false,
        };
        scan(&b, vec![root], &control, &mut r, "pause", None).unwrap();
        r
    });
    let deadline = Instant::now() + Duration::from_secs(2);
    while f.backend.scan_report("pause").is_err() {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(
        f.backend.scan_report("pause").unwrap().progress.status,
        TaskStatus::Paused
    );
    flag.store(2, Ordering::SeqCst);
    let r = join.join().unwrap();
    assert_eq!(r.progress.status, TaskStatus::Cancelled);
    assert_eq!(r.imported, 0);
}
#[test]
fn manual_configuration_requires_contained_pe_and_never_executes_on_mac() {
    let f = Fixture::new();
    let game = f.game("game");
    let detail = manual_import(
        &f.backend,
        &ImportGameRequest {
            directory: game.to_str().unwrap().into(),
            title: "手动作品".into(),
            game_id: None,
            skip_metadata: false,
        },
    )
    .unwrap();
    let install = &detail.summary.installations[0];
    assert!(install.executable_path.is_none());
    let mut r = ConfigureInstallationRequest {
        main_process_name: None,
        track_after_launcher_exit: true,
        idle_timeout_minutes: None,
        use_locale_emulator: None,
        use_magpie: None,
        install_id: install.id.clone(),
        executable_path: game.join("game.exe").to_str().unwrap().into(),
        arguments: vec!["a b".into()],
        working_directory: None,
        environment: Default::default(),
        steam_app_id: None,
    };
    f.backend.configure_installation(&r).unwrap();
    let outside = f.game("outside");
    r.executable_path = outside.join("game.exe").to_str().unwrap().into();
    assert!(f.backend.configure_installation(&r).is_err());
    #[cfg(not(windows))]
    assert_eq!(
        f.backend
            .launch(
                &crate::domain::requests::LaunchGameRequest {
                    install_id: install.id.clone(),
                    options: crate::domain::requests::LaunchOptions {
                        user_initiated: true
                    }
                },
                "launch",
                None
            )
            .unwrap_err()
            .0,
        ErrorCode::NotImplemented
    );
}

#[test]
fn single_directory_keeps_selected_game_boundary_and_checks_legacy_defaults() {
    let f = Fixture::new();
    let game = f.game("one-game");
    fs::write(game.join("alternative.exe"), pe::fixture()).unwrap();
    fs::create_dir_all(game.join("engine")).unwrap();
    fs::write(game.join("engine/second.exe"), pe::fixture()).unwrap();
    fs::write(game.join("setup.exe"), pe::fixture()).unwrap();
    let mut request = PreviewImportRequest {
        roots: vec![path_text(&game).unwrap()],
        follow_symlinks: false,
        single_directory: true,
        single_executable: None,
    };
    let report = preview_import(&f.backend, &request).unwrap();
    assert_eq!(report.items.len(), 1);
    assert_eq!(report.items[0].directory, path_text(&game).unwrap());
    assert_eq!(report.items[0].folder_name, "one-game");
    assert_eq!(report.items[0].executables.len(), 2);
    assert_eq!(
        f.backend
            .database()
            .unwrap()
            .home_summary()
            .unwrap()
            .game_count,
        0
    );
    request.single_executable = Some(path_text(&game.join("game.exe")).unwrap());
    assert!(preview_import(&f.backend, &request).is_err());
    request.single_executable = None;
    fs::remove_file(game.join("game.exe")).unwrap();
    fs::remove_file(game.join("alternative.exe")).unwrap();
    let nested = preview_import(&f.backend, &request).unwrap();
    assert_eq!(nested.items.len(), 1);
    assert_eq!(nested.items[0].directory, path_text(&game).unwrap());
    assert!(nested.items[0].executables[0]
        .path
        .ends_with("engine/second.exe"));
    request.roots.push(path_text(&f.game("other")).unwrap());
    assert!(preview_import(&f.backend, &request).is_err());
    let legacy: PreviewImportRequest = serde_json::from_value(
        serde_json::json!({"roots":[path_text(&game).unwrap()],"follow_symlinks":false}),
    )
    .unwrap();
    assert!(!legacy.single_directory);
}
