use super::{migrations::*, *};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

const NOW: &str = "2026-01-02T03:04:05Z";
const SHA: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn game(connection: &Connection, id: &str) {
    connection
        .execute(
            "INSERT INTO games (id, title) VALUES (?1, ?2)",
            params![id, format!("Fixture {id}")],
        )
        .unwrap();
}

fn install(connection: &Connection, id: &str, game_id: &str) {
    connection.execute("INSERT INTO game_installations (id, game_id, absolute_path, folder_name) VALUES (?1, ?2, ?3, 'fixture')", params![id, game_id, format!("C:/fixture/{id}")]).unwrap();
}

fn library() -> Database {
    let database = Database::in_memory().unwrap();
    game(&database.connection, "g1");
    game(&database.connection, "g2");
    install(&database.connection, "i1", "g1");
    install(&database.connection, "i2", "g1");
    install(&database.connection, "i3", "g2");
    database
}

fn profile(connection: &Connection) {
    connection.execute("INSERT INTO save_profiles (id, game_id, install_id, source_path, target_path) VALUES ('p1', 'g1', 'i1', 'C:/fixture/save', 'C:/fixture/backups')", []).unwrap();
}

fn collections(connection: &Connection) {
    connection.execute_batch("INSERT INTO smart_filters (id, name, query_json) VALUES ('f1', 'Unplayed', '{\"status\":\"not_started\"}');
        INSERT INTO collections (id, name, kind) VALUES ('c1', 'Favorites', 'normal');
        INSERT INTO collections (id, name, kind, smart_filter_id) VALUES ('c2', 'Smart', 'smart', 'f1');").unwrap();
}

fn count(connection: &Connection, table: &str) -> i64 {
    connection
        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
            row.get(0)
        })
        .unwrap()
}

fn exists(connection: &Connection, table: &str) -> bool {
    connection
        .query_row(
            "SELECT EXISTS (SELECT 1 FROM sqlite_schema WHERE name = ?1 AND type = 'table')",
            [table],
            |row| row.get(0),
        )
        .unwrap()
}

#[test]
fn fresh_database_has_seventeen_empty_domain_tables_and_verified_history() {
    let database = Database::in_memory().unwrap();
    assert_eq!(database.schema_version().unwrap(), SCHEMA_VERSION);
    for table in DOMAIN_TABLES {
        assert!(exists(&database.connection, table));
        assert_eq!(count(&database.connection, table), 0);
    }
    let total: i64 = database
        .connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_schema WHERE type = 'table' AND name NOT LIKE 'sqlite_%'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(total, 18);
    let (stored, timestamp): (String, String) = database
        .connection
        .query_row(
            "SELECT checksum, applied_at FROM schema_migrations",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(stored, checksum(&MIGRATIONS[0]));
    assert!(timestamp.ends_with('Z') && timestamp.contains('T'));
    assert_eq!(
        database
            .connection
            .pragma_query_value::<i64, _>(None, "foreign_keys", |row| row.get(0))
            .unwrap(),
        1
    );
    assert_eq!(
        database
            .connection
            .pragma_query_value::<i64, _>(None, "busy_timeout", |row| row.get(0))
            .unwrap(),
        5_000
    );
}

#[test]
fn migrations_are_idempotent_and_do_not_rewrite_history() {
    let mut database = Database::in_memory().unwrap();
    game(&database.connection, "g1");
    let original: String = database
        .connection
        .query_row("SELECT applied_at FROM schema_migrations", [], |r| r.get(0))
        .unwrap();
    for _ in 0..3 {
        assert_eq!(
            database.migrate().unwrap(),
            MigrationReport {
                changed_versions: vec![],
                current_version: 1
            }
        );
    }
    assert_eq!(count(&database.connection, "games"), 1);
    assert_eq!(
        database
            .connection
            .query_row::<String, _, _>("SELECT applied_at FROM schema_migrations", [], |r| r.get(0))
            .unwrap(),
        original
    );
}

#[test]
fn one_game_can_have_multiple_installations_without_duplicating_the_game() {
    let database = library();
    assert_eq!(
        database
            .connection
            .query_row::<i64, _, _>(
                "SELECT COUNT(*) FROM game_installations WHERE game_id = 'g1'",
                [],
                |r| r.get(0)
            )
            .unwrap(),
        2
    );
    assert_eq!(count(&database.connection, "games"), 2);
    assert!(database.connection.execute("INSERT INTO game_installations (id, game_id, absolute_path, folder_name) VALUES ('duplicate', 'g1', 'C:/fixture/i1', 'fixture')", []).is_err());
}

#[test]
fn save_profiles_and_sessions_enforce_game_install_ownership_on_insert_and_update() {
    let database = library();
    profile(&database.connection);
    assert!(database.connection.execute("INSERT INTO save_profiles (id, game_id, install_id, source_path, target_path) VALUES ('bad', 'g2', 'i1', 'save', 'backup')", []).is_err());
    assert!(database
        .connection
        .execute(
            "UPDATE save_profiles SET game_id = 'g2' WHERE id = 'p1'",
            []
        )
        .is_err());
    database.connection.execute("INSERT INTO play_sessions (id, game_id, install_id, started_at) VALUES ('s1', 'g1', 'i1', ?1)", [NOW]).unwrap();
    assert!(database.connection.execute("INSERT INTO play_sessions (id, game_id, install_id, started_at) VALUES ('bad', 'g1', 'i3', ?1)", [NOW]).is_err());
    assert!(database
        .connection
        .execute(
            "UPDATE play_sessions SET install_id = 'i3' WHERE id = 's1'",
            []
        )
        .is_err());
    assert!(database
        .connection
        .execute(
            "UPDATE game_installations SET game_id = 'g2' WHERE id = 'i1'",
            []
        )
        .is_err());
}

#[test]
fn orphaned_domain_records_are_rejected() {
    let database = library();
    collections(&database.connection);
    let statements = [
        "INSERT INTO game_installations (id, game_id, absolute_path, folder_name) VALUES ('bad', 'missing', 'no-game', 'folder')",
        "INSERT INTO aliases (id, game_id, alias) VALUES ('bad', 'missing', 'name')",
        "INSERT INTO metadata_records (id, game_id, provider, field_name, value_json) VALUES ('bad', 'missing', 'vndb', 'title', '\"name\"')",
        "INSERT INTO notes (id, game_id) VALUES ('bad', 'missing')",
        "INSERT INTO screenshots (id, game_id, absolute_path) VALUES ('bad', 'missing', 'image.png')",
        "INSERT INTO game_tags (id, game_id, tag_id) VALUES ('bad', 'g1', 'missing')",
        "INSERT INTO collection_members (id, collection_id, game_id) VALUES ('bad', 'c1', 'missing')",
        "INSERT INTO collections (id, name, kind, smart_filter_id) VALUES ('bad', 'Smart', 'smart', 'missing')",
        "INSERT INTO recommendation_preferences (id, game_id, preference) VALUES ('bad', 'missing', 'not_interested')",
    ];
    for sql in statements {
        assert!(
            database.connection.execute(sql, []).is_err(),
            "allowed orphan: {sql}"
        );
    }
    assert!(database.connection.execute("INSERT INTO play_sessions (id, game_id, install_id, started_at) VALUES ('bad', 'g1', 'missing', ?1)", [NOW]).is_err());
    assert!(database.connection.execute("INSERT INTO save_profiles (id, game_id, install_id, source_path, target_path) VALUES ('bad', 'g1', 'missing', 'save', 'backup')", []).is_err());
}

#[test]
fn snapshots_require_a_profile_and_valid_sha256_and_sizes() {
    let database = library();
    profile(&database.connection);
    for profile_id in [Some("missing"), None] {
        assert!(database.connection.execute("INSERT INTO save_snapshots (id, profile_id, archive_path, sha256, size_bytes) VALUES ('bad', ?1, 'backup.zip', ?2, 0)", params![profile_id, SHA]).is_err());
    }
    for sha in [
        "bad",
        "GGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGG",
    ] {
        assert!(database.connection.execute("INSERT INTO save_snapshots (id, profile_id, archive_path, sha256, size_bytes) VALUES ('bad', 'p1', 'backup.zip', ?1, 0)", [sha]).is_err());
    }
    assert!(database.connection.execute("INSERT INTO save_snapshots (id, profile_id, archive_path, sha256, size_bytes) VALUES ('bad', 'p1', 'backup.zip', ?1, -1)", [SHA]).is_err());
    database.connection.execute("INSERT INTO save_snapshots (id, profile_id, archive_path, sha256, size_bytes) VALUES ('snap1', 'p1', 'backup.zip', ?1, 32)", [SHA]).unwrap();
}

#[test]
fn screenshot_optional_install_still_requires_consistent_ownership() {
    let database = library();
    assert!(database.connection.execute("INSERT INTO screenshots (id, game_id, install_id, absolute_path) VALUES ('bad', 'g1', 'i3', 'bad.png')", []).is_err());
    database.connection.execute("INSERT INTO screenshots (id, game_id, absolute_path) VALUES ('manual', 'g1', 'manual.png')", []).unwrap();
}

#[test]
fn statuses_sources_boolean_rating_and_duration_constraints_are_checked() {
    let database = library();
    for status in [
        "not_started",
        "playing",
        "paused",
        "completed",
        "dropped",
        "pending_confirmation",
    ] {
        database
            .connection
            .execute("UPDATE games SET status = ?1 WHERE id = 'g1'", [status])
            .unwrap();
    }
    for source in ["local", "steam", "manual", "unknown"] {
        database
            .connection
            .execute(
                "UPDATE game_installations SET source = ?1 WHERE id = 'i1'",
                [source],
            )
            .unwrap();
    }
    for sql in [
        "UPDATE games SET status = 'invalid' WHERE id = 'g1'",
        "UPDATE games SET is_favorite = 2 WHERE id = 'g1'",
        "UPDATE games SET user_rating = 10.1 WHERE id = 'g1'",
        "UPDATE games SET user_rating = -1 WHERE id = 'g1'",
        "UPDATE game_installations SET source = 'piracy' WHERE id = 'i1'",
        "UPDATE game_installations SET run_as_admin = 2 WHERE id = 'i1'",
        "INSERT INTO games (id, title) VALUES ('', 'empty id')",
    ] {
        assert!(
            database.connection.execute(sql, []).is_err(),
            "allowed invalid value: {sql}"
        );
    }
    for seconds in [-1.0, 1.5] {
        assert!(database.connection.execute("INSERT INTO play_sessions (id, game_id, install_id, started_at, duration_seconds) VALUES ('bad', 'g1', 'i1', ?1, ?2)", params![NOW, seconds]).is_err());
    }
    database.connection.execute("INSERT INTO play_sessions (id, game_id, install_id, started_at, duration_seconds) VALUES ('valid', 'g1', 'i1', ?1, 3600)", [NOW]).unwrap();
}

#[test]
fn timestamps_must_be_parseable_utc_iso8601_and_sessions_cannot_end_before_start() {
    let database = library();
    for timestamp in [
        "2026-01-02 03:04:05",
        "2026-01-02T03:04:05+08:00",
        "2026-99-99T03:04:05Z",
        "garbage",
    ] {
        assert!(database
            .connection
            .execute(
                "UPDATE games SET created_at = ?1 WHERE id = 'g1'",
                [timestamp]
            )
            .is_err());
    }
    database
        .connection
        .execute("UPDATE games SET updated_at = ?1 WHERE id = 'g1'", [NOW])
        .unwrap();
    assert!(database.connection.execute("INSERT INTO play_sessions (id, game_id, install_id, started_at, ended_at) VALUES ('bad', 'g1', 'i1', ?1, '2026-01-01T00:00:00Z')", [NOW]).is_err());
}

#[test]
fn json_fields_have_valid_json_and_the_expected_container_shape() {
    let database = library();
    for sql in [
        "UPDATE game_installations SET environment_json = '[]' WHERE id = 'i1'",
        "UPDATE game_installations SET launch_arguments_json = '{}' WHERE id = 'i1'",
        "INSERT INTO settings (id, key, value_json) VALUES ('bad', 'bad', '{broken')",
        "INSERT INTO smart_filters (id, name, query_json) VALUES ('bad', 'Bad', '[]')",
        "INSERT INTO metadata_cache (id, provider, query, status) VALUES ('bad', 'vndb', 'test', 'success')",
    ] { assert!(database.connection.execute(sql, []).is_err(), "allowed malformed JSON: {sql}"); }
    let data = serde_json::json!({"locale":"zh-CN"}).to_string();
    database
        .connection
        .execute(
            "INSERT INTO settings (id, key, value_json) VALUES ('s1', 'appearance', ?1)",
            [data],
        )
        .unwrap();
}

#[test]
fn smart_collection_members_are_rejected_by_method_and_insert_update_triggers() {
    let mut database = library();
    collections(&database.connection);
    database.add_collection_member("m1", "c1", "g1", 0).unwrap();
    assert!(matches!(
        database.add_collection_member("m2", "c2", "g2", 0),
        Err(DatabaseError::SmartCollection)
    ));
    assert!(matches!(
        database.add_collection_member("m2", "missing", "g2", 0),
        Err(DatabaseError::CollectionNotFound)
    ));
    assert!(database.connection.execute("INSERT INTO collection_members (id, collection_id, game_id) VALUES ('bad', 'c2', 'g1')", []).is_err());
    assert!(database
        .connection
        .execute(
            "UPDATE collection_members SET collection_id = 'c2' WHERE id = 'm1'",
            []
        )
        .is_err());
    assert!(database
        .connection
        .execute(
            "UPDATE collections SET kind = 'smart', smart_filter_id = 'f1' WHERE id = 'c1'",
            []
        )
        .is_err());
    assert!(database
        .add_collection_member("duplicate", "c1", "g1", 1)
        .is_err());
    assert!(database
        .add_collection_member("negative", "c1", "g2", -1)
        .is_err());
    assert!(database
        .add_collection_member("orphan", "c1", "missing", 0)
        .is_err());
    assert_eq!(count(&database.connection, "collection_members"), 1);
}

#[test]
fn collection_kind_filter_and_filter_delete_constraints_are_enforced() {
    let database = library();
    collections(&database.connection);
    assert!(database
        .connection
        .execute(
            "INSERT INTO collections (id, name, kind) VALUES ('bad', 'Bad', 'smart')",
            []
        )
        .is_err());
    assert!(database
        .connection
        .execute(
            "UPDATE collections SET smart_filter_id = 'f1' WHERE id = 'c1'",
            []
        )
        .is_err());
    assert!(database
        .connection
        .execute("DELETE FROM smart_filters WHERE id = 'f1'", [])
        .is_err());
}

#[test]
fn deleting_collections_cascades_members_but_not_games_or_installations() {
    let mut database = library();
    collections(&database.connection);
    database.add_collection_member("m1", "c1", "g1", 0).unwrap();
    assert!(database.delete_collection("c1").unwrap());
    assert!(!database.delete_collection("c1").unwrap());
    assert_eq!(count(&database.connection, "collection_members"), 0);
    assert_eq!(count(&database.connection, "games"), 2);
    assert_eq!(count(&database.connection, "game_installations"), 3);
    assert!(database.delete_collection("c2").unwrap());
    assert_eq!(count(&database.connection, "smart_filters"), 1);
}

#[test]
fn failed_initial_sql_rolls_back_tables_and_migration_history() {
    let mut connection = Connection::open_in_memory().unwrap();
    configure(&connection).unwrap();
    let bad = [Migration {
        version: 1,
        name: "bad",
        up: "CREATE TABLE partial (id TEXT); INVALID SQL;",
        down: "DROP TABLE partial;",
    }];
    assert!(migrate_connection(&mut connection, &bad).is_err());
    assert!(!exists(&connection, "partial"));
    assert!(!exists(&connection, "schema_migrations"));
}

#[test]
fn multiple_pending_migrations_share_one_atomic_transaction() {
    let mut connection = Connection::open_in_memory().unwrap();
    configure(&connection).unwrap();
    let bad = Migration {
        version: 2,
        name: "bad",
        up: "CREATE TABLE partial (id TEXT); INSERT INTO does_not_exist VALUES (1);",
        down: "DROP TABLE partial;",
    };
    assert!(migrate_connection(&mut connection, &[MIGRATIONS[0], bad]).is_err());
    assert!(!exists(&connection, "games"));
    assert!(!exists(&connection, "partial"));
    assert!(!exists(&connection, "schema_migrations"));
    migrate_connection(&mut connection, MIGRATIONS).unwrap();
    game(&connection, "preserved");
    assert!(migrate_connection(&mut connection, &[MIGRATIONS[0], bad]).is_err());
    assert!(!exists(&connection, "partial"));
    assert_eq!(count(&connection, "games"), 1);
    assert_eq!(current_version(&connection, MIGRATIONS).unwrap(), 1);
}

#[test]
fn manifests_reject_missing_duplicate_unsorted_versions_and_duplicate_names() {
    let version_one = MIGRATIONS[0];
    let version_two = Migration {
        version: 2,
        name: "second",
        up: "CREATE TABLE second (id TEXT);",
        down: "DROP TABLE second;",
    };
    let invalid = [
        vec![version_two],
        vec![version_one, version_one],
        vec![
            version_one,
            Migration {
                version: 3,
                ..version_two
            },
        ],
        vec![version_two, version_one],
        vec![
            version_one,
            Migration {
                name: "initial",
                ..version_two
            },
        ],
    ];
    for manifest in invalid {
        let mut connection = Connection::open_in_memory().unwrap();
        assert!(matches!(
            migrate_connection(&mut connection, &manifest),
            Err(DatabaseError::InvalidMigrationSet(_))
        ));
        assert!(!exists(&connection, "schema_migrations"));
    }
}

#[test]
fn future_versions_and_checksum_tampering_are_rejected_without_writes() {
    let mut database = Database::in_memory().unwrap();
    database
        .connection
        .execute(
            "INSERT INTO schema_migrations (version, name, checksum) VALUES (2, 'future', ?1)",
            [SHA],
        )
        .unwrap();
    assert!(matches!(
        database.migrate(),
        Err(DatabaseError::UnknownVersion(2))
    ));
    assert_eq!(count(&database.connection, "schema_migrations"), 2);
    database
        .connection
        .execute("DELETE FROM schema_migrations WHERE version = 2", [])
        .unwrap();
    database
        .connection
        .execute(
            "UPDATE schema_migrations SET checksum = ?1 WHERE version = 1",
            [SHA],
        )
        .unwrap();
    assert!(matches!(
        database.migrate(),
        Err(DatabaseError::ChecksumMismatch { version: 1 })
    ));
    assert!(matches!(
        database.rollback_empty(0),
        Err(DatabaseError::ChecksumMismatch { version: 1 })
    ));
    assert_eq!(count(&database.connection, "schema_migrations"), 1);
    assert!(exists(&database.connection, "games"));
}

#[test]
fn modifying_either_sql_direction_or_migration_name_is_detected() {
    let mut database = Database::in_memory().unwrap();
    for changed in [
        Migration {
            up: "CREATE TABLE drift (id TEXT);",
            ..MIGRATIONS[0]
        },
        Migration {
            down: "DROP TABLE drift;",
            ..MIGRATIONS[0]
        },
    ] {
        assert!(matches!(
            migrate_connection(&mut database.connection, &[changed]),
            Err(DatabaseError::ChecksumMismatch { version: 1 })
        ));
    }
    assert!(matches!(
        migrate_connection(
            &mut database.connection,
            &[Migration {
                name: "renamed",
                ..MIGRATIONS[0]
            }]
        ),
        Err(DatabaseError::MigrationNameMismatch { version: 1 })
    ));
}

#[test]
fn migration_checksum_is_stable_across_lf_and_windows_crlf() {
    let lf = Migration {
        version: 1,
        name: "line-endings",
        up: "CREATE TABLE test (id TEXT);\n",
        down: "DROP TABLE test;\n",
    };
    let crlf = Migration {
        up: "CREATE TABLE test (id TEXT);\r\n",
        down: "DROP TABLE test;\r\n",
        ..lf
    };
    assert_eq!(checksum(&lf), checksum(&crlf));
    assert_ne!(
        checksum(&lf),
        checksum(&Migration {
            down: "DROP TABLE test; \n",
            ..lf
        })
    );
}

#[test]
fn omitted_or_duplicate_applied_versions_are_rejected() {
    let second = Migration {
        version: 2,
        name: "second",
        up: "CREATE TABLE second (id TEXT);",
        down: "DROP TABLE second;",
    };
    let known = [MIGRATIONS[0], second];
    let mut database = Database::in_memory().unwrap();
    migrate_connection(&mut database.connection, &known).unwrap();
    database
        .connection
        .execute("DELETE FROM schema_migrations WHERE version = 1", [])
        .unwrap();
    assert!(matches!(
        migrate_connection(&mut database.connection, &known),
        Err(DatabaseError::HistoryGap {
            expected: 1,
            found: 2
        })
    ));
    let mut connection = Connection::open_in_memory().unwrap();
    configure(&connection).unwrap();
    // Deliberately corrupt legacy metadata lacking its intended primary key.
    connection
        .execute_batch("CREATE TABLE schema_migrations (version INTEGER, name TEXT, checksum TEXT)")
        .unwrap();
    for _ in 0..2 {
        connection
            .execute(
                "INSERT INTO schema_migrations VALUES (1, 'initial', ?1)",
                [checksum(&MIGRATIONS[0])],
            )
            .unwrap();
    }
    assert!(matches!(
        migrate_connection(&mut connection, MIGRATIONS),
        Err(DatabaseError::HistoryGap {
            expected: 2,
            found: 1
        })
    ));
    assert!(!exists(&connection, "games"));
}

#[test]
fn loss_of_all_history_is_not_silently_accepted_as_a_new_database() {
    let mut database = Database::in_memory().unwrap();
    database
        .connection
        .execute("DELETE FROM schema_migrations", [])
        .unwrap();
    assert!(database.migrate().is_err());
    assert_eq!(count(&database.connection, "schema_migrations"), 0);
    for table in DOMAIN_TABLES {
        assert!(exists(&database.connection, table));
    }
}

#[test]
fn existing_orphans_are_detected_instead_of_silently_deleted() {
    let mut database = Database::in_memory().unwrap();
    database
        .connection
        .pragma_update(None, "foreign_keys", false)
        .unwrap();
    database
        .connection
        .execute(
            "INSERT INTO notes (id, game_id) VALUES ('orphan', 'missing')",
            [],
        )
        .unwrap();
    database
        .connection
        .pragma_update(None, "foreign_keys", true)
        .unwrap();
    assert!(matches!(
        database.migrate(),
        Err(DatabaseError::ForeignKeyViolation)
    ));
    assert_eq!(count(&database.connection, "notes"), 1);
}

#[test]
fn empty_rollback_removes_domain_tables_then_reapplication_is_clean() {
    let mut database = Database::in_memory().unwrap();
    assert!(matches!(
        database.rollback_empty(2),
        Err(DatabaseError::InvalidRollbackTarget {
            target: 2,
            current: 1
        })
    ));
    assert_eq!(
        database.rollback_empty(0).unwrap(),
        MigrationReport {
            changed_versions: vec![1],
            current_version: 0
        }
    );
    assert_eq!(database.schema_version().unwrap(), 0);
    for table in DOMAIN_TABLES {
        assert!(!exists(&database.connection, table));
    }
    assert!(database
        .rollback_empty(0)
        .unwrap()
        .changed_versions
        .is_empty());
    assert_eq!(database.migrate().unwrap().changed_versions, vec![1]);
    assert_eq!(database.schema_version().unwrap(), 1);
}

#[test]
fn rollback_refuses_even_non_game_domain_records_and_noop_targets() {
    for (table, sql) in [
        ("games", "INSERT INTO games (id, title) VALUES ('g1', 'Fixture')"),
        ("tags", "INSERT INTO tags (id, name) VALUES ('t1', 'Fixture')"),
        ("settings", "INSERT INTO settings (id, key, value_json) VALUES ('s1', 'locale', '\"zh-CN\"')"),
        ("smart_filters", "INSERT INTO smart_filters (id, name, query_json) VALUES ('f1', 'Fixture', '{}')"),
        ("metadata_cache", "INSERT INTO metadata_cache (id, provider, query, status, response_json) VALUES ('m1', 'vndb', 'Fixture', 'success', '{}')"),
        ("collections", "INSERT INTO collections (id, name, kind) VALUES ('c1', 'Fixture', 'normal')"),
    ] {
        let mut database = Database::in_memory().unwrap();
        database.connection.execute(sql, []).unwrap();
        for target in [0, 1] {
            assert!(matches!(database.rollback_empty(target), Err(DatabaseError::NonEmptyDatabase { table: found }) if found == table));
            assert_eq!(count(&database.connection, table), 1);
            assert_eq!(database.schema_version().unwrap(), 1);
        }
    }
}

#[test]
fn rollback_sql_failure_is_transactional_too() {
    let mut connection = Connection::open_in_memory().unwrap();
    configure(&connection).unwrap();
    let known = [Migration {
        version: 1,
        name: "test",
        up: "CREATE TABLE games (id TEXT);",
        down: "DROP TABLE games; CREATE TABLE partial (id TEXT); INVALID SQL;",
    }];
    migrate_connection(&mut connection, &known).unwrap();
    assert!(rollback_empty(&mut connection, &known, 0).is_err());
    assert!(exists(&connection, "games"));
    assert!(!exists(&connection, "partial"));
    assert_eq!(current_version(&connection, &known).unwrap(), 1);
}

struct FixtureDirectory(PathBuf);
impl FixtureDirectory {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let parent = std::env::temp_dir().canonicalize().unwrap();
        let leaf = format!(
            "ai02-db-test-{}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        );
        let path = parent.join(leaf);
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for FixtureDirectory {
    fn drop(&mut self) {
        // Verify the resolved absolute target before removing any test files.
        let expected_parent = std::env::temp_dir().canonicalize().unwrap();
        if self.0.parent() == Some(expected_parent.as_path())
            && self
                .0
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("ai02-db-test-")
            && self.0.canonicalize().ok().as_ref() == Some(&self.0)
        {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }
}

#[test]
fn file_open_is_persistent_and_rejects_future_history_on_reopen() {
    let temporary = FixtureDirectory::new();
    let path = temporary.0.join("library.sqlite");
    let database = Database::open(&path).unwrap();
    game(&database.connection, "persistent");
    drop(database);
    let database = Database::open(&path).unwrap();
    assert_eq!(count(&database.connection, "games"), 1);
    database
        .connection
        .execute(
            "INSERT INTO schema_migrations (version, name, checksum) VALUES (2, 'future', ?1)",
            [SHA],
        )
        .unwrap();
    drop(database);
    assert!(matches!(
        Database::open(&path),
        Err(DatabaseError::UnknownVersion(2))
    ));
}

#[test]
fn valid_history_does_not_hide_a_dropped_domain_table() {
    let temporary = FixtureDirectory::new();
    let path = temporary.0.join("library.sqlite");
    let mut database = Database::open(&path).unwrap();
    // settings has no incoming FK, so foreign_key_check alone cannot detect loss.
    database
        .connection
        .execute_batch("DROP TABLE settings")
        .unwrap();
    assert_eq!(
        current_version(&database.connection, MIGRATIONS).unwrap(),
        1
    );
    assert!(
        matches!(database.schema_version(), Err(DatabaseError::MissingDomainTable { table }) if table == "settings")
    );
    assert!(
        matches!(database.migrate(), Err(DatabaseError::MissingDomainTable { table }) if table == "settings")
    );
    assert!(!exists(&database.connection, "settings"));
    drop(database);
    assert!(
        matches!(Database::open(&path), Err(DatabaseError::MissingDomainTable { table }) if table == "settings")
    );
    let connection = Connection::open(&path).unwrap();
    assert_eq!(current_version(&connection, MIGRATIONS).unwrap(), 1);
    assert!(!exists(&connection, "settings"));
}

#[test]
fn database_deletions_never_remove_external_game_save_snapshot_or_screenshot_files() {
    let temporary = FixtureDirectory::new();
    let game_path = temporary.0.join("game.exe");
    let save_path = temporary.0.join("save.dat");
    let archive_path = temporary.0.join("snapshot.zip");
    let screenshot_path = temporary.0.join("screenshot.png");
    for path in [&game_path, &save_path, &archive_path, &screenshot_path] {
        fs::write(path, b"fixture-only-do-not-execute").unwrap();
    }
    let database = Database::open(temporary.0.join("library.sqlite")).unwrap();
    game(&database.connection, "g1");
    database.connection.execute("INSERT INTO game_installations (id, game_id, absolute_path, executable_path, folder_name) VALUES ('i1', 'g1', ?1, ?2, 'fixture')", params![temporary.0.to_string_lossy(), game_path.to_string_lossy()]).unwrap();
    database.connection.execute("INSERT INTO save_profiles (id, game_id, install_id, source_path, target_path) VALUES ('p1', 'g1', 'i1', ?1, ?2)", params![save_path.to_string_lossy(), temporary.0.to_string_lossy()]).unwrap();
    database.connection.execute("INSERT INTO save_snapshots (id, profile_id, archive_path, sha256, size_bytes) VALUES ('snap1', 'p1', ?1, ?2, 32)", params![archive_path.to_string_lossy(), SHA]).unwrap();
    database.connection.execute("INSERT INTO screenshots (id, game_id, install_id, absolute_path) VALUES ('shot1', 'g1', 'i1', ?1)", [screenshot_path.to_string_lossy()]).unwrap();
    database
        .connection
        .execute("DELETE FROM games WHERE id = 'g1'", [])
        .unwrap();
    for table in [
        "game_installations",
        "save_profiles",
        "save_snapshots",
        "screenshots",
    ] {
        assert_eq!(count(&database.connection, table), 0);
    }
    for path in [&game_path, &save_path, &archive_path, &screenshot_path] {
        assert_eq!(fs::read(path).unwrap(), b"fixture-only-do-not-execute");
    }
    drop(database);
}

#[test]
fn application_update_refuses_live_games_and_pending_recovery() {
    let database = library();
    assert!(database.prepare_application_update().is_ok());
    database.connection.execute("INSERT INTO play_sessions (id, game_id, install_id, started_at) VALUES ('update-session', 'g1', 'i1', ?1)", [NOW]).unwrap();
    assert!(database.prepare_application_update().is_err());
    database
        .connection
        .execute(
            "UPDATE play_sessions SET ended_at=started_at WHERE id='update-session'",
            [],
        )
        .unwrap();
    assert!(database.prepare_application_update().is_ok());
    for key in [
        "database.import.pending",
        "saves.restore.pending",
        "backup.restore.pending",
        "backup.restore.journal",
    ] {
        database.put_setting(key, &true).unwrap();
        assert!(database.prepare_application_update().is_err());
        database.delete_setting(key).unwrap();
    }
    assert!(database.prepare_application_update().is_ok());
}
