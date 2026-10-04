use super::*;
use std::{cell::Cell, fs};

struct Fixture {
    root: PathBuf,
    backend: Backend,
    directory: String,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("gm-prepared-{}", id()));
        let backend = Backend::open(root.join("data")).unwrap();
        let directory = root.join("本地文件夹");
        fs::create_dir_all(&directory).unwrap();
        fs::write(directory.join("game.exe"), super::super::pe::fixture()).unwrap();
        Self {
            root,
            backend,
            directory: path_text(&directory.canonicalize().unwrap()).unwrap(),
        }
    }
    fn request(&self) -> PrepareRequest {
        PrepareRequest {
            manual: false,
            single_source: false,
            title_hint: None,
            batch_id: None,
            directory: self.directory.clone(),
            provider: "bangumi".into(),
            remote_id: "123".into(),
        }
    }
    fn commit_request(&self, preparation_id: Option<String>) -> CommitRequest {
        CommitRequest {
            directory: self.directory.clone(),
            title: "本地文件夹".into(),
            executable_path: Some(format!("{}/game.exe", self.directory)),
            preparation_id,
        }
    }
    fn count(&self) -> u64 {
        self.backend
            .database()
            .unwrap()
            .home_summary()
            .unwrap()
            .game_count
    }
    fn prepare(&self) -> Preparation {
        prepare_with(&self.backend, self.request(), fill).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn image(backend: &Backend) -> String {
    let mut encoded = std::io::Cursor::new(Vec::new());
    image::DynamicImage::new_rgb8(2, 2)
        .write_to(&mut encoded, image::ImageFormat::Png)
        .unwrap();
    let bytes = encoded.into_inner();
    let filename = format!("{:x}.png", Sha256::digest(&bytes));
    fs::create_dir_all(backend.data_directory.join("covers")).unwrap();
    fs::write(backend.data_directory.join("covers").join(&filename), bytes).unwrap();
    format!("covers/{filename}")
}
fn fill(stage: &Backend, request: &ConfirmMetadataMatchRequest) -> Result<Option<String>> {
    let cover = image(stage);
    stage.database()?.apply_remote_fields(
        &request.game_id,
        "bangumi",
        &request.remote_id,
        &[
            ("title_zh".into(), "中文作品名".into()),
            ("title_ja".into(), "日本語タイトル".into()),
            ("description_zh".into(), "这是来源维护的中文简介。".into()),
            ("cover_path".into(), cover),
            ("source_rating".into(), "8.1".into()),
            ("publisher".into(), "发行商".into()),
        ],
        &[("别名".into(), "zh".into())],
        &now(),
        true,
    )?;
    Ok(Some("资料已准备".into()))
}

#[test]
fn scraping_prepares_full_data_and_local_cover_without_writing_formal_library() {
    let f = Fixture::new();
    let fetches = Cell::new(0);
    let prepared = prepare_with(&f.backend, f.request(), |stage, request| {
        fetches.set(fetches.get() + 1);
        fill(stage, request)
    })
    .unwrap();
    assert_eq!(f.count(), 0);
    assert_eq!(prepared.title, "日本語タイトル");
    assert!(f
        .backend
        .data_directory
        .join(&prepared.cover_path)
        .is_file());
    let game = commit(
        &f.backend,
        f.commit_request(Some(prepared.preparation_id.clone())),
    )
    .unwrap();
    assert_eq!(fetches.get(), 1, "confirm import must not scrape again");
    assert_eq!(f.count(), 1);
    assert_eq!(game.summary.title, "日本語タイトル");
    assert_eq!(game.summary.title_ja.as_deref(), Some("日本語タイトル"));
    assert_eq!(game.summary.publisher.as_deref(), Some("发行商"));
    assert_eq!(game.summary.source_rating, Some(8.1));
    assert_eq!(
        game.description.as_deref(),
        Some("这是来源维护的中文简介。")
    );
    assert!(game
        .metadata
        .iter()
        .any(|field| field.provider == "bangumi"));
    let installation = f
        .backend
        .database()
        .unwrap()
        .installation(&game.summary.installations[0].id)
        .unwrap();
    assert_eq!(
        installation.executable_path.as_deref(),
        Some(format!("{}/game.exe", f.directory).as_str())
    );
    assert_eq!(
        installation.working_directory.as_deref(),
        Some(f.directory.as_str())
    );
    assert!(f.backend.prepared_imports.lock().unwrap().items.is_empty());
}

#[test]
fn generated_import_titles_follow_remote_updates_but_explicit_names_are_preserved() {
    let f = Fixture::new();
    let prepared = f.prepare();
    let game = commit(&f.backend, f.commit_request(Some(prepared.preparation_id))).unwrap();
    let mut db = f.backend.database().unwrap();
    let apply_title = |db: &mut crate::database::Database, title: &str| {
        db.apply_remote_fields(
            &game.summary.id,
            "bangumi",
            "123",
            &[("title".into(), title.into())],
            &[],
            &now(),
            true,
        )
        .unwrap();
    };
    apply_title(&mut db, "后续中文名称");
    assert_eq!(
        db.get_game(&game.summary.id).unwrap().summary.title,
        "后续中文名称"
    );
    db.update_game(&types::UpdateGameRequest {
        game_id: game.summary.id.clone(),
        title: "用户指定名称".into(),
        status: game.summary.status,
        favorite: false,
        hidden: false,
        user_rating: None,
    })
    .unwrap();
    apply_title(&mut db, "再次更新中文名称");
    assert_eq!(
        db.get_game(&game.summary.id).unwrap().summary.title,
        "用户指定名称"
    );
}

#[test]
fn missing_cover_is_not_success_and_retry_does_not_leave_half_a_game() {
    let f = Fixture::new();
    let prepared = f.prepare();
    let path = f.backend.data_directory.join(&prepared.cover_path);
    fs::remove_file(&path).unwrap();
    assert!(commit(
        &f.backend,
        f.commit_request(Some(prepared.preparation_id.clone()))
    )
    .is_err());
    assert_eq!(f.count(), 0);
    assert_eq!(f.backend.prepared_imports.lock().unwrap().items.len(), 1);
    image(&f.backend);
    assert!(commit(&f.backend, f.commit_request(Some(prepared.preparation_id))).is_ok());
    assert_eq!(f.count(), 1);
}

#[test]
fn preparation_reports_actual_fallback_binding_when_selected_source_is_unavailable() {
    let f = Fixture::new();
    let prepared = prepare_with(&f.backend, f.request(), |stage, request| {
        let cover = image(stage);
        stage.database()?.apply_remote_fields(
            &request.game_id,
            "vndb",
            "v123",
            &[
                ("title".into(), "Fallback Title".into()),
                ("cover_path".into(), cover),
                ("description".into(), "Fallback prose.".into()),
            ],
            &[],
            &now(),
            true,
        )?;
        Ok(None)
    })
    .unwrap();
    assert_eq!(prepared.provider, "vndb");
    assert_eq!(prepared.remote_id, "v123");
    let game = commit(&f.backend, f.commit_request(Some(prepared.preparation_id))).unwrap();
    assert_eq!(game.summary.title, "Fallback Title");
    assert_eq!(game.description.as_deref(), Some("Fallback prose."));
}

#[test]
fn failed_or_incomplete_fetch_never_produces_a_ready_token() {
    let f = Fixture::new();
    assert!(
        prepare_with(&f.backend, f.request(), |_, _| Err(ServiceError(
            ErrorCode::NetworkUnavailable,
            "离线"
        )))
        .is_err()
    );
    assert!(prepare_with(&f.backend, f.request(), |stage, request| {
        stage.database()?.apply_remote_fields(
            &request.game_id,
            "bangumi",
            "123",
            &[("title_zh".into(), "只有名称".into())],
            &[],
            &now(),
            true,
        )?;
        Ok(None)
    })
    .is_err());
    assert_eq!(f.count(), 0);
    assert!(f.backend.prepared_imports.lock().unwrap().items.is_empty());
}

#[test]
fn wrong_directory_bad_executable_and_discarded_token_reject_without_writes() {
    let f = Fixture::new();
    let p = f.prepare();
    let other = f.root.join("另一个游戏");
    fs::create_dir_all(&other).unwrap();
    let mut q = f.commit_request(Some(p.preparation_id.clone()));
    q.directory = path_text(&other).unwrap();
    q.executable_path = None;
    assert!(commit(&f.backend, q).is_err());
    let mut q = f.commit_request(Some(p.preparation_id.clone()));
    q.executable_path = Some(path_text(&f.root.join("data/app.lock")).unwrap());
    assert!(commit(&f.backend, q).is_err());
    discard(
        &f.backend,
        DiscardRequest {
            preparation_ids: vec![p.preparation_id.clone()],
        },
    )
    .unwrap();
    assert!(commit(&f.backend, f.commit_request(Some(p.preparation_id))).is_err());
    assert_eq!(f.count(), 0);
    assert!(
        f.backend.data_directory.join(p.cover_path).is_file(),
        "discard keeps reusable cover bytes"
    );
}

#[test]
fn repeated_path_and_remote_binding_do_not_overwrite_existing_game() {
    let f = Fixture::new();
    let p = f.prepare();
    commit(&f.backend, f.commit_request(Some(p.preparation_id))).unwrap();
    assert!(commit(&f.backend, f.commit_request(None)).is_err());
    let other = f.root.join("重复资料目录");
    fs::create_dir_all(&other).unwrap();
    let mut request = f.request();
    request.directory = path_text(&other).unwrap();
    let p = prepare_with(&f.backend, request, fill).unwrap();
    assert!(commit(
        &f.backend,
        CommitRequest {
            directory: path_text(&other).unwrap(),
            title: "另一个名字".into(),
            executable_path: None,
            preparation_id: Some(p.preparation_id)
        }
    )
    .is_err());
    assert_eq!(f.count(), 1);
}

#[test]
fn unmatched_import_is_local_only_and_does_not_require_scraping() {
    let f = Fixture::new();
    let game = commit(&f.backend, f.commit_request(None)).unwrap();
    assert_eq!(game.summary.title, "本地文件夹");
    assert!(game.metadata.is_empty());
    assert!(game.summary.cover_url.is_none());
    assert_eq!(f.count(), 1);
}

#[test]
fn metadata_insert_failure_rolls_back_game_installation_and_source_rows() {
    let f = Fixture::new();
    let p = f.prepare();
    {
        let mut manager = f.backend.prepared_imports.lock().unwrap();
        // Simulate an internal snapshot corruption failing after game/installation inserts.
        manager
            .items
            .get_mut(&p.preparation_id)
            .unwrap()
            .snapshot
            .fields[0]
            .value_json = "not json".into();
    }
    assert!(commit(&f.backend, f.commit_request(Some(p.preparation_id.clone()))).is_err());
    assert_eq!(f.count(), 0);
    assert!(f
        .backend
        .database()
        .unwrap()
        .installation_conflict(&f.directory)
        .unwrap()
        .is_none());
    assert!(f
        .backend
        .prepared_imports
        .lock()
        .unwrap()
        .items
        .contains_key(&p.preparation_id));
}

#[test]
fn cancellation_cleans_only_unreferenced_batch_downloads_and_rejects_late_writes() {
    let f = Fixture::new();
    let first = begin_batch(&f.backend).unwrap();
    let second = begin_batch(&f.backend).unwrap();
    let mut a = f.backend.clone();
    a.import_batch = Some(first.batch_id.clone());
    let mut b = f.backend.clone();
    b.import_batch = Some(second.batch_id.clone());
    let directory = f.backend.data_directory.join("covers");
    fs::create_dir_all(&directory).unwrap();
    let original = directory.join("existing.jpg");
    fs::write(&original, b"existing").unwrap();
    persist_cover(&a, &original, b"existing", "covers/existing.jpg").unwrap();
    let shared = directory.join("shared.jpg");
    persist_cover(&a, &shared, b"new shared", "covers/shared.jpg").unwrap();
    persist_cover(&b, &shared, b"new shared", "covers/shared.jpg").unwrap();
    let owned = directory.join("owned.jpg");
    persist_cover(&a, &owned, b"new own", "covers/owned.jpg").unwrap();
    cancel_batch(
        &f.backend,
        CancelBatch {
            batch_id: first.batch_id.clone(),
        },
    )
    .unwrap();
    assert!(original.exists());
    assert!(shared.exists());
    assert!(!owned.exists());
    assert!(persist_cover(&a, &owned, b"late", "covers/owned.jpg").is_err());
    assert!(!owned.exists());
    cancel_batch(
        &f.backend,
        CancelBatch {
            batch_id: second.batch_id,
        },
    )
    .unwrap();
    assert!(!shared.exists());
    assert!(original.exists());
}
#[test]
fn batch_settings_are_snapshotted_even_after_global_reordering() {
    let f = Fixture::new();
    let started = begin_batch(&f.backend).unwrap();
    let mut changed = metadata_sources::Config::default();
    changed.sources.reverse();
    changed.sources[1].enabled = false;
    metadata_sources::save(&f.backend, changed.clone()).unwrap();
    assert_eq!(
        batch_config(&f.backend, &started.batch_id)
            .unwrap()
            .enabled(),
        vec!["hikarinagi", "bangumi", "vndb"]
    );
    assert_eq!(
        metadata_sources::get(&f.backend).unwrap().enabled(),
        changed.enabled()
    );
    cancel_batch(
        &f.backend,
        CancelBatch {
            batch_id: started.batch_id.clone(),
        },
    )
    .unwrap();
    let mut request = f.request();
    request.batch_id = Some(started.batch_id);
    assert!(prepare_with(&f.backend, request, fill).is_err());
    assert_eq!(f.count(), 0);
}

#[test]
fn cancellation_keeps_a_new_cover_now_referenced_by_a_formal_game() {
    let f = Fixture::new();
    let started = begin_batch(&f.backend).unwrap();
    let mut stage = f.backend.clone();
    stage.import_batch = Some(started.batch_id.clone());
    let target = stage.data_directory.join("covers").join("registered.jpg");
    fs::create_dir_all(target.parent().unwrap()).unwrap();
    persist_cover(&stage, &target, b"new cover", "covers/registered.jpg").unwrap();
    let game = f
        .backend
        .database()
        .unwrap()
        .import_installation(
            "/fixture/owned-game",
            "owned-game",
            None,
            InstallSource::Manual,
            &[],
            "",
        )
        .unwrap();
    f.backend
        .database()
        .unwrap()
        .apply_remote_fields(
            &game,
            "bangumi",
            "5",
            &[("cover_path".into(), "covers/registered.jpg".into())],
            &[],
            &now(),
            true,
        )
        .unwrap();
    cancel_batch(
        &f.backend,
        CancelBatch {
            batch_id: started.batch_id,
        },
    )
    .unwrap();
    assert!(target.exists());
}
#[test]
fn cached_cover_must_decode_not_only_have_a_valid_hash() {
    let f = Fixture::new();
    let bytes = b"\xff\xd8\xfftruncated image";
    let filename = format!("{:x}.jpg", Sha256::digest(bytes));
    fs::create_dir_all(f.backend.data_directory.join("covers")).unwrap();
    fs::write(
        f.backend.data_directory.join("covers").join(&filename),
        bytes,
    )
    .unwrap();
    assert!(validate_cover(&f.backend, &format!("covers/{filename}")).is_err());
}

#[test]
fn single_source_preparation_is_task_scoped_and_does_not_change_global_sources() {
    let f = Fixture::new();
    let original = metadata_sources::Config::default();
    metadata_sources::save(&f.backend, original.clone()).unwrap();
    let batch = begin_batch(&f.backend).unwrap();
    let mut request = f.request();
    request.manual = true;
    request.single_source = true;
    request.batch_id = Some(batch.batch_id.clone());
    let prepared = prepare_with(&f.backend, request, |stage, request| {
        assert_eq!(
            metadata_sources::get(stage).unwrap().enabled(),
            vec!["bangumi"]
        );
        fill(stage, request)
    })
    .unwrap();
    assert_eq!(
        metadata_sources::get(&f.backend).unwrap().enabled(),
        original.enabled()
    );
    assert_eq!(
        batch_config(&f.backend, &batch.batch_id).unwrap().enabled(),
        original.enabled()
    );
    let game = commit(&f.backend, f.commit_request(Some(prepared.preparation_id))).unwrap();
    assert!(game
        .metadata
        .iter()
        .all(|field| field.provider == "bangumi"));
}

#[test]
fn prepared_hikarinagi_bilingual_intro_commits_chinese_and_preserves_both_sources() {
    let f = Fixture::new();
    let mut request = f.request();
    request.provider = "hikarinagi".into();
    let prepared = prepare_with(&f.backend, request, |stage, request| {
        let cover = image(stage);
        stage.database()?.apply_remote_fields(
            &request.game_id,
            "hikarinagi",
            &request.remote_id,
            &[
                ("title".into(), "作品".into()),
                ("description".into(), "主人公は高校生。".into()),
                ("description_zh".into(), "主人公是一名高中生。".into()),
                ("cover_path".into(), cover),
            ],
            &[],
            &now(),
            true,
        )?;
        Ok(None)
    })
    .unwrap();
    assert_eq!(f.count(), 0);
    let detail = commit(&f.backend, f.commit_request(Some(prepared.preparation_id))).unwrap();
    assert_eq!(detail.description.as_deref(), Some("主人公是一名高中生。"));
    for name in ["description", "description_zh"] {
        assert!(detail
            .metadata
            .iter()
            .any(|field| field.provider == "hikarinagi" && field.field == name));
    }
}
