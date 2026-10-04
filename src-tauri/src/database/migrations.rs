use super::{DatabaseError, Result};
use rusqlite::{params, Connection, TransactionBehavior};
use sha2::{Digest, Sha256};

pub const SCHEMA_VERSION: u32 = 1;
pub const DOMAIN_TABLES: [&str; 17] = [
    "games",
    "game_installations",
    "metadata_records",
    "metadata_cache",
    "aliases",
    "play_sessions",
    "save_profiles",
    "save_snapshots",
    "screenshots",
    "tags",
    "game_tags",
    "notes",
    "smart_filters",
    "collections",
    "collection_members",
    "recommendation_preferences",
    "settings",
];

#[derive(Clone, Copy)]
pub(super) struct Migration {
    pub version: u32,
    pub name: &'static str,
    pub up: &'static str,
    pub down: &'static str,
}

pub(super) const MIGRATIONS: &[Migration] = &[Migration {
    version: SCHEMA_VERSION,
    name: "initial",
    up: include_str!("../../migrations/0001_initial.up.sql"),
    down: include_str!("../../migrations/0001_initial.down.sql"),
}];

#[derive(Debug, PartialEq, Eq)]
pub struct MigrationReport {
    /// Applied or removed versions, in execution order; empty on a no-op.
    pub changed_versions: Vec<u32>,
    pub current_version: u32,
}

const HISTORY_SQL: &str = "CREATE TABLE IF NOT EXISTS schema_migrations (
    version INTEGER PRIMARY KEY NOT NULL CHECK (version > 0),
    name TEXT NOT NULL UNIQUE,
    checksum TEXT NOT NULL CHECK (length(checksum) = 64 AND checksum NOT GLOB '*[^0-9a-f]*'),
    applied_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
        CHECK (applied_at GLOB '????-??-??T??:??:??*Z' AND julianday(applied_at) IS NOT NULL)
) STRICT";

pub(super) fn checksum(migration: &Migration) -> String {
    let mut hash = Sha256::new();
    hash.update(b"galgame-manager:migration:v1\0");
    hash.update(migration.version.to_be_bytes());
    for part in [migration.name, migration.up, migration.down] {
        // Git on Windows may check out CRLF. Canonical LF makes identical SQL
        // stable across hosts while still detecting every other content change.
        let normalized = part.replace("\r\n", "\n");
        hash.update((normalized.len() as u64).to_be_bytes());
        hash.update(normalized.as_bytes());
    }
    format!("{:x}", hash.finalize())
}

fn validate_manifest(migrations: &[Migration]) -> Result<()> {
    for (index, migration) in migrations.iter().enumerate() {
        let expected = index as u32 + 1;
        if migration.version != expected {
            return Err(DatabaseError::InvalidMigrationSet(format!(
                "expected consecutive version {expected}, found {}",
                migration.version,
            )));
        }
        if migration.name.trim().is_empty()
            || migration.up.trim().is_empty()
            || migration.down.trim().is_empty()
        {
            return Err(DatabaseError::InvalidMigrationSet(
                "empty name/up/down SQL".into(),
            ));
        }
        if migrations[..index]
            .iter()
            .any(|previous| previous.name == migration.name)
        {
            return Err(DatabaseError::InvalidMigrationSet(
                "duplicate migration name".into(),
            ));
        }
    }
    Ok(())
}

pub(super) fn current_version(connection: &Connection, migrations: &[Migration]) -> Result<u32> {
    validate_manifest(migrations)?;
    let mut statement = connection
        .prepare("SELECT version, name, checksum FROM schema_migrations ORDER BY version")?;
    let history = statement.query_map([], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
        ))
    })?;
    let mut count = 0;
    for entry in history {
        let (version, name, stored_checksum) = entry?;
        if version > migrations.len() as i64 {
            return Err(DatabaseError::UnknownVersion(version));
        }
        let expected = count as i64 + 1;
        if version != expected {
            return Err(DatabaseError::HistoryGap {
                expected,
                found: version,
            });
        }
        let known = &migrations[count];
        if known.name != name {
            return Err(DatabaseError::MigrationNameMismatch { version });
        }
        if checksum(known) != stored_checksum {
            return Err(DatabaseError::ChecksumMismatch { version });
        }
        count += 1;
    }
    Ok(count as u32)
}

fn check_foreign_keys(connection: &Connection) -> Result<()> {
    let mut statement = connection.prepare("PRAGMA foreign_key_check")?;
    if statement.query([])?.next()?.is_some() {
        return Err(DatabaseError::ForeignKeyViolation);
    }
    Ok(())
}

pub(super) fn migrate_connection(
    connection: &mut Connection,
    migrations: &[Migration],
) -> Result<MigrationReport> {
    validate_manifest(migrations)?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    // Bootstrapping and ALL pending versions share one transaction. A bad SQL
    // statement also rolls back the history table, not just the last version.
    transaction.execute_batch(HISTORY_SQL)?;
    let current = current_version(&transaction, migrations)?;
    let mut changed_versions = Vec::new();
    for migration in migrations.iter().skip(current as usize) {
        transaction.execute_batch(migration.up)?;
        transaction.execute(
            "INSERT INTO schema_migrations (version, name, checksum) VALUES (?1, ?2, ?3)",
            params![migration.version, migration.name, checksum(migration)],
        )?;
        changed_versions.push(migration.version);
    }
    check_foreign_keys(&transaction)?;
    transaction.commit()?;
    Ok(MigrationReport {
        changed_versions,
        current_version: migrations.len() as u32,
    })
}

pub(super) fn rollback_empty(
    connection: &mut Connection,
    migrations: &[Migration],
    target: u32,
) -> Result<MigrationReport> {
    validate_manifest(migrations)?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let current = current_version(&transaction, migrations)?;
    if target > current {
        return Err(DatabaseError::InvalidRollbackTarget { target, current });
    }
    // Static trusted identifiers only. Includes tables such as settings/cache/tags
    // that have no game FK: an "empty library" alone does not make rollback safe.
    for table in DOMAIN_TABLES {
        let exists: bool = transaction.query_row(
            "SELECT EXISTS (SELECT 1 FROM sqlite_schema WHERE type = 'table' AND name = ?1)",
            [table],
            |row| row.get(0),
        )?;
        if exists {
            let occupied: bool = transaction.query_row(
                &format!("SELECT EXISTS (SELECT 1 FROM {table} LIMIT 1)"),
                [],
                |row| row.get(0),
            )?;
            if occupied {
                return Err(DatabaseError::NonEmptyDatabase {
                    table: table.to_owned(),
                });
            }
        }
    }
    check_foreign_keys(&transaction)?;
    let mut changed_versions = Vec::new();
    for migration in migrations
        .iter()
        .take(current as usize)
        .skip(target as usize)
        .rev()
    {
        transaction.execute_batch(migration.down)?;
        transaction.execute(
            "DELETE FROM schema_migrations WHERE version = ?1",
            [migration.version],
        )?;
        changed_versions.push(migration.version);
    }
    transaction.commit()?;
    Ok(MigrationReport {
        changed_versions,
        current_version: target,
    })
}
