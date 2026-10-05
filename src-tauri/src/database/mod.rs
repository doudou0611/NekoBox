//! SQLite migrations and production repositories. Opening/migrating never executes
//! or deletes game/save files. Runtime directory policy lives in backend.

mod account_sync;
mod activity;
pub(crate) mod application_backup;
mod detail_metadata;
mod hikarifield;
mod home;
mod library;
mod metadata;
mod migrations;
mod personal;
mod playtime;
pub(crate) mod prepared_import;
mod saves;
mod sources;
pub mod transfer;

use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use std::{error::Error, fmt, path::Path, time::Duration};

pub use migrations::{MigrationReport, DOMAIN_TABLES, SCHEMA_VERSION};

pub type Result<T> = std::result::Result<T, DatabaseError>;

#[derive(Debug)]
pub enum DatabaseError {
    Sqlite(rusqlite::Error),
    InvalidMigrationSet(String),
    UnknownVersion(i64),
    HistoryGap { expected: i64, found: i64 },
    ChecksumMismatch { version: i64 },
    MigrationNameMismatch { version: i64 },
    ForeignKeyViolation,
    MissingDomainTable { table: String },
    NonEmptyDatabase { table: String },
    InvalidRollbackTarget { target: u32, current: u32 },
    CollectionNotFound,
    SmartCollection,
}

impl fmt::Display for DatabaseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sqlite(error) => write!(f, "SQLite error: {error}"),
            Self::InvalidMigrationSet(reason) => write!(f, "Invalid migration set: {reason}"),
            Self::UnknownVersion(version) => write!(f, "Unsupported database migration version {version}"),
            Self::HistoryGap { expected, found } => write!(f, "Migration history gap/duplicate: expected {expected}, found {found}"),
            Self::ChecksumMismatch { version } => write!(f, "Migration {version} checksum drift; restore a verified backup or use the matching application version"),
            Self::MigrationNameMismatch { version } => write!(f, "Migration {version} name drift"),
            Self::ForeignKeyViolation => write!(f, "Database contains orphaned foreign-key records"),
            Self::MissingDomainTable { table } => write!(f, "Current database schema is incomplete: missing table {table}; restore a verified backup"),
            Self::NonEmptyDatabase { table } => write!(f, "Rollback refused: domain table {table} contains records; production recovery must use a backup"),
            Self::InvalidRollbackTarget { target, current } => write!(f, "Rollback target {target} exceeds current version {current}"),
            Self::CollectionNotFound => write!(f, "Collection does not exist"),
            Self::SmartCollection => write!(f, "Smart collection members are computed and cannot be written manually"),
        }
    }
}

impl Error for DatabaseError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Sqlite(error) => Some(error),
            _ => None,
        }
    }
}

impl From<rusqlite::Error> for DatabaseError {
    fn from(value: rusqlite::Error) -> Self {
        Self::Sqlite(value)
    }
}

#[derive(Debug)]
pub struct Database {
    connection: Connection,
}

impl Database {
    /// The caller owns path validation and app-data selection. Parent directories
    /// are NOT created. This only opens the database file, never stored game paths.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        Self::from_connection(Connection::open(path)?)
    }

    /// An empty migrated database. No fixture/mock data is automatically inserted.
    pub fn in_memory() -> Result<Self> {
        Self::from_connection(Connection::open_in_memory()?)
    }

    fn from_connection(mut connection: Connection) -> Result<Self> {
        configure(&connection)?;
        migrations::migrate_connection(&mut connection, migrations::MIGRATIONS)?;
        validate_domain_tables(&connection, SCHEMA_VERSION)?;
        Ok(Self { connection })
    }

    pub fn migrate(&mut self) -> Result<MigrationReport> {
        let report = migrations::migrate_connection(&mut self.connection, migrations::MIGRATIONS)?;
        validate_domain_tables(&self.connection, report.current_version)?;
        Ok(report)
    }

    /// Validates the entire applied history and required current-schema tables,
    /// not simply MAX(version). A deliberately rolled-back empty schema is v0.
    pub fn schema_version(&self) -> Result<u32> {
        let version = migrations::current_version(&self.connection, migrations::MIGRATIONS)?;
        validate_domain_tables(&self.connection, version)?;
        Ok(version)
    }

    /// Developer/test-only recovery helper. Refuses ANY domain records, even when
    /// target equals current version. No frontend command may expose this.
    /// Production recovery should back up then restore a verified database.
    pub fn rollback_empty(&mut self, target: u32) -> Result<MigrationReport> {
        migrations::rollback_empty(&mut self.connection, migrations::MIGRATIONS, target)
    }

    /// Minimal safety example, not a complete collection repository.
    /// BEGIN IMMEDIATE closes the race between checking kind and inserting.
    /// The SQL triggers independently enforce the same condition.
    pub fn add_collection_member(
        &mut self,
        id: &str,
        collection_id: &str,
        game_id: &str,
        position: i64,
    ) -> Result<()> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let kind: Option<String> = transaction
            .query_row(
                "SELECT kind FROM collections WHERE id = ?1",
                [collection_id],
                |row| row.get(0),
            )
            .optional()?;
        match kind.as_deref() {
            None => return Err(DatabaseError::CollectionNotFound),
            Some("normal") => {}
            Some(_) => return Err(DatabaseError::SmartCollection),
        }
        transaction.execute(
            "INSERT INTO collection_members (id, collection_id, game_id, position) VALUES (?1, ?2, ?3, ?4)",
            params![id, collection_id, game_id, position],
        )?;
        transaction.commit()?;
        Ok(())
    }

    /// Deletes organization records only; games/installations/files are untouched.
    /// Future user-facing callers must still obtain their required confirmation.
    pub fn delete_collection(&mut self, id: &str) -> Result<bool> {
        Ok(self
            .connection
            .execute("DELETE FROM collections WHERE id = ?1", [id])?
            != 0)
    }
}

fn validate_domain_tables(connection: &Connection, version: u32) -> Result<()> {
    if version != SCHEMA_VERSION {
        return Ok(());
    }
    for table in DOMAIN_TABLES {
        let exists: bool = connection.query_row(
            "SELECT EXISTS (SELECT 1 FROM sqlite_schema WHERE type = 'table' AND name = ?1)",
            [table],
            |row| row.get(0),
        )?;
        if !exists {
            return Err(DatabaseError::MissingDomainTable {
                table: table.to_owned(),
            });
        }
    }
    Ok(())
}

fn configure(connection: &Connection) -> Result<()> {
    connection.busy_timeout(Duration::from_secs(5))?;
    connection.pragma_update(None, "foreign_keys", true)?;
    let enabled: bool = connection.pragma_query_value(None, "foreign_keys", |row| row.get(0))?;
    if !enabled {
        return Err(DatabaseError::InvalidMigrationSet(
            "foreign_keys could not be enabled".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
