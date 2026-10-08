use super::*;
use crate::backend::types::ConfigureInstallationRequest;
use sha2::{Digest, Sha256};
use std::fs;

#[test]
fn parses_localized_fields_and_rejects_wrong_identity() {
    let data = serde_json::json!({"123":{"success":true,"data":{"steam_appid":123,"name":"作品","short_description":"<p>中文 &amp; 简介</p>","developers":["开发商"],"publishers":["发行商"],"release_date":{"date":"2024年7月13日"},"metacritic":{"score":85}}}});
    let fields = fields(details(&data, "123").unwrap());
    assert!(fields.contains(&("description".into(), "中文 & 简介".into())));
    assert!(fields.contains(&("release_date".into(), "2024-07-13".into())));
    assert!(fields.contains(&("source_rating".into(), "8.50".into())));
    assert!(details(&data, "999").is_err());
    assert!(details(&serde_json::json!({"123":{"success":false}}), "123").is_err());
}
#[test]
fn modern_covers_use_hashed_assets_with_trusted_host_only() {
    let item = serde_json::json!({"assets":{"asset_url_format":"steam/apps/123/abcd/${FILENAME}","library_capsule":"library.jpg","library_capsule_2x":"https://example.com/library.jpg"}});
    assert_eq!(asset_covers(&item,"123"), vec!["https://shared.akamai.steamstatic.com/store_item_assets/steam/apps/123/abcd/library.jpg"]);
    assert!(!trusted_steam_image(
        "https://cdn.akamai.steamstatic.com.evil.test/steam/apps/123/a.jpg"
    ));
    assert!(!trusted_steam_image(
        "https://user@cdn.akamai.steamstatic.com/steam/apps/123/a.jpg"
    ));
}
fn fixture() -> (PathBuf, Backend, String) {
    let root = std::env::temp_dir().join(format!("nb-steam-{}", id()));
    let directory = root.join("Steam/steamapps/common/作品");
    fs::create_dir_all(&directory).unwrap();
    fs::write(
        root.join("Steam/steamapps/appmanifest_123.acf"),
        r#""AppState" { "appid" "123" "name" "本地作品" "installdir" "作品" "StateFlags" "4" }"#,
    )
    .unwrap();
    let backend = Backend::open(root.join("data")).unwrap();
    let directory = path_text(&directory.canonicalize().unwrap()).unwrap();
    (root, backend, directory)
}
fn prepared(b: &Backend, directory: &str, batch: &str) -> import_metadata::Preparation {
    import_metadata::prepare_with(
        b,
        import_metadata::PrepareRequest {
            manual: false,
            single_source: true,
            title_hint: None,
            batch_id: Some(batch.into()),
            directory: directory.into(),
            provider: "steam".into(),
            remote_id: "123".into(),
        },
        |stage, q| {
            let mut encoded = std::io::Cursor::new(Vec::new());
            image::DynamicImage::new_rgb8(2, 2)
                .write_to(&mut encoded, image::ImageFormat::Png)
                .unwrap();
            let bytes = encoded.into_inner();
            let filename = format!("{:x}.png", Sha256::digest(&bytes));
            fs::create_dir_all(stage.data_directory.join("covers")).unwrap();
            let relative = format!("covers/{filename}");
            import_metadata::persist_cover(
                stage,
                &stage.data_directory.join(&relative),
                &bytes,
                &relative,
            )?;
            stage
                .database()?
                .set_metadata_priority(&q.game_id, &["steam".into()])?;
            stage.database()?.apply_remote_fields(
                &q.game_id,
                "steam",
                "123",
                &[
                    ("title".into(), "Steam 中文作品".into()),
                    ("description".into(), "Steam 简介".into()),
                    ("developer".into(), "Steam 开发商".into()),
                    ("cover_path".into(), relative),
                ],
                &[],
                &now(),
                true,
            )?;
            bind_community(stage, &q.game_id, "456")?;
            Ok(Some("已关联安利墙".into()))
        },
    )
    .unwrap()
}
#[test]
fn steam_preparation_is_isolated_and_commit_preserves_primary_metadata_and_identity() {
    let (root, b, directory) = fixture();
    let batch = import_metadata::begin_batch(&b).unwrap().batch_id;
    let prep = prepared(&b, &directory, &batch);
    assert_eq!(b.database().unwrap().home_summary().unwrap().game_count, 0);
    assert!(import_metadata::commit(
        &b,
        import_metadata::CommitRequest {
            directory: directory.clone(),
            title: "wrong channel".into(),
            executable_path: None,
            preparation_id: Some(prep.preparation_id.clone())
        }
    )
    .is_err());
    let game = import(
        &b,
        ImportRequest {
            app_id: "123".into(),
            directory: directory.clone(),
            preparation_id: Some(prep.preparation_id.clone()),
        },
    )
    .unwrap()
    .game;
    assert_eq!(game.summary.title, "Steam 中文作品");
    assert_eq!(game.description.as_deref(), Some("Steam 简介"));
    assert_eq!(game.summary.developer.as_deref(), Some("Steam 开发商"));
    assert!(game.summary.installations[0].can_launch());
    assert_eq!(
        game.summary.installations[0].steam_app_id.as_deref(),
        Some("123")
    );
    assert!(matches!(
        game.summary.installations[0].source,
        crate::domain::protocol::InstallSource::Steam
    ));
    assert!(game.summary.installations[0].executable_path.is_none());
    let request: ConfigureInstallationRequest = serde_json::from_value(serde_json::json!({"install_id":game.summary.installations[0].id,"executable_path":"","steam_app_id":"123","arguments":["--windowed"],"working_directory":null,"environment":{}})).unwrap();
    let configured = b.configure_installation(&request).unwrap();
    assert!(configured.executable_path.is_none());
    assert_eq!(configured.arguments, vec!["--windowed"]);
    assert_eq!(configured.use_locale_emulator, Some(false));
    let mut wrong = request;
    wrong.steam_app_id = Some("999".into());
    assert!(b.configure_installation(&wrong).is_err());
    metadata_sources::confirm(
        &b,
        &ConfirmMetadataMatchRequest {
            game_id: game.summary.id.clone(),
            provider: "hikarinagi".into(),
            remote_id: "789".into(),
            manual: true,
            title_hint: None,
        },
    )
    .unwrap();
    let rebound = b.database().unwrap().get_game(&game.summary.id).unwrap();
    assert_eq!(rebound.description, game.description);
    assert_eq!(rebound.summary.title, game.summary.title);
    let sources = b
        .database()
        .unwrap()
        .list_external_sources(&game.summary.id)
        .unwrap();
    assert!(sources
        .iter()
        .any(|s| s.provider == "hikarinagi" && s.remote_id.as_deref() == Some("789")));
    assert_eq!(
        metadata_sources::get(&b).unwrap().enabled(),
        vec!["hikarinagi"]
    );
    assert!(import(
        &b,
        ImportRequest {
            app_id: "123".into(),
            directory: directory.clone(),
            preparation_id: None
        }
    )
    .is_err());
    let scan = scan::scan(
        &b,
        scan::ScanRequest {
            steam_path: Some(root.join("Steam").to_string_lossy().into_owned()),
        },
    )
    .unwrap();
    assert_eq!(
        scan.games[0].existing_game_id.as_deref(),
        Some(game.summary.id.as_str())
    );
    import_metadata::cancel_batch(&b, import_metadata::CancelBatch { batch_id: batch }).unwrap();
    assert!(b.data_directory.join(prep.cover_path).is_file());
    drop(b);
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn cancelled_steam_preparation_cannot_be_committed() {
    let (root, b, directory) = fixture();
    let batch = import_metadata::begin_batch(&b).unwrap().batch_id;
    let prep = prepared(&b, &directory, &batch);
    import_metadata::cancel_batch(&b, import_metadata::CancelBatch { batch_id: batch }).unwrap();
    assert!(import(
        &b,
        ImportRequest {
            app_id: "123".into(),
            directory,
            preparation_id: Some(prep.preparation_id)
        }
    )
    .is_err());
    assert_eq!(b.database().unwrap().home_summary().unwrap().game_count, 0);
    assert!(!b.data_directory.join(prep.cover_path).exists());
    drop(b);
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn multiple_libraries_deduplicate_ids_and_show_corrupt_manifest_warnings_without_writes() {
    let (root, b, _) = fixture();
    let extra = root.join("SteamLibrary");
    fs::create_dir_all(extra.join("steamapps/common/Second")).unwrap();
    fs::write(extra.join("steamapps/appmanifest_124.acf"),r#""AppState" { "appid" "124" "name" "Other Genre" "installdir" "Second" "StateFlags" "4" }"#).unwrap();
    fs::write(extra.join("steamapps/appmanifest_125.acf"),r#""AppState" { "appid" "125" "name" "Downloading" "installdir" "Second" "StateFlags" "2" }"#).unwrap();
    fs::write(extra.join("steamapps/appmanifest_126.acf"), "bad {").unwrap();
    fs::write(
        root.join("Steam/steamapps/libraryfolders.vdf"),
        format!(
            r#""libraryfolders" {{ "0" {{ "path" "{}" }} "1" "{}" "2" {{ "path" "{}" }} }}"#,
            root.join("Steam").display(),
            extra.display(),
            extra.display()
        ),
    )
    .unwrap();
    let result = scan::scan(
        &b,
        scan::ScanRequest {
            steam_path: Some(root.join("Steam").to_string_lossy().into_owned()),
        },
    )
    .unwrap();
    assert_eq!(result.library_count, 2);
    assert_eq!(result.games.len(), 2);
    assert_eq!(result.warnings.len(), 1);
    assert!(result.games.iter().any(|g| g.name == "Other Genre"));
    assert_eq!(b.database().unwrap().home_summary().unwrap().game_count, 0);
    drop(b);
    fs::remove_dir_all(root).unwrap();
}
#[test]
#[ignore = "requires live Steam public APIs; isolated temporary library only"]
fn live_steam_metadata_uses_store_cover_and_never_writes_real_library() {
    let (root, b, directory) = fixture();
    let game = b
        .database()
        .unwrap()
        .import_installation(
            &directory,
            "作品",
            None,
            crate::domain::protocol::InstallSource::Manual,
            &[],
            "",
        )
        .unwrap();
    confirm(
        &b,
        &ConfirmMetadataMatchRequest {
            game_id: game.clone(),
            provider: "steam".into(),
            remote_id: "1230140".into(),
            manual: false,
            title_hint: None,
        },
        None,
        false,
    )
    .unwrap();
    let detail = b.database().unwrap().get_game(&game).unwrap();
    assert!(detail.summary.title.to_lowercase().contains("atri"));
    assert!(detail.description.as_deref().is_some_and(|s| !s.is_empty()));
    assert!(b
        .data_directory
        .join(detail.summary.cover_url.unwrap())
        .is_file());
    assert!(detail.metadata.iter().all(|f| f.provider == "steam"));
    assert!(
        !modern_covers(&b, &client().unwrap(), "1230140")
            .unwrap()
            .is_empty(),
        "modern StoreBrowse capsule available"
    );
    println!(
        "Steam live metadata and cached cover verified: {}",
        detail.summary.title
    );
    drop(b);
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn only_unique_exact_community_names_are_bound() {
    let candidate = |id: &str, title: &str| {
        serde_json::from_value::<crate::domain::models::MetadataCandidate>(serde_json::json!({"provider":"hikarinagi","remote_id":id,"title":title,"subtitle":null,"cover_url":null,"confidence":1.0,"matched_fields":["title"],"explanation":"fixture","fetched_at":now(),"cached":false})).unwrap()
    };
    let aliases = vec!["ATRI -My Dear Moments-".into()];
    assert_eq!(
        unique_community_id(&aliases, &[candidate("897", "ATRI My Dear Moments")]).as_deref(),
        Some("897")
    );
    assert!(unique_community_id(&aliases, &[candidate("897", "ATRI fan disc")]).is_none());
    assert!(unique_community_id(
        &aliases,
        &[
            candidate("897", "ATRI My Dear Moments"),
            candidate("898", "ATRI My Dear Moments")
        ]
    )
    .is_none());
    assert_eq!(
        unique_community_id(
            &aliases,
            &[
                candidate("897", "ATRI My Dear Moments"),
                candidate("897", "ATRI My Dear Moments")
            ]
        )
        .as_deref(),
        Some("897")
    );
}
#[test]
fn local_only_steam_entry_can_later_receive_store_data_without_freezing_manifest_title() {
    let (root, b, directory) = fixture();
    let game = import(
        &b,
        ImportRequest {
            app_id: "123".into(),
            directory,
            preparation_id: None,
        },
    )
    .unwrap()
    .game;
    assert!(game.summary.installations[0].can_launch());
    b.database()
        .unwrap()
        .apply_remote_fields(
            &game.summary.id,
            "steam",
            "123",
            &[
                ("title".into(), "商店新标题".into()),
                ("description".into(), "商店简介".into()),
            ],
            &[],
            &now(),
            true,
        )
        .unwrap();
    assert_eq!(
        b.database()
            .unwrap()
            .get_game(&game.summary.id)
            .unwrap()
            .summary
            .title,
        "商店新标题"
    );
    drop(b);
    fs::remove_dir_all(root).unwrap();
}
