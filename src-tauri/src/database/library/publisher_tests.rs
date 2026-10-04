use super::*;

fn game(db: &mut Database) -> String {
    db.import_installation(
        "/fixture/publisher",
        "发行商测试",
        None,
        InstallSource::Manual,
        &[],
        "fixture",
    )
    .unwrap()
}

fn apply(db: &mut Database, game: &str, provider: &str, remote_id: &str, publisher: &str) {
    db.apply_remote_fields(
        game,
        provider,
        remote_id,
        &[("publisher".into(), publisher.into())],
        &[],
        &backend::now(),
        true,
    )
    .unwrap();
}

#[test]
fn publisher_projection_prefers_sources_and_skips_empty_or_invalid_values() {
    let mut db = Database::in_memory().unwrap();
    let game = game(&mut db);
    db.set_metadata_priority(
        &game,
        &["bangumi".into(), "hikarinagi".into(), "vndb".into()],
    )
    .unwrap();
    // Results must be independent of insertion order and SQL row ordering.
    for (provider, remote_id, publisher) in [
        ("vndb", "v17", "VNDB Publisher"),
        ("bangumi", "123", "Bangumi Publisher"),
        ("hikarinagi", "456", "Hikarinagi Publisher"),
    ] {
        apply(&mut db, &game, provider, remote_id, publisher);
    }
    assert_eq!(
        db.get_game(&game).unwrap().summary.publisher.as_deref(),
        Some("Bangumi Publisher")
    );
    for value in ["\"  \"", "null", "{}"] {
        db.connection.execute(
            "UPDATE metadata_records SET value_json=?1 WHERE game_id=?2 AND provider='bangumi' AND field_name='publisher'",
            params![value, game],
        ).unwrap();
        assert_eq!(
            db.get_game(&game).unwrap().summary.publisher.as_deref(),
            Some("Hikarinagi Publisher")
        );
    }
    db.unbind_metadata(&game, "hikarinagi").unwrap();
    assert_eq!(
        db.get_game(&game).unwrap().summary.publisher.as_deref(),
        Some("VNDB Publisher")
    );
    db.unbind_metadata(&game, "vndb").unwrap();
    assert!(db.get_game(&game).unwrap().summary.publisher.is_none());
}

#[test]
fn refreshing_and_rebinding_publishers_preserve_manual_fields() {
    let mut db = Database::in_memory().unwrap();
    let game = game(&mut db);
    apply(&mut db, &game, "vndb", "v17", "Old Publisher");
    apply(&mut db, &game, "vndb", "v18", "New Publisher");
    assert_eq!(
        db.get_game(&game).unwrap().summary.publisher.as_deref(),
        Some("New Publisher")
    );
    db.connection.execute(
        "UPDATE metadata_records SET value_json='\"手工发行商\"',is_user_edited=1 WHERE game_id=? AND provider='vndb' AND field_name='publisher'",
        [&game],
    ).unwrap();
    apply(&mut db, &game, "bangumi", "123", "Automatic Publisher");
    apply(&mut db, &game, "vndb", "v18", "Refreshed Publisher");
    assert_eq!(
        db.get_game(&game).unwrap().summary.publisher.as_deref(),
        Some("手工发行商")
    );
    db.unbind_metadata(&game, "vndb").unwrap();
    assert_eq!(
        db.get_game(&game).unwrap().summary.publisher.as_deref(),
        Some("手工发行商")
    );
    assert!(db
        .get_game(&game)
        .unwrap()
        .metadata
        .iter()
        .any(|field| { field.field == "publisher" && field.manually_edited }));
}
