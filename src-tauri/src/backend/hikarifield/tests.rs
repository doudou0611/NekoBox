use super::*;
#[test]
fn import_owned_games_is_idempotent_and_preserves_personal_data() {
    let mut db = crate::database::Database::in_memory().unwrap();
    let app:App=serde_json::from_value(json!({"id":7,"tag":"fixture","name":"已购游戏","have":1,"released":1,"build_id":11,"version":"1.0","install_path":"Fixture","exec_file":"Fixture.exe"})).unwrap();
    assert!(db.import_hf_app(10, &app, None).unwrap());
    assert!(!db.import_hf_app(10, &app, None).unwrap());
    assert!(!db.import_hf_app(20, &app, None).unwrap());
    let games = db
        .list_games(&crate::domain::models::GameQuery {
            page: 1,
            page_size: 100,
            search: String::new(),
            statuses: vec![],
            sources: vec![],
            tag_ids: vec![],
            favorite: None,
            collection_id: None,
            sort: crate::domain::models::GameSort::Title,
            direction: crate::domain::models::SortDirection::Asc,
            filters: Default::default(),
        })
        .unwrap();
    assert_eq!(games.total, 1);
    let game = &games.items[0];
    assert!(db
        .get_game(&game.id)
        .unwrap()
        .metadata
        .iter()
        .any(|m| m.provider == "hikarifield" && m.field == "title"));
    db.update_game(&crate::backend::types::UpdateGameRequest {
        game_id: game.id.clone(),
        title: "我的自定义标题".into(),
        status: crate::domain::protocol::GameStatus::Playing,
        favorite: true,
        hidden: false,
        user_rating: Some(8.5),
    })
    .unwrap();
    assert!(!db.import_hf_app(10, &app, None).unwrap());
    let updated = db.get_game(&game.id).unwrap();
    assert_eq!(updated.summary.title, "我的自定义标题");
    assert!(updated.summary.favorite);
    assert_eq!(updated.summary.user_rating, Some(8.5));
    assert_eq!(
        updated.summary.status,
        crate::domain::protocol::GameStatus::Playing
    );
    assert!(game.installations.is_empty());
    assert_eq!(game.hikari_field.as_ref().unwrap().app_id, 7);
    assert_eq!(db.hf_ownership(&game.id).unwrap().owners, vec![10, 20]);
    assert!(updated
        .metadata
        .iter()
        .all(|m| matches!(m.provider.as_str(), "manual" | "hikarifield")));
}
#[test]
fn owned_games_survive_database_reopen_without_an_account_token() {
    let fixture = std::env::temp_dir().join(format!("hf-owned-{}", id()));
    let b = Backend::open(fixture.join("data")).unwrap();
    let app: App = serde_json::from_value(
        json!({"id":7,"tag":"fixture","name":"已购游戏","have":1,"released":1}),
    )
    .unwrap();
    b.database().unwrap().import_hf_app(10, &app, None).unwrap();
    drop(b);
    let reopened = Backend::open(fixture.join("data")).unwrap();
    assert!(reopened.database().unwrap().hf_needs_cover(7).unwrap());
    // No vault is accessed when loading library ownership.
    let app_again = reopened
        .database()
        .unwrap()
        .import_hf_app(10, &app, None)
        .unwrap();
    assert!(!app_again);
    drop(reopened);
    std::fs::remove_dir_all(fixture).unwrap();
}
#[test]
fn api_uses_client_routes_json_and_bearer_without_leaking_credentials() {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    let server = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = server.local_addr().unwrap();
    let task = std::thread::spawn(move || {
        let (mut stream, _) = server.accept().unwrap();
        let mut buf = [0; 8192];
        let n = stream.read(&mut buf).unwrap();
        let text = String::from_utf8_lossy(&buf[..n]);
        assert!(text.starts_with("POST /v1/apps HTTP/1.1"));
        assert!(text
            .to_lowercase()
            .contains("authorization: bearer fixture"));
        assert!(text.to_lowercase().contains(&format!(
            "user-agent: nekobox/{}",
            env!("CARGO_PKG_VERSION")
        )));
        let body = "[]";
        write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{body}").unwrap();
    });
    let v = send_at(
        &format!("http://{addr}/v1/"),
        reqwest::Method::POST,
        "apps",
        json!({"category_id":3}),
        Some("fixture"),
    )
    .unwrap();
    assert!(v.is_array());
    task.join().unwrap();
    assert!(token_from(&json!({"access_token":"bad\ntoken"})).is_err());
}

#[test]
fn gateway_rejection_is_distinguished_from_account_or_download_permissions() {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    let server = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = server.local_addr().unwrap();
    let task = std::thread::spawn(move || {
        let (mut stream, _) = server.accept().unwrap();
        let mut buf = [0; 8192];
        assert!(stream.read(&mut buf).unwrap() > 0);
        let body = "<html><h1>403 Forbidden</h1></html>";
        write!(stream, "HTTP/1.1 403 Forbidden\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
    });
    let error = send_at(
        &format!("http://{addr}/v1/"),
        reqwest::Method::POST,
        "auth/login",
        json!({"email":"","password":""}),
        None,
    )
    .unwrap_err();
    assert_eq!(error.0, ErrorCode::PermissionDenied);
    assert!(error.1.contains("网络网关"));
    assert!(!error.1.contains("购买权限"));
    assert!(!error.1.contains("下载额度"));
    task.join().unwrap();
}

#[test]
#[ignore = "requires official network access; only submits empty login fields, never account credentials"]
fn live_empty_login_reaches_api_validation_with_identified_client() {
    let response = http()
        .unwrap()
        .post(format!("{BASE}auth/login"))
        .header("Accept", "application/json")
        .json(&json!({"email":"","password":""}))
        .send()
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::UNPROCESSABLE_ENTITY);
    let value: Value = response.json().unwrap();
    assert!(value["errors"]["email"].is_array());
    assert!(value["errors"]["password"].is_array());
}

#[test]
fn only_owned_full_games_are_parsed_and_unowned_nullable_fields_do_not_break_sync() {
    let apps = parse_apps(json!([
    {"id":1,"tag":"owned","name":"正式版","have":1,"released":1},
    {"id":2,"tag":"unowned","name":"未购买","have":0,"build_id":null,"version":null},
    {"id":3,"tag":"trial","name":"试玩","have":1,"trial":1},
    {"id":4,"tag":"audio","name":"音声","have":1,"asmr":1},
        {"id":5,"tag":"upcoming","name":"尚未开放下载","have":1,"released":0,"build_id":null,"version":null,"install_path":null,"exec_file":null,"depots":null}
    ]))
    .unwrap();
    assert_eq!(apps.len(), 2);
    assert_eq!(apps[0].id, 1);
}
#[test]
fn folder_selection_appends_root_once_and_persists_stable_uuid() {
    let fixture = std::env::temp_dir().join(format!("hf-root-{}", id()));
    let b = Backend::open(fixture.join("data")).unwrap();
    let parent = fixture.join("games");
    std::fs::create_dir_all(&parent).unwrap();
    let first = save_path(
        &b,
        PathRequest {
            parent: parent.to_string_lossy().into(),
        },
    )
    .unwrap();
    assert!(first.root.as_ref().unwrap().ends_with("HikariFieldGames"));
    let again = save_path(
        &b,
        PathRequest {
            parent: first.root.clone().unwrap(),
        },
    )
    .unwrap();
    assert_eq!(first.root, again.root);
    assert_eq!(first.uuid, again.uuid);
    assert_eq!(settings(&b).unwrap().root, first.root);
    drop(b);
    std::fs::remove_dir_all(fixture).unwrap();
}

#[test]
fn official_metadata_is_the_baseline_and_ordered_scrapers_only_fill_missing_fields() {
    let mut db = crate::database::Database::in_memory().unwrap();
    let app: App = serde_json::from_value(
        json!({"id":7,"tag":"fixture","name":"官方名称","have":1,"released":1}),
    )
    .unwrap();
    db.import_hf_app(10, &app, None).unwrap();
    let query = crate::domain::models::GameQuery {
        page: 1,
        page_size: 100,
        search: String::new(),
        statuses: vec![],
        sources: vec![],
        tag_ids: vec![],
        favorite: None,
        collection_id: None,
        sort: crate::domain::models::GameSort::Title,
        direction: crate::domain::models::SortDirection::Asc,
        filters: Default::default(),
    };
    let game = db.list_games(&query).unwrap().items[0].id.clone();
    db.set_hf_cover(7, "covers/official.jpg").unwrap();
    assert_eq!(
        db.get_game(&game).unwrap().summary.metadata_status,
        crate::domain::protocol::MetadataStatus::LocalOnly
    );
    let config = crate::backend::metadata_sources::Config {
        sources: vec![
            crate::backend::metadata_sources::Source {
                provider: "bangumi".into(),
                enabled: true,
            },
            crate::backend::metadata_sources::Source {
                provider: "hikarinagi".into(),
                enabled: true,
            },
            crate::backend::metadata_sources::Source {
                provider: "vndb".into(),
                enabled: false,
            },
        ],
    };
    db.put_setting(crate::backend::metadata_sources::SETTING, &config)
        .unwrap();
    db.set_metadata_priority(&game, &config.enabled()).unwrap();
    db.apply_remote_fields(
        &game,
        "hikarinagi",
        "789",
        &[
            ("title".into(), "其他来源名称".into()),
            ("cover_path".into(), "covers/other.jpg".into()),
            ("description".into(), "第二来源的简介。".into()),
            ("developer".into(), "第二来源开发商".into()),
            ("release_date".into(), "2020-01-01".into()),
        ],
        &[],
        &now(),
        true,
    )
    .unwrap();
    db.apply_remote_fields(
        &game,
        "bangumi",
        "123",
        &[
            ("title".into(), "第一来源名称".into()),
            ("cover_path".into(), "covers/first.jpg".into()),
            ("description".into(), "第一来源的简介。".into()),
            ("developer".into(), "第一来源开发商".into()),
        ],
        &[],
        &now(),
        true,
    )
    .unwrap();
    let detail = db.get_game(&game).unwrap();
    assert_eq!(detail.summary.title, "官方名称");
    assert_eq!(
        detail.summary.cover_url.as_deref(),
        Some("covers/official.jpg")
    );
    assert_eq!(detail.summary.developer.as_deref(), Some("第一来源开发商"));
    assert_eq!(detail.summary.release_date.as_deref(), Some("2020-01-01"));
    assert_eq!(detail.description.as_deref(), Some("第一来源的简介。"));
    assert!(detail.summary.installations.is_empty());
    db.edit_metadata(&crate::backend::detail_metadata::EditRequest {
        game_id: game.clone(),
        changes: [("description".into(), json!("我自己的简介"))]
            .into_iter()
            .collect(),
        expected: [("description".into(), json!("第一来源的简介。"))]
            .into_iter()
            .collect(),
    })
    .unwrap();
    db.apply_remote_fields(
        &game,
        "bangumi",
        "123",
        &[("description".into(), "刷新后的简介".into())],
        &[],
        &now(),
        false,
    )
    .unwrap();
    assert_eq!(
        db.get_game(&game).unwrap().description.as_deref(),
        Some("我自己的简介")
    );
    db.freeze_metadata_order(&game).unwrap();
    db.freeze_metadata_order(&game).unwrap();
    let order: Vec<String> = db
        .setting(&format!("metadata.priority.{game}"))
        .unwrap()
        .unwrap();
    assert_eq!(order.iter().filter(|p| *p == "hikarifield").count(), 1);
    db.put_setting(&format!("metadata.locked.{game}"), &true)
        .unwrap();
    let before = serde_json::to_value(db.get_game(&game).unwrap().metadata).unwrap();
    let mut changed = app.clone();
    changed.name = "修改后的官方名称".into();
    db.import_hf_app(10, &changed, None).unwrap();
    db.set_hf_cover(7, "covers/replacement.jpg").unwrap();
    assert_eq!(
        serde_json::to_value(db.get_game(&game).unwrap().metadata).unwrap(),
        before
    );
}
