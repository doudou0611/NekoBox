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
    assert!(db.get_game(&game.id).unwrap().metadata.is_empty());
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
    assert!(updated.metadata.iter().all(|m| m.provider == "manual"));
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
