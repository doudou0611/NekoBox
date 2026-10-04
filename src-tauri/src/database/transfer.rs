use super::*;
use crate::backend::{invalid, Result as ServiceResult};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseCounts {
    pub games: u64,
    pub installations: u64,
    pub collections: u64,
    pub sessions: u64,
}
fn schema(connection: &Connection) -> Result<Vec<(String, String, String, String)>> {
    let mut statement = connection.prepare(
        "SELECT name,type,tbl_name,coalesce(sql,'') FROM sqlite_schema WHERE name NOT LIKE 'sqlite_%' ORDER BY name",
    )?;
    let rows = statement
        .query_map([], |row| {
            let sql: String = row.get(3)?;
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                sql.replace("\r\n", "\n"),
            ))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows)
}
impl Database {
    pub(crate) fn ensure_recovery_idle_except(&self, allowed: Option<&str>) -> ServiceResult<()> {
        for key in [
            "database.import.pending",
            "saves.restore.pending",
            "backup.restore.pending",
            "backup.restore.journal",
        ] {
            if Some(key) != allowed
                && self.connection.query_row(
                    "SELECT EXISTS(SELECT 1 FROM settings WHERE key=?)",
                    [key],
                    |r| r.get::<_, bool>(0),
                )?
            {
                return Err(invalid(
                    "已有恢复或导入事务，请先完成恢复或取消待重启任务。",
                ));
            }
        }
        Ok(())
    }

    /// Never migrates or writes the selected source. Accept only the exact current schema.
    pub fn verified_snapshot(path: &Path) -> ServiceResult<Self> {
        let connection =
            Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        connection.pragma_update(None, "trusted_schema", false)?;
        let db = Self { connection };
        if db.schema_version()? != SCHEMA_VERSION {
            return Err(invalid("备份数据库版本不兼容，请使用对应版本的软件。"));
        }
        let integrity: String = db
            .connection
            .query_row("PRAGMA quick_check", [], |r| r.get(0))?;
        if integrity != "ok" {
            return Err(invalid("数据库完整性检查失败。"));
        }
        if db
            .connection
            .prepare("PRAGMA foreign_key_check")?
            .query([])?
            .next()?
            .is_some()
        {
            return Err(invalid("数据库存在无效归属关系，拒绝导入。"));
        }
        let reference = Self::in_memory()?;
        if schema(&db.connection)? != schema(&reference.connection)? {
            return Err(invalid("数据库结构与当前应用不一致，拒绝导入。"));
        }
        {
            let mut settings = db
                .connection
                .prepare("SELECT key,value_json FROM settings")?;
            let rows =
                settings.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
            for row in rows {
                let (key, raw) = row?;
                crate::backend::application_backup::validate_persistent_setting(&key, &raw)?;
            }
        }
        {
            let mut filters = db
                .connection
                .prepare("SELECT query_json FROM smart_filters")?;
            for raw in filters.query_map([], |r| r.get::<_, String>(0))? {
                let query: crate::domain::models::GameQuery =
                    serde_json::from_str(&raw?).map_err(|_| invalid("智能集合查询无效。"))?;
                super::library::validate_query(&query)?;
                if query.collection_id.is_some() {
                    return Err(invalid("智能集合不能引用集合。"));
                }
            }
        }
        for collection in db.list_collections()? {
            db.get_collection(&collection.id)?;
        }
        Ok(db)
    }
    pub fn transfer_counts(&self) -> Result<DatabaseCounts> {
        let count = |table: &str| {
            self.connection
                .query_row(&format!("SELECT count(*) FROM {table}"), [], |r| {
                    r.get::<_, i64>(0)
                })
                .map(|count| count as u64)
        };
        Ok(DatabaseCounts {
            games: count("games")?,
            installations: count("game_installations")?,
            collections: count("collections")?,
            sessions: count("play_sessions")?,
        })
    }
    pub fn sanitize_import(&mut self) -> ServiceResult<()> {
        // A foreign recovery log must never trigger writes to any user's save directory.
        self.connection.execute(
            "DELETE FROM settings WHERE key IN ('saves.restore.pending','database.import.pending','database.import.error','bangumi.profile','backup.restore.pending','backup.restore.journal')",
            [],
        )?;
        self.connection.execute("DELETE FROM settings WHERE key LIKE 'backup.uploaded.%' OR key IN ('backup.history','backup.last_auto','backup.last_attempt')",[])?;
        self.recover_interrupted()?;
        Ok(())
    }
    pub fn restore_verified_snapshot(&mut self, source: &Self) -> Result<()> {
        // SQLite keeps the destination write transaction open until backup completes.
        // A failed/abandoned backup rolls it back instead of partially replacing pages.
        let backup = rusqlite::backup::Backup::new(&source.connection, &mut self.connection)?;
        let deadline = std::time::Instant::now() + Duration::from_secs(30);
        loop {
            if backup.step(128)? == rusqlite::backup::StepResult::Done {
                return Ok(());
            }
            if std::time::Instant::now() > deadline {
                return Err(DatabaseError::InvalidMigrationSet(
                    "restore deadline exceeded".into(),
                ));
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unfinished_sqlite_backup_rolls_back_written_pages() {
        let source = Database::in_memory().unwrap();
        source
            .connection
            .execute("INSERT INTO tags(id,name) VALUES('source','source')", [])
            .unwrap();
        source.put_setting("large", &"a".repeat(2_000_000)).unwrap();
        let mut destination = Database::in_memory().unwrap();
        destination
            .connection
            .execute(
                "INSERT INTO tags(id,name) VALUES('original','original')",
                [],
            )
            .unwrap();
        {
            let backup =
                rusqlite::backup::Backup::new(&source.connection, &mut destination.connection)
                    .unwrap();
            assert_eq!(backup.step(1).unwrap(), rusqlite::backup::StepResult::More);
            // Drop abandons an incomplete transaction after actual destination page writes.
        }
        let tag: String = destination
            .connection
            .query_row("SELECT id FROM tags", [], |r| r.get(0))
            .unwrap();
        assert_eq!(tag, "original");
        assert_eq!(destination.schema_version().unwrap(), SCHEMA_VERSION);
    }
}
