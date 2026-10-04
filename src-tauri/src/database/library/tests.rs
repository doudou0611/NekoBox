use super::*;
fn create(db: &mut Database, path: &str, title: &str) -> String {
    db.import_installation(path, title, None, InstallSource::Manual, &[], "f")
        .unwrap()
}

#[test]
fn imported_tags_obey_merged_limit_and_keep_user_tags() {
    let mut db = Database::in_memory().unwrap();
    let game = create(&mut db, "/fixture/Tags", "Tags");
    for (provider, tags) in [("bangumi", "A\nB"), ("vndb", "b\nC\nD")] {
        db.apply_remote_fields(
            &game,
            provider,
            "123",
            &[("source_tags".into(), tags.into())],
            &[],
            &backend::now(),
            false,
        )
        .unwrap();
    }
    db.set_metadata_priority(&game, &["bangumi".into(), "vndb".into()])
        .unwrap();
    db.replace_game_tags(&crate::backend::types::ReplaceGameTagsRequest {
        game_id: game.clone(),
        tag_names: vec!["User tag".into()],
    })
    .unwrap();
    db.limit_source_tags(&game, 3).unwrap();
    let summary = db.get_game(&game).unwrap().summary;
    assert_eq!(summary.source_tags, vec!["A", "B", "C"]);
    assert_eq!(
        summary
            .tags
            .iter()
            .map(|tag| tag.name.as_str())
            .collect::<Vec<_>>(),
        vec!["User tag"]
    );
}

#[test]
fn scraped_titles_follow_source_order_without_language_fallbacks() {
    let mut db = Database::in_memory().unwrap();
    let game = create(&mut db, "/fixture/Folder [Patch]", "Folder [Patch]");
    for (provider, title) in [
        ("vndb", "中文名"),
        ("bangumi", "日本語名"),
        ("hikarinagi", "English Title"),
    ] {
        db.apply_remote_fields(
            &game,
            provider,
            "123",
            &[("title".into(), title.into())],
            &[],
            &backend::now(),
            false,
        )
        .unwrap();
    }
    for (order, expected) in [
        (vec!["hikarinagi", "bangumi", "vndb"], "English Title"),
        (vec!["bangumi", "vndb"], "日本語名"),
        (vec!["vndb"], "中文名"),
    ] {
        db.set_metadata_priority(
            &game,
            &order.into_iter().map(String::from).collect::<Vec<_>>(),
        )
        .unwrap();
        assert_eq!(db.get_game(&game).unwrap().summary.title, expected);
        assert_eq!(
            db.list_games(&default_query()).unwrap().items[0].title,
            expected
        );
    }
}

#[test]
fn favorite_changes_do_not_lock_scraped_names_and_manual_rename_is_preserved() {
    let mut db = Database::in_memory().unwrap();
    let game = create(&mut db, "/fixture/Folder [Patch]", "Folder [Patch]");
    db.apply_vndb_match(
        &game,
        "v17",
        "Romanized Title",
        None,
        Some("English Title"),
        Some("日本語名"),
        None,
        None,
        None,
        None,
        &backend::now(),
    )
    .unwrap();
    let request = |title: &str| UpdateGameRequest {
        game_id: game.clone(),
        title: title.into(),
        status: GameStatus::NotStarted,
        favorite: true,
        hidden: false,
        user_rating: None,
    };
    let current_title = db.get_game(&game).unwrap().summary.title;
    let detail = db.update_game(&request(&current_title)).unwrap();
    assert!(!detail
        .metadata
        .iter()
        .any(|field| field.provider == "manual" && field.field == "title"));
    db.apply_remote_fields(
        &game,
        "bangumi",
        "123",
        &[("title_zh".into(), "中文名".into())],
        &[],
        &backend::now(),
        false,
    )
    .unwrap();
    assert_eq!(db.get_game(&game).unwrap().summary.title, "中文名");
    let renamed = db.update_game(&request("我自定义的名称")).unwrap();
    assert!(renamed
        .metadata
        .iter()
        .any(|field| field.provider == "manual"
            && field.field == "title"
            && field.manually_edited));
    db.unbind_metadata(&game, "bangumi").unwrap();
    db.unbind_metadata(&game, "vndb").unwrap();
    assert_eq!(db.get_game(&game).unwrap().summary.title, "我自定义的名称");
    assert_eq!(
        db.import_installation(
            "/fixture/Folder [Patch]",
            "Folder [Patch]",
            None,
            InstallSource::Local,
            &[],
            "new"
        )
        .unwrap(),
        game
    );
    let mut search = default_query();
    search.search = "Folder [Patch]".into();
    assert_eq!(
        db.list_games(&search).unwrap().items[0].title,
        "我自定义的名称"
    );

    // Legacy custom names were stored before manual/title provenance existed.
    let legacy = create(&mut db, "/fixture/Another Folder", "原有自定义名称");
    db.apply_remote_fields(
        &legacy,
        "vndb",
        "v18",
        &[("title_zh".into(), "远端中文名".into())],
        &[],
        &backend::now(),
        true,
    )
    .unwrap();
    assert_eq!(
        db.get_game(&legacy).unwrap().summary.title,
        "原有自定义名称"
    );

    // Long provider titles must not prevent unrelated status/favorite updates.
    let long_game = create(&mut db, "/fixture/Long Folder", "Long Folder");
    let long_title = "Long Provider Title ".repeat(12).trim().to_owned();
    db.apply_remote_fields(
        &long_game,
        "vndb",
        "v19",
        &[("title_en".into(), long_title.clone())],
        &[],
        &backend::now(),
        true,
    )
    .unwrap();
    let mut request = request(&long_title);
    request.game_id = long_game;
    assert!(db.update_game(&request).unwrap().summary.favorite);
    request.title = "New Manual Title ".repeat(13);
    assert!(db.update_game(&request).is_err());

    let rebound = create(&mut db, "/fixture/Rebind Folder", "Rebind Folder");
    db.apply_remote_fields(
        &rebound,
        "bangumi",
        "1",
        &[("title_zh".into(), "旧作品中文名".into())],
        &[("旧作品别名".into(), "zh".into())],
        &backend::now(),
        false,
    )
    .unwrap();
    db.apply_remote_fields(
        &rebound,
        "bangumi",
        "2",
        &[("title".into(), "新しい作品名".into())],
        &[],
        &backend::now(),
        false,
    )
    .unwrap();
    let detail = db.get_game(&rebound).unwrap();
    assert_eq!(detail.summary.title, "新しい作品名");
    assert!(detail.summary.title_zh.is_none());
    search.search = "旧作品别名".into();
    assert_eq!(db.list_games(&search).unwrap().total, 0);
}

#[test]
fn legacy_empty_priority_uses_settings_and_later_sources_only_fill_missing_facts() {
    let mut db = Database::in_memory().unwrap();
    let game = create(&mut db, "/fixture/source-order", "source-order");
    let mut config = backend::metadata_sources::Config::default();
    config.sources.swap(0, 2);
    db.put_setting(backend::metadata_sources::SETTING, &config)
        .unwrap();
    db.put_setting(&format!("metadata.priority.{game}"), &Vec::<String>::new())
        .unwrap();
    db.apply_remote_fields(
        &game,
        "vndb",
        "v1",
        &[
            ("title".into(), "English Title".into()),
            ("developer".into(), "Original Studio".into()),
            ("publisher".into(), "Original Publisher".into()),
            ("source_tags".into(), "Romance\nUnlisted Tag".into()),
            ("source_rating".into(), "7.1".into()),
            ("description".into(), "Original prose.".into()),
        ],
        &[],
        &backend::now(),
        false,
    )
    .unwrap();
    db.apply_remote_fields(
        &game,
        "bangumi",
        "2",
        &[
            ("title".into(), "中文标题".into()),
            ("developer".into(), "中文厂商".into()),
            ("publisher".into(), "中文发行商".into()),
            ("source_tags".into(), "恋爱".into()),
            ("source_rating".into(), "8.8".into()),
            ("description_zh".into(), "中文简介。".into()),
            ("release_date".into(), "2026-10-03".into()),
        ],
        &[],
        &backend::now(),
        false,
    )
    .unwrap();
    let detail = db.get_game(&game).unwrap();
    assert_eq!(detail.summary.title, "English Title");
    assert_eq!(detail.summary.developer.as_deref(), Some("Original Studio"));
    assert_eq!(
        detail.summary.publisher.as_deref(),
        Some("Original Publisher")
    );
    assert_eq!(detail.summary.source_rating, Some(7.1));
    assert_eq!(
        detail.summary.source_tags,
        vec!["Romance", "Unlisted Tag", "恋爱"]
    );
    assert_eq!(detail.summary.release_date.as_deref(), Some("2026-10-03"));
    assert_eq!(detail.description.as_deref(), Some("Original prose."));
}

#[test]
fn title_sort_activity_and_preferences_share_the_scraped_display_name() {
    use crate::backend::playtime::{SessionQuery, StatsQuery};
    let mut db = Database::in_memory().unwrap();
    let a = create(&mut db, "/fixture/Z Folder", "Z Folder");
    let b = create(&mut db, "/fixture/A Folder", "A Folder");
    for (game, title) in [(&a, "A Scraped Title"), (&b, "Z Scraped Title")] {
        db.apply_remote_fields(
            game,
            "vndb",
            "v17",
            &[("title_en".into(), title.into())],
            &[],
            &backend::now(),
            true,
        )
        .unwrap();
    }
    let mut q = default_query();
    q.sort = GameSort::Title;
    q.direction = SortDirection::Asc;
    assert_eq!(db.list_games(&q).unwrap().items[0].id, a);
    let install = db.get_game(&a).unwrap().summary.installations.remove(0).id;
    db.begin_session(
        &LaunchSession {
            session_id: "title-session".into(),
            game_id: a.clone(),
            install_id: install,
            started_at: backend::now(),
        },
        "game.exe",
    )
    .unwrap();
    db.end_session("title-session", &backend::now(), 30, "process_exit")
        .unwrap();
    assert_eq!(
        db.list_play_sessions(&SessionQuery {
            game_id: None,
            page: 1,
            page_size: 100
        })
        .unwrap()
        .items[0]
            .game_title,
        "A Scraped Title"
    );
    assert_eq!(
        db.playtime_stats(&StatsQuery {
            game_id: None,
            days: 7
        })
        .unwrap()
        .games[0]
            .title,
        "A Scraped Title"
    );
    db.set_recommendation_preference(&SetRecommendationPreferenceRequest {
        game_id: a,
        preference: RecommendationPreference::NotInterested,
        expires_at: None,
    })
    .unwrap();
    assert_eq!(
        db.recommendation_preferences().unwrap()[0].game_title,
        "A Scraped Title"
    );
}
#[test]
fn failed_launch_is_not_recent_play_and_scan_history_is_bounded() {
    let mut db = Database::in_memory().unwrap();
    let failed = create(&mut db, "/fixture/failed", "Failed");
    let played = create(&mut db, "/fixture/played", "Played");
    for (game, session, start, duration, reason) in [
        (&failed, "failed", backend::now(), 0, "launch_failed"),
        (
            &played,
            "played",
            (chrono::Utc::now() - chrono::Duration::hours(1))
                .to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            10,
            "process_exit",
        ),
    ] {
        let install = db
            .get_game(game)
            .unwrap()
            .summary
            .installations
            .remove(0)
            .id;
        db.begin_session(
            &LaunchSession {
                session_id: session.into(),
                game_id: game.clone(),
                install_id: install,
                started_at: start,
            },
            "game.exe",
        )
        .unwrap();
        db.end_session(session, &backend::now(), duration, reason)
            .unwrap();
    }
    assert!(db
        .get_game(&failed)
        .unwrap()
        .summary
        .last_played_at
        .is_none());
    let mut q = default_query();
    q.sort = GameSort::LastPlayedAt;
    assert_eq!(db.list_games(&q).unwrap().items[0].id, played);
    for n in 0..40 {
        db.put_setting(&format!("scan.task.{n}"), &serde_json::json!({"fixture":n}))
            .unwrap();
    }
    assert_eq!(
        db.connection
            .query_row(
                "SELECT count(*) FROM settings WHERE key LIKE 'scan.task.%'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        32
    );
    assert!(db
        .setting::<serde_json::Value>("scan.task.39")
        .unwrap()
        .is_some());
}
fn collection(name: &str) -> SaveCollectionRequest {
    SaveCollectionRequest {
        id: None,
        name: name.into(),
        kind: CollectionKind::Normal,
        icon: None,
        color: None,
        cover_url: None,
        position: 0,
        hidden: false,
        query: None,
        member_ids: None,
    }
}
#[test]
fn collection_name_and_members_commit_together_or_roll_back_together() {
    let mut db = Database::in_memory().unwrap();
    let a = create(&mut db, "/fixture/a", "A");
    let mut r = collection("新分组");
    r.member_ids = Some(vec!["missing".into()]);
    assert!(db.save_collection(&r).is_err());
    assert!(db.list_collections().unwrap().is_empty());
    r.member_ids = Some(vec![a.clone()]);
    let c = db.save_collection(&r).unwrap();
    assert_eq!(c.member_ids, vec![a.clone()]);
    r.id = Some(c.summary.id.clone());
    r.name = "改名".into();
    r.member_ids = Some(vec![a, "missing".into()]);
    assert!(db.save_collection(&r).is_err());
    let retained = db.get_collection(&c.summary.id).unwrap();
    assert_eq!(retained.summary.name, "新分组");
    assert_eq!(retained.member_ids.len(), 1);
}
#[test]
fn persists_real_records_without_fixtures_and_preserves_files() {
    let root = std::env::temp_dir().join(backend::id());
    std::fs::create_dir_all(&root).unwrap();
    let game = root.join("game");
    std::fs::create_dir_all(&game).unwrap();
    std::fs::write(game.join("keep.txt"), b"keep").unwrap();
    let db_path = root.join("db.sqlite3");
    let mut db = Database::open(&db_path).unwrap();
    assert_eq!(db.list_games(&default_query()).unwrap().total, 0);
    let id = create(&mut db, game.to_str().unwrap(), "测试作品");
    db.update_game(&UpdateGameRequest {
        game_id: id.clone(),
        title: "手动改名".into(),
        status: GameStatus::Paused,
        favorite: true,
        hidden: false,
        user_rating: None,
    })
    .unwrap();
    let c = db.save_collection(&collection("计划补完")).unwrap();
    db.replace_members(&c.summary.id, std::slice::from_ref(&id))
        .unwrap();
    drop(db);
    let mut db = Database::open(&db_path).unwrap();
    assert!(db.get_game(&id).unwrap().summary.favorite);
    assert_eq!(
        db.get_collection(&c.summary.id).unwrap().member_ids,
        vec![id.clone()]
    );
    assert!(db.delete_collection_record(&c.summary.id).unwrap());
    assert!(db.get_game(&id).is_ok());
    assert!(db.remove_game_record(&id).unwrap());
    assert_eq!(std::fs::read(game.join("keep.txt")).unwrap(), b"keep");
    drop(db);
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn query_filters_paginate_and_treat_sql_and_wildcards_as_data() {
    let mut db = Database::in_memory().unwrap();
    let a = create(&mut db, "/fixture/first", "A_100%");
    let b = create(&mut db, "/fixture/second", "另一个作品");
    db.update_game(&UpdateGameRequest {
        game_id: a.clone(),
        title: "A_100%".into(),
        status: GameStatus::Playing,
        favorite: true,
        hidden: false,
        user_rating: None,
    })
    .unwrap();
    let mut q = default_query();
    q.page_size = 1;
    assert_eq!(db.list_games(&q).unwrap().total, 2);
    q.page = 2;
    assert_eq!(db.list_games(&q).unwrap().items.len(), 1);
    q.search = "100%".into();
    q.page = 1;
    assert_eq!(db.list_games(&q).unwrap().items[0].id, a);
    q.search = "%' OR 1=1 --".into();
    assert_eq!(db.list_games(&q).unwrap().total, 0);
    q.search.clear();
    q.favorite = Some(true);
    q.statuses = vec![GameStatus::Playing];
    q.sources = vec![InstallSource::Manual];
    assert_eq!(db.list_games(&q).unwrap().total, 1);
    q.sources = vec![InstallSource::Local];
    assert_eq!(db.list_games(&q).unwrap().total, 0);
    q.page = 0;
    assert!(db.list_games(&q).is_err());
    assert!(db.get_game(&b).is_ok());
}
#[test]
fn member_replacement_is_atomic_and_smart_collections_are_computed() {
    let mut db = Database::in_memory().unwrap();
    let a = create(&mut db, "/fixture/a", "A");
    let b = create(&mut db, "/fixture/b", "B");
    let c = db.save_collection(&collection("普通")).unwrap();
    db.replace_members(&c.summary.id, std::slice::from_ref(&a))
        .unwrap();
    assert!(db
        .replace_members(&c.summary.id, &[b.clone(), "missing".into()])
        .is_err());
    assert_eq!(
        db.get_collection(&c.summary.id).unwrap().member_ids,
        vec![a.clone()]
    );
    let mut r = collection("收藏筛选");
    r.kind = CollectionKind::Smart;
    let mut q = default_query();
    q.favorite = Some(true);
    r.query = Some(q);
    let smart = db.save_collection(&r).unwrap();
    assert_eq!(smart.summary.game_count, 0);
    db.update_game(&UpdateGameRequest {
        game_id: a.clone(),
        title: "A".into(),
        status: GameStatus::NotStarted,
        favorite: true,
        hidden: false,
        user_rating: None,
    })
    .unwrap();
    let mut q = default_query();
    q.collection_id = Some(smart.summary.id.clone());
    assert_eq!(db.list_games(&q).unwrap().items[0].id, a);
    assert!(db.replace_members(&smart.summary.id, &[b]).is_err());
    r.query.as_mut().unwrap().collection_id = Some(c.summary.id);
    assert!(db.save_collection(&r).is_err());
}
#[test]
fn incremental_refresh_keeps_user_fields_and_installation_settings() {
    let mut db = Database::in_memory().unwrap();
    let a = create(&mut db, "/fixture/a", "原名");
    db.update_game(&UpdateGameRequest {
        game_id: a.clone(),
        title: "手动名称".into(),
        status: GameStatus::Completed,
        favorite: true,
        hidden: false,
        user_rating: None,
    })
    .unwrap();
    let i = db.get_game(&a).unwrap().summary.installations.remove(0).id;
    db.save_installation(&ConfigureInstallationRequest {
        install_id: i.clone(),
        executable_path: "/fixture/a/game.exe".into(),
        arguments: vec!["a b".into()],
        environment: Default::default(),
        working_directory: None,
        steam_app_id: None,
        main_process_name: Some("game.exe".into()),
        track_after_launcher_exit: true,
        idle_timeout_minutes: Some(10),
        use_locale_emulator: None,
        use_magpie: None,
    })
    .unwrap();
    assert_eq!(
        db.import_installation(
            "/fixture/a",
            "扫描名称",
            None,
            InstallSource::Local,
            &[],
            "new"
        )
        .unwrap(),
        a
    );
    assert_eq!(db.get_game(&a).unwrap().summary.title, "手动名称");
    assert_eq!(db.installation(&i).unwrap().arguments, vec!["a b"]);
    assert_eq!(
        db.installation(&i).unwrap().main_process_name.as_deref(),
        Some("game.exe")
    );
    assert!(db.installation(&i).unwrap().track_after_launcher_exit);
    assert_eq!(db.installation(&i).unwrap().idle_timeout_minutes, Some(10));
    let mut invalid: ConfigureInstallationRequest = serde_json::from_value(serde_json::json!({"install_id":i,"executable_path":"/fixture/a/new.exe","arguments":[],"working_directory":null,"environment":{},"idle_timeout_minutes":121})).unwrap();
    assert!(db.save_installation(&invalid).is_err());
    assert_eq!(
        db.installation(&i).unwrap().executable_path.as_deref(),
        Some("/fixture/a/game.exe")
    );
    invalid.idle_timeout_minutes = None;
    assert!(db
        .save_installation(&invalid)
        .unwrap()
        .idle_timeout_minutes
        .is_none());
    assert!(db.get_game(&a).unwrap().summary.favorite);
}
#[test]
fn recovery_marks_tasks_failed_and_never_invents_playtime() {
    let mut db = Database::in_memory().unwrap();
    let a = create(&mut db, "/fixture/a", "A");
    let i = db.get_game(&a).unwrap().summary.installations.remove(0).id;
    db.begin_session(
        &LaunchSession {
            session_id: "session".into(),
            game_id: a.clone(),
            install_id: i,
            started_at: backend::now(),
        },
        "game.exe",
    )
    .unwrap();
    db.put_setting(
        "scan.task.old",
        &ScanReport {
            progress: TaskProgress {
                task_id: "old".into(),
                status: TaskStatus::Running,
                phase: "walking".into(),
                processed: 10,
                total: None,
                message: "".into(),
            },
            imported: 1,
            unchanged: 0,
            issue_count: 0,
            issues: vec![],
            truncated: false,
        },
    )
    .unwrap();
    db.recover_interrupted().unwrap();
    assert!(!db.has_open_session(&a).unwrap());
    assert_eq!(db.get_game(&a).unwrap().summary.total_playtime_seconds, 0);
    assert_eq!(
        db.setting::<ScanReport>("scan.task.old")
            .unwrap()
            .unwrap()
            .progress
            .status,
        TaskStatus::Failed
    );
}
#[test]
fn recommendations_are_local_explainable_and_respect_preferences() {
    let mut db = Database::in_memory().unwrap();
    let first = create(&mut db, "/fixture/recommend-first", "First");
    let second = create(&mut db, "/fixture/recommend-second", "Second");
    db.update_game(&UpdateGameRequest {
        game_id: first.clone(),
        title: "First".into(),
        status: GameStatus::NotStarted,
        favorite: true,
        hidden: false,
        user_rating: None,
    })
    .unwrap();
    let recommendations = db
        .recommendations(&RecommendationQuery {
            limit: 5,
            excluded_game_ids: vec![],
        })
        .unwrap();
    assert_eq!(
        recommendations.first().map(|item| item.game_id.as_str()),
        Some(first.as_str())
    );
    assert!(recommendations
        .iter()
        .all(|item| matches!(item.source, RecommendationSource::Local)));
    db.set_recommendation_preference(&SetRecommendationPreferenceRequest {
        game_id: first.clone(),
        preference: RecommendationPreference::NotInterested,
        expires_at: None,
    })
    .unwrap();
    assert!(!db
        .recommendations(&RecommendationQuery {
            limit: 5,
            excluded_game_ids: vec![]
        })
        .unwrap()
        .iter()
        .any(|item| item.game_id == first));
    assert!(db
        .set_recommendation_preference(&SetRecommendationPreferenceRequest {
            game_id: second.clone(),
            preference: RecommendationPreference::Snoozed,
            expires_at: None
        })
        .is_err());
    let preferences = db.recommendation_preferences().unwrap();
    assert_eq!(preferences.len(), 1);
    assert_eq!(preferences[0].preference.game_id, first);
    assert_eq!(preferences[0].game_title, "First");
    db.set_recommendation_preference(&SetRecommendationPreferenceRequest {
        game_id: first.clone(),
        preference: RecommendationPreference::None,
        expires_at: None,
    })
    .unwrap();
    assert!(db
        .recommendations(&RecommendationQuery {
            limit: 5,
            excluded_game_ids: vec![first]
        })
        .unwrap()
        .iter()
        .all(|item| item.game_id != second));
}

#[test]
fn metadata_provider_binding_rebuilds_fields_and_unbinds_without_removing_game() {
    let mut db = Database::in_memory().unwrap();
    let game = create(&mut db, "/fixture/metadata", "Metadata");
    db.apply_remote_fields(
        &game,
        "vndb",
        "v123",
        &[
            ("title_zh".into(), "中文标题".into()),
            ("description".into(), "远端简介".into()),
        ],
        &[("Metadata Alias".into(), "und".into())],
        &backend::now(),
        true,
    )
    .unwrap();
    let detail = db.get_game(&game).unwrap();
    assert_eq!(detail.summary.title_zh.as_deref(), Some("中文标题"));
    assert!(detail
        .metadata
        .iter()
        .any(|field| field.provider == "vndb" && field.field == "description"));
    db.unbind_metadata(&game, "vndb").unwrap();
    let detail = db.get_game(&game).unwrap();
    assert!(detail.summary.title_zh.is_none());
    assert!(detail.metadata.iter().all(|field| field.provider != "vndb"));
    assert!(db.get_game(&game).is_ok());
}

#[test]
fn source_descriptions_follow_default_order_and_unbind_cleanly() {
    let mut db = Database::in_memory().unwrap();
    let game = create(&mut db, "/fixture/chinese-description", "中文作品");
    for (provider, remote_id, description) in [
        ("vndb", "v123", "女仆Scarlet遇到了Otori Sayumi。"),
        ("hikarinagi", "456", "女仆小春遇到了小夏。"),
        ("bangumi", "789", "女仆小春与小夏在宿舍共同生活。"),
    ] {
        db.apply_remote_fields(
            &game,
            provider,
            remote_id,
            &[("description_zh".into(), description.into())],
            &[],
            &backend::now(),
            false,
        )
        .unwrap();
        assert_eq!(
            db.get_game(&game).unwrap().description.as_deref(),
            Some(if provider == "vndb" {
                description
            } else {
                "女仆小春遇到了小夏。"
            })
        );
    }
    db.apply_remote_fields(
        &game,
        "vndb",
        "v123",
        &[("description_zh".into(), "新机器译文Scarlet。".into())],
        &[],
        &backend::now(),
        false,
    )
    .unwrap();
    assert_eq!(
        db.get_game(&game).unwrap().description.as_deref(),
        Some("女仆小春遇到了小夏。")
    );
    db.unbind_metadata(&game, "bangumi").unwrap();
    assert_eq!(
        db.get_game(&game).unwrap().description.as_deref(),
        Some("女仆小春遇到了小夏。")
    );
    db.unbind_metadata(&game, "hikarinagi").unwrap();
    assert_eq!(
        db.get_game(&game).unwrap().description.as_deref(),
        Some("新机器译文Scarlet。")
    );
}

#[test]
fn refreshing_a_legacy_automatic_description_replaces_old_mixed_language_text() {
    let mut db = Database::in_memory().unwrap();
    let game = create(&mut db, "/fixture/legacy-description", "中文作品");
    for (provider, remote_id, description) in [
        ("vndb", "v123", "Scarlet在宿舍生活。"),
        ("hikarinagi", "456", "小春在宿舍生活。"),
    ] {
        db.apply_remote_fields(
            &game,
            provider,
            remote_id,
            &[("description_zh".into(), description.into())],
            &[],
            &backend::now(),
            false,
        )
        .unwrap();
    }
    // Simulate a game row projected using the old VNDB-before-Hikarinagi order.
    db.connection
        .execute(
            "UPDATE games SET description=?1 WHERE id=?2",
            params!["Scarlet在宿舍生活。", game],
        )
        .unwrap();
    db.apply_remote_fields(
        &game,
        "hikarinagi",
        "456",
        &[("description_zh".into(), "小春和小夏共同生活。".into())],
        &[],
        &backend::now(),
        true,
    )
    .unwrap();
    assert_eq!(
        db.get_game(&game).unwrap().description.as_deref(),
        Some("小春和小夏共同生活。")
    );
}

#[test]
fn earlier_source_prose_overrides_later_language_preferences_and_preserves_manual() {
    let mut db = Database::in_memory().unwrap();
    let game = create(&mut db, "/fixture/manual-description", "中文作品");
    db.apply_remote_fields(
        &game,
        "vndb",
        "v123",
        &[("description_zh".into(), "中文译文。".into())],
        &[],
        &backend::now(),
        false,
    )
    .unwrap();
    db.apply_remote_fields(
        &game,
        "bangumi",
        "789",
        &[("description".into(), "主人公は高校生。".into())],
        &[],
        &backend::now(),
        false,
    )
    .unwrap();
    assert_eq!(
        db.get_game(&game).unwrap().description.as_deref(),
        Some("主人公は高校生。")
    );
    db.connection
        .execute(
            "UPDATE games SET description=?1 WHERE id=?2",
            params!["我自己写的简介。", game],
        )
        .unwrap();
    db.apply_remote_fields(
        &game,
        "bangumi",
        "789",
        &[("description_zh".into(), "来源中文简介。".into())],
        &[],
        &backend::now(),
        false,
    )
    .unwrap();
    assert_eq!(
        db.get_game(&game).unwrap().description.as_deref(),
        Some("我自己写的简介。")
    );
    db.unbind_metadata(&game, "bangumi").unwrap();
    assert_eq!(
        db.get_game(&game).unwrap().description.as_deref(),
        Some("我自己写的简介。")
    );
}

#[test]
fn source_order_is_independent_of_legacy_language_labels() {
    let mut db = Database::in_memory().unwrap();
    let game = create(&mut db, "/fixture/legacy-japanese", "中文作品");
    db.apply_remote_fields(
        &game,
        "hikarinagi",
        "456",
        &[("description_zh".into(), "主人公は高校生。".into())],
        &[],
        &backend::now(),
        false,
    )
    .unwrap();
    assert_eq!(
        db.get_game(&game).unwrap().description.as_deref(),
        Some("主人公は高校生。")
    );
    db.apply_remote_fields(
        &game,
        "vndb",
        "v123",
        &[("description_zh".into(), "主人公是一名高中生。".into())],
        &[],
        &backend::now(),
        false,
    )
    .unwrap();
    assert_eq!(
        db.get_game(&game).unwrap().description.as_deref(),
        Some("主人公は高校生。")
    );
    db.unbind_metadata(&game, "vndb").unwrap();
    assert_eq!(
        db.get_game(&game).unwrap().description.as_deref(),
        Some("主人公は高校生。")
    );
}

#[test]
fn advanced_smart_filters_round_trip_and_track_new_records() {
    let mut db = Database::in_memory().unwrap();
    let a = create(&mut db, "/fixture/filter-a", "A_%");
    let b = create(&mut db, "/fixture/filter-b", "B");
    db.connection.execute("UPDATE games SET developer='Studio',release_date='2024-03-01',cover_path='covers/fixture.jpg' WHERE id=?", [&a]).unwrap();
    db.connection
        .execute(
            "UPDATE games SET developer='Other',release_date='2023-03-01' WHERE id=?",
            [&b],
        )
        .unwrap();
    let mut q = default_query();
    q.filters.developer = Some("Studio".into());
    q.filters.release_year = Some(2024);
    q.filters.max_playtime_seconds = Some(0);
    q.filters.metadata_incomplete = Some(false);
    q.filters.has_save_backup = Some(false);
    assert_eq!(db.list_games(&q).unwrap().total, 1);
    let mut r = collection("规则完整保存");
    r.kind = CollectionKind::Smart;
    r.query = Some(q);
    let smart = db.save_collection(&r).unwrap();
    assert_eq!(
        smart.query.as_ref().unwrap().filters.release_year,
        Some(2024)
    );
    assert_eq!(smart.summary.game_count, 1);
    db.connection.execute("UPDATE games SET developer='Studio',release_date='2024-06-01',cover_path='covers/fixture.jpg' WHERE id=?", [&b]).unwrap();
    assert_eq!(
        db.get_collection(&smart.summary.id)
            .unwrap()
            .summary
            .game_count,
        2
    );
    let mut query = default_query();
    query.search = "A_%".into();
    assert_eq!(db.list_games(&query).unwrap().items[0].id, a);
    query.filters.min_playtime_seconds = Some(20);
    query.filters.max_playtime_seconds = Some(10);
    assert!(db.list_games(&query).is_err());
    let mut legacy = serde_json::to_value(default_query()).unwrap();
    legacy.as_object_mut().unwrap().remove("filters");
    assert!(serde_json::from_value::<GameQuery>(legacy).is_ok());
}

#[test]
fn collection_order_and_attributes_are_atomic_and_preserve_members() {
    let mut db = Database::in_memory().unwrap();
    let g = create(&mut db, "/fixture/order", "Game");
    db.connection
        .execute(
            "UPDATE games SET cover_path='covers/fixture.jpg' WHERE id=?",
            [&g],
        )
        .unwrap();
    let mut r = collection("A");
    r.icon = Some("heart".into());
    r.color = Some("#123456".into());
    r.cover_url = Some("covers/fixture.jpg".into());
    r.hidden = true;
    r.member_ids = Some(vec![g.clone()]);
    let a = db.save_collection(&r).unwrap();
    let b = db.save_collection(&collection("B")).unwrap();
    let c = db.save_collection(&collection("C")).unwrap();
    let before = db
        .list_collections()
        .unwrap()
        .into_iter()
        .map(|x| x.id)
        .collect::<Vec<_>>();
    assert!(db
        .reorder_collections(&[c.summary.id.clone(), "missing".into(), a.summary.id.clone()])
        .is_err());
    assert_eq!(
        db.list_collections()
            .unwrap()
            .into_iter()
            .map(|x| x.id)
            .collect::<Vec<_>>(),
        before
    );
    assert!(db
        .reorder_collections(&[
            a.summary.id.clone(),
            a.summary.id.clone(),
            c.summary.id.clone()
        ])
        .is_err());
    db.reorder_collections(&[
        c.summary.id.clone(),
        b.summary.id.clone(),
        a.summary.id.clone(),
    ])
    .unwrap();
    let retained = db.get_collection(&a.summary.id).unwrap();
    assert_eq!(retained.member_ids, vec![g]);
    assert_eq!(retained.summary.position, 2);
    assert!(retained.summary.hidden);
    assert_eq!(
        retained.summary.cover_url.as_deref(),
        Some("covers/fixture.jpg")
    );
    assert_eq!(retained.summary.icon.as_deref(), Some("heart"));
    r.cover_url = Some("https://untrusted.example/image.jpg".into());
    assert!(db.save_collection(&r).is_err());
}

#[test]
fn home_summary_counts_registered_save_errors() {
    let mut db = Database::in_memory().unwrap();
    let game = create(&mut db, "/fixture/save-issue", "存档作品");
    let install = db.get_game(&game).unwrap().summary.installations[0]
        .id
        .clone();
    db.connection.execute("INSERT INTO save_profiles(id,game_id,install_id,source_path,target_path) VALUES('issue-profile',?1,?2,'/fixture/save-issue/Save','save-backups/issue-profile')",rusqlite::params![game,install]).unwrap();
    db.put_setting("saves.error.issue-profile", &"备份失败")
        .unwrap();
    db.put_setting("saves.error.orphan", &"孤立错误不计数")
        .unwrap();
    assert_eq!(db.home_summary().unwrap().save_issue_count, 1);
    db.delete_setting("saves.error.issue-profile").unwrap();
    assert_eq!(db.home_summary().unwrap().save_issue_count, 0);
}

#[test]
fn hikarinagi_chinese_intro_wins_within_source_and_keeps_original() {
    let mut db = Database::in_memory().unwrap();
    let game = create(&mut db, "/fixture/hikari-bilingual", "中文作品");
    db.apply_remote_fields(
        &game,
        "hikarinagi",
        "456",
        &[
            ("description".into(), "主人公は高校生。".into()),
            ("description_zh".into(), "主人公是一名高中生。".into()),
        ],
        &[],
        &backend::now(),
        true,
    )
    .unwrap();
    let detail = db.get_game(&game).unwrap();
    assert_eq!(detail.description.as_deref(), Some("主人公是一名高中生。"));
    assert!(detail
        .metadata
        .iter()
        .any(|f| f.field == "description" && f.value.contains("主人公は高校生")));
    // A stored optional machine translation must not displace source Chinese.
    let translation = backend::translation::Settings {
        enabled: true,
        ..Default::default()
    };
    db.save_translation_config(&translation).unwrap();
    db.apply_translation_fields(
        &game,
        &[(
            "hikarinagi".into(),
            "description".into(),
            "description_zh_translation".into(),
            "机器生成的简介。".into(),
        )],
    )
    .unwrap();
    assert_eq!(
        db.get_game(&game).unwrap().description.as_deref(),
        Some("主人公是一名高中生。")
    );
    // A legacy automatic row must be corrected on explicit refresh.
    db.connection
        .execute(
            "UPDATE games SET description=? WHERE id=?",
            params!["主人公は高校生。", game],
        )
        .unwrap();
    db.apply_remote_fields(
        &game,
        "hikarinagi",
        "456",
        &[
            ("description".into(), "主人公は高校生。".into()),
            ("description_zh".into(), "主人公是一名高中生。".into()),
        ],
        &[],
        &backend::now(),
        true,
    )
    .unwrap();
    assert_eq!(
        db.get_game(&game).unwrap().description.as_deref(),
        Some("主人公是一名高中生。")
    );
}

#[test]
fn hikarinagi_chinese_intro_falls_back_and_preserves_source_order_and_manual() {
    for translated in ["", "   ", "彼女は同級生。", "A summer story."] {
        let mut db = Database::in_memory().unwrap();
        let game = create(&mut db, "/fixture/hikari-fallback", "中文作品");
        db.apply_remote_fields(
            &game,
            "hikarinagi",
            "456",
            &[
                ("description".into(), "主人公は高校生。".into()),
                ("description_zh".into(), translated.into()),
            ],
            &[],
            &backend::now(),
            true,
        )
        .unwrap();
        assert_eq!(
            db.get_game(&game).unwrap().description.as_deref(),
            Some("主人公は高校生。")
        );
    }
    let mut db = Database::in_memory().unwrap();
    let game = create(&mut db, "/fixture/hikari-order", "中文作品");
    db.set_metadata_priority(&game, &["vndb".into(), "hikarinagi".into()])
        .unwrap();
    db.apply_remote_fields(
        &game,
        "vndb",
        "v1",
        &[("description".into(), "Earlier source prose.".into())],
        &[],
        &backend::now(),
        true,
    )
    .unwrap();
    let fields = [
        ("description".into(), "主人公は高校生。".into()),
        ("description_zh".into(), "主人公是一名高中生。".into()),
    ];
    db.apply_remote_fields(
        &game,
        "hikarinagi",
        "456",
        &fields,
        &[],
        &backend::now(),
        true,
    )
    .unwrap();
    assert_eq!(
        db.get_game(&game).unwrap().description.as_deref(),
        Some("Earlier source prose.")
    );
    db.unbind_metadata(&game, "vndb").unwrap();
    assert_eq!(
        db.get_game(&game).unwrap().description.as_deref(),
        Some("主人公是一名高中生。")
    );
    db.connection
        .execute(
            "UPDATE games SET description=? WHERE id=?",
            params!["我自己写的简介。", game],
        )
        .unwrap();
    db.apply_remote_fields(
        &game,
        "hikarinagi",
        "456",
        &fields,
        &[],
        &backend::now(),
        true,
    )
    .unwrap();
    assert_eq!(
        db.get_game(&game).unwrap().description.as_deref(),
        Some("我自己写的简介。")
    );
}
