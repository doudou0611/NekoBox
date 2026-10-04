//! Selective exports contain JSON rows, never executable SQL or recovery journals.
use super::Database;
use crate::backend::{self, Result};
use rusqlite::{
    params_from_iter,
    types::{Value as SqlValue, ValueRef},
    Connection,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

pub type Row = BTreeMap<String, Value>;
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Data {
    pub tables: BTreeMap<String, Vec<Row>>,
    #[serde(default)]
    pub collection_covers: BTreeMap<String, Option<String>>,
}
pub const CATEGORIES: [&str; 9] = [
    "metadata",
    "covers",
    "playtime",
    "groups",
    "personal",
    "settings",
    "save_archives",
    "current_saves",
    "screenshots",
];
pub fn selected(categories: &[String], category: &str) -> bool {
    categories.iter().any(|c| c == category)
}
fn table_categories(table: &str) -> &[&str] {
    match table {
        "games" | "game_installations" => &[],
        "metadata_records" | "aliases" => &["metadata", "covers"],
        "play_sessions" => &["playtime"],
        "smart_filters" | "collections" | "collection_members" => &["groups"],
        "notes" | "tags" | "game_tags" | "recommendation_preferences" => &["personal"],
        "save_profiles" => &["save_archives", "current_saves", "settings"],
        "save_snapshots" => &["save_archives"],
        "screenshots" => &["screenshots"],
        "settings" => &["metadata", "covers", "playtime", "settings"],
        _ => &["invalid"],
    }
}
const TABLES: [&str; 16] = [
    "games",
    "game_installations",
    "metadata_records",
    "aliases",
    "play_sessions",
    "smart_filters",
    "collections",
    "collection_members",
    "tags",
    "game_tags",
    "notes",
    "recommendation_preferences",
    "save_profiles",
    "save_snapshots",
    "screenshots",
    "settings",
];
fn allowed(table: &str, categories: &[String]) -> bool {
    table_categories(table).is_empty()
        || table_categories(table)
            .iter()
            .any(|c| selected(categories, c))
}
fn setting_category(key: &str) -> Option<&'static str> {
    if key.starts_with("metadata.priority.")
        || key.starts_with("metadata.generated_title.")
        || key.starts_with("metadata.locked.")
    {
        return Some("metadata");
    }
    if key.starts_with("playtime.") {
        return Some("playtime");
    }
    if key.starts_with("bangumi.cover.") {
        return Some("covers");
    }
    if [
        "app.preferences",
        "metadata.sources",
        "metadata.translation",
        "metadata.hikarinagi",
        "vndb.settings",
        "backup.config",
    ]
    .contains(&key)
        || key.starts_with("launch.")
    {
        return Some("settings");
    }
    None
}
fn keep_row(table: &str, row: &mut Row, categories: &[String]) -> bool {
    if table == "games" {
        row.retain(|k, _| {
            ["id", "title", "created_at", "updated_at"].contains(&k.as_str())
                || selected(categories, "metadata")
                    && [
                        "title_zh",
                        "title_ja",
                        "title_en",
                        "description",
                        "developer",
                        "release_date",
                    ]
                    .contains(&k.as_str())
                || selected(categories, "personal")
                    && ["status", "is_favorite", "is_hidden", "user_rating"].contains(&k.as_str())
                || selected(categories, "covers") && k == "cover_path"
        });
    }
    if table == "game_installations" && !selected(categories, "settings") {
        row.retain(|k, _| {
            [
                "id",
                "game_id",
                "absolute_path",
                "folder_name",
                "source",
                "created_at",
                "updated_at",
            ]
            .contains(&k.as_str())
        });
    }
    if table == "metadata_records" {
        let cover = row.get("field_name").and_then(Value::as_str) == Some("cover_path");
        return if cover {
            selected(categories, "covers")
        } else {
            selected(categories, "metadata")
        };
    }
    if table == "aliases" {
        return selected(categories, "metadata");
    }
    if table == "settings" {
        return row
            .get("key")
            .and_then(Value::as_str)
            .and_then(setting_category)
            .is_some_and(|c| selected(categories, c));
    }
    if table == "collections" && !selected(categories, "covers") {
        row.remove("cover_path");
    }
    true
}
fn rows(connection: &Connection, table: &str) -> Result<Vec<Row>> {
    let mut stmt = connection.prepare(&format!("SELECT * FROM {table}"))?;
    let names = stmt
        .column_names()
        .iter()
        .map(|n| n.to_string())
        .collect::<Vec<_>>();
    let mut query = stmt.query([])?;
    let mut values = Vec::new();
    while let Some(row) = query.next()? {
        let mut value = Row::new();
        for (index, name) in names.iter().enumerate() {
            let v = match row.get_ref(index)? {
                ValueRef::Null => Value::Null,
                ValueRef::Integer(v) => Value::from(v),
                ValueRef::Real(v) => Value::from(v),
                ValueRef::Text(v) => Value::String(
                    String::from_utf8(v.to_vec())
                        .map_err(|_| backend::invalid("记录编码无效。"))?,
                ),
                ValueRef::Blob(_) => return Err(backend::invalid("不支持的记录类型。")),
            };
            value.insert(name.clone(), v);
        }
        values.push(value);
    }
    Ok(values)
}
fn sql_value(value: &Value) -> Result<SqlValue> {
    match value {
        Value::Null => Ok(SqlValue::Null),
        Value::String(v) => Ok(SqlValue::Text(v.clone())),
        Value::Number(v) if v.is_i64() => Ok(SqlValue::Integer(v.as_i64().unwrap())),
        Value::Number(v) => v
            .as_f64()
            .map(SqlValue::Real)
            .ok_or_else(|| backend::invalid("无效数值。")),
        _ => Err(backend::invalid("备份记录类型无效。")),
    }
}
impl Database {
    pub(crate) fn export_application_data(&self, categories: &[String]) -> Result<Data> {
        let mut data = Data::default();
        for table in TABLES {
            if allowed(table, categories) {
                let values = rows(&self.connection, table)?
                    .into_iter()
                    .filter_map(|mut r| keep_row(table, &mut r, categories).then_some(r))
                    .collect();
                data.tables.insert(table.into(), values);
            }
        }
        if selected(categories, "covers") {
            for r in rows(&self.connection, "collections")? {
                if let Some(id) = r["id"].as_str() {
                    data.collection_covers
                        .insert(id.into(), r["cover_path"].as_str().map(str::to_owned));
                }
            }
        }
        Ok(data)
    }
    pub(crate) fn restore_application_data(
        &mut self,
        data: &Data,
        categories: &[String],
    ) -> Result<()> {
        // Validate against the actual released schema and the chosen category policy.
        let reference = Database::in_memory()?;
        for (table, records) in &data.tables {
            if !TABLES.contains(&table.as_str()) || !allowed(table, categories) {
                return Err(backend::invalid("备份含未批准的数据类别。"));
            }
            let stmt = reference
                .connection
                .prepare(&format!("SELECT * FROM {table} LIMIT 0"))?;
            let columns = stmt.column_names();
            for row in records {
                let mut sanitized = row.clone();
                if row.keys().any(|k| !columns.contains(&k.as_str()))
                    || !keep_row(table, &mut sanitized, categories)
                    || sanitized != *row
                    || !row.get("id").is_some_and(Value::is_string)
                {
                    return Err(backend::invalid("备份字段超出所选范围。"));
                }
            }
        }
        if !selected(categories, "covers") && !data.collection_covers.is_empty() {
            return Err(backend::invalid("备份含未批准的分组封面。"));
        }
        let tx = self.connection.transaction()?;
        let retained_covers = if selected(categories, "groups") && !selected(categories, "covers") {
            rows(&tx, "collections")?
        } else {
            Vec::new()
        };
        // Base rows may be replaced only when every linked category is selected.
        let full = CATEGORIES.iter().all(|c| selected(categories, c));
        for table in TABLES.into_iter().rev() {
            if ![
                "games",
                "game_installations",
                "save_profiles",
                "settings",
                "metadata_records",
                "aliases",
            ]
            .contains(&table)
                && allowed(table, categories)
                || full && ["games", "game_installations", "save_profiles"].contains(&table)
            {
                tx.execute(&format!("DELETE FROM {table}"), [])?;
            }
        }
        if selected(categories, "metadata") {
            tx.execute("DELETE FROM metadata_cache", [])?;
            tx.execute(
                "DELETE FROM metadata_records WHERE field_name!='cover_path'",
                [],
            )?;
            tx.execute("DELETE FROM aliases", [])?;
        }
        if selected(categories, "covers") {
            tx.execute(
                "DELETE FROM metadata_records WHERE field_name='cover_path'",
                [],
            )?;
            tx.execute("UPDATE games SET cover_path=NULL", [])?;
        }
        // Remove only approved persistent setting namespaces; recovery/runtime keys are retained.
        let current_settings = rows(&tx, "settings")?;
        for row in current_settings {
            if row
                .get("key")
                .and_then(Value::as_str)
                .and_then(setting_category)
                .is_some_and(|c| selected(categories, c))
            {
                tx.execute(
                    "DELETE FROM settings WHERE id=?",
                    [row["id"].as_str().unwrap_or_default()],
                )?;
            }
        }
        for table in TABLES {
            let Some(records) = data.tables.get(table) else {
                continue;
            };
            for row in records {
                let keys = row.keys().cloned().collect::<Vec<_>>();
                let values = row.values().map(sql_value).collect::<Result<Vec<_>>>()?;
                let placeholders = vec!["?"; keys.len()].join(",");
                let assignments = keys
                    .iter()
                    .filter(|k| {
                        k.as_str() != "id"
                            && !(table == "games"
                                && !selected(categories, "metadata")
                                && ["title", "created_at", "updated_at"].contains(&k.as_str()))
                    })
                    .map(|k| format!("{k}=excluded.{k}"))
                    .collect::<Vec<_>>()
                    .join(",");
                let conflict = if assignments.is_empty()
                    || table == "game_installations" && !selected(categories, "settings")
                {
                    "DO NOTHING".into()
                } else {
                    format!("DO UPDATE SET {assignments}")
                };
                tx.execute(
                    &format!(
                        "INSERT INTO {table}({}) VALUES({placeholders}) ON CONFLICT(id) {conflict}",
                        keys.join(",")
                    ),
                    params_from_iter(values.iter()),
                )?;
            }
        }
        for row in retained_covers {
            tx.execute(
                "UPDATE collections SET cover_path=? WHERE id=?",
                rusqlite::params![sql_value(&row["cover_path"])?, sql_value(&row["id"])?],
            )?;
        }
        for (id, path) in &data.collection_covers {
            tx.execute(
                "UPDATE collections SET cover_path=? WHERE id=?",
                rusqlite::params![path, id],
            )?;
        }
        if tx
            .prepare("PRAGMA foreign_key_check")?
            .query([])?
            .next()?
            .is_some()
        {
            return Err(backend::invalid("备份存在无效归属关系。"));
        }
        if tx.execute("UPDATE settings SET value_json=json_set(value_json,'$.committed',json('true')) WHERE key='backup.restore.journal'",[])? > 0 {
            tx.execute("DELETE FROM settings WHERE key='backup.restore.pending'",[])?;
        }
        tx.commit()?;
        self.recover_interrupted()?;
        Ok(())
    }
    pub(crate) fn installation_paths(&self) -> Result<Vec<String>> {
        let mut statement = self
            .connection
            .prepare("SELECT absolute_path FROM game_installations")?;
        let paths = statement
            .query_map([], |row| row.get(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(paths)
    }
    pub(crate) fn refresh_game_ids(&self) -> Result<Vec<String>> {
        let mut stmt = self
            .connection
            .prepare("SELECT id FROM games WHERE NOT EXISTS(SELECT 1 FROM settings s WHERE s.key='metadata.locked.'||games.id AND s.value_json='true') ORDER BY created_at,id")?;
        let values = stmt
            .query_map([], |r| r.get(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(values)
    }
    pub(crate) fn clear_refresh_cache(&self, game: &str) -> Result<()> {
        self.connection.execute("DELETE FROM metadata_cache", [])?;
        self.connection.execute("DELETE FROM metadata_records WHERE game_id=? AND field_name LIKE '%_translation' AND is_user_edited=0",[game])?;
        Ok(())
    }
}
impl Database {
    pub(crate) fn metadata_cache_snapshot(&self) -> Result<Vec<Row>> {
        rows(&self.connection, "metadata_cache")
    }
    pub(crate) fn commit_metadata_refresh(
        &mut self,
        game: &str,
        expected: &str,
        staged: &Database,
        order: &[String],
    ) -> Result<()> {
        self.commit_metadata_refresh_with_cache(game, expected, staged, order, None)
    }
    pub(crate) fn commit_metadata_refresh_with_cache(
        &mut self,
        game: &str,
        expected: &str,
        staged: &Database,
        order: &[String],
        cache_before: Option<&[Row]>,
    ) -> Result<()> {
        self.ensure_metadata_unlocked(game)?;
        let live = serde_json::to_string(&self.get_game(game)?.metadata)
            .map_err(|_| backend::invalid("资料状态无效。"))?;
        if live != expected {
            return Err(backend::invalid("更新期间资料已改变，此游戏未写入。"));
        }
        let tx = self.connection.transaction()?;
        let before = super::metadata::current(&tx, game)?;
        let old = super::metadata::sourced(
            &tx,
            game,
            before.get("cover_path").and_then(|p| p.as_deref()),
        )?;
        let automatic = super::metadata::is_automatic_description(&tx, game, &before)?;
        tx.execute("DELETE FROM metadata_records WHERE game_id=? AND is_user_edited=0 AND provider!='manual'",[game])?;
        tx.execute(
            "DELETE FROM aliases WHERE game_id=? AND provider!='manual'",
            [game],
        )?;
        for table in ["metadata_records", "aliases"] {
            for row in rows(&staged.connection, table)?
                .into_iter()
                .filter(|r| r.get("game_id").and_then(Value::as_str) == Some(game))
            {
                if row.get("provider").and_then(Value::as_str) == Some("manual")
                    || row.get("is_user_edited").and_then(Value::as_i64) == Some(1)
                {
                    continue;
                }
                let keys = row.keys().cloned().collect::<Vec<_>>();
                let vals = row.values().map(sql_value).collect::<Result<Vec<_>>>()?;
                tx.execute(
                    &format!(
                        "INSERT INTO {table}({}) VALUES({})",
                        keys.join(","),
                        vec!["?"; keys.len()].join(",")
                    ),
                    params_from_iter(vals.iter()),
                )?;
            }
        }
        tx.execute("INSERT INTO settings(id,key,value_json) VALUES(?1,?2,?3) ON CONFLICT(key) DO UPDATE SET value_json=excluded.value_json",rusqlite::params![backend::id(),format!("metadata.priority.{game}"),serde_json::to_string(order).map_err(|_|backend::invalid("排序无效。"))?])?;
        let new = super::metadata::sourced(&tx, game, None)?;
        super::metadata::merge(&tx, game, &before, &old, &new, automatic)?;
        if let Some(before) = cache_before {
            for row in staged
                .metadata_cache_snapshot()?
                .into_iter()
                .filter(|r| !before.contains(r))
            {
                let keys = row.keys().cloned().collect::<Vec<_>>();
                let values = row.values().map(sql_value).collect::<Result<Vec<_>>>()?;
                let assignments = keys
                    .iter()
                    .filter(|key| !["id", "provider", "query"].contains(&key.as_str()))
                    .map(|key| format!("{key}=excluded.{key}"))
                    .collect::<Vec<_>>()
                    .join(",");
                tx.execute(&format!("INSERT INTO metadata_cache({}) VALUES({}) ON CONFLICT(provider,query) DO UPDATE SET {assignments} WHERE julianday(excluded.fetched_at)>=julianday(metadata_cache.fetched_at)", keys.join(","), vec!["?"; keys.len()].join(",")), params_from_iter(values.iter()))?;
            }
        }
        tx.commit()?;
        Ok(())
    }
}

impl Data {
    pub fn subset(&self, categories: &[String]) -> Self {
        let mut data = Data::default();
        for (table, rows) in &self.tables {
            if allowed(table, categories) {
                let records = rows
                    .iter()
                    .filter_map(|r| {
                        let mut r = r.clone();
                        keep_row(table, &mut r, categories).then_some(r)
                    })
                    .collect();
                data.tables.insert(table.clone(), records);
            }
        }
        if selected(categories, "covers") {
            data.collection_covers = self.collection_covers.clone();
        }
        data
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> Database {
        let db = Database::in_memory().unwrap();
        db.connection.execute("INSERT INTO games(id,title,description,cover_path,is_favorite) VALUES('g','Original','Private metadata','covers/private.png',1)",[]).unwrap();
        db.connection.execute("INSERT INTO game_installations(id,game_id,absolute_path,folder_name,environment_json) VALUES('i','g','C:/missing/game','Game','{\"SECRET\":\"private\"}')",[]).unwrap();
        db.connection.execute("INSERT INTO play_sessions(id,game_id,install_id,started_at,ended_at,duration_seconds) VALUES('s','g','i','2026-10-01T00:00:00Z','2026-10-01T00:01:00Z',60)",[]).unwrap();
        db.connection.execute("INSERT INTO notes(id,game_id,title,content_markdown) VALUES('n','g','Personal','Private notes')",[]).unwrap();
        db
    }
    #[test]
    fn metadata_lock_round_trips_with_metadata_backup_only() {
        let source = fixture();
        source.put_setting("metadata.locked.g", &true).unwrap();
        let metadata = vec!["metadata".into()];
        let exported = source.export_application_data(&metadata).unwrap();
        let mut restored = Database::in_memory().unwrap();
        restored
            .restore_application_data(&exported, &metadata)
            .unwrap();
        assert!(restored.get_game("g").unwrap().metadata_locked);
        let playtime_only = source
            .export_application_data(&["playtime".into()])
            .unwrap();
        assert!(!serde_json::to_string(&playtime_only)
            .unwrap()
            .contains("metadata.locked."));
        restored.remove_game_record("g").unwrap();
        assert!(restored
            .setting::<bool>("metadata.locked.g")
            .unwrap()
            .is_none());
    }
    #[test]
    fn selected_exports_do_not_contain_unselected_content_or_recovery_commands() {
        let db = fixture();
        db.put_setting("backup.restore.pending", &"foreign recovery")
            .unwrap();
        let chosen = vec!["playtime".into()];
        let data = db.export_application_data(&chosen).unwrap();
        let json = serde_json::to_string(&data).unwrap();
        assert!(!json.contains("Private metadata"));
        assert!(!json.contains("Private notes"));
        assert!(!json.contains("private.png"));
        assert!(!json.contains("SECRET"));
        assert!(!json.contains("foreign recovery"));
        let mut recovered = Database::in_memory().unwrap();
        recovered.restore_application_data(&data, &chosen).unwrap();
        assert_eq!(
            recovered
                .get_game("g")
                .unwrap()
                .summary
                .total_playtime_seconds,
            60
        );
        assert!(recovered.get_game("g").unwrap().description.is_none());
    }
    #[test]
    fn partial_restore_preserves_unselected_categories_and_manual_base_name() {
        let source = fixture();
        let chosen = vec!["playtime".into()];
        let data = source.export_application_data(&chosen).unwrap();
        let mut current = fixture();
        current
            .connection
            .execute(
                "UPDATE games SET title='New manual name',description='New metadata'",
                [],
            )
            .unwrap();
        current
            .connection
            .execute("UPDATE play_sessions SET duration_seconds=0", [])
            .unwrap();
        current.restore_application_data(&data, &chosen).unwrap();
        let detail = current.get_game("g").unwrap();
        assert_eq!(detail.summary.title, "New manual name");
        assert_eq!(detail.description.as_deref(), Some("New metadata"));
        assert_eq!(detail.summary.total_playtime_seconds, 60);
        let note: String = current
            .connection
            .query_row("SELECT content_markdown FROM notes", [], |r| r.get(0))
            .unwrap();
        assert_eq!(note, "Private notes");
    }
    #[test]
    fn personal_restore_preserves_current_title_and_creates_missing_base_rows() {
        let source = fixture();
        let categories = vec!["personal".into()];
        let data = source.export_application_data(&categories).unwrap();
        let mut current = fixture();
        current
            .connection
            .execute("UPDATE games SET title='New manual name',is_favorite=0", [])
            .unwrap();
        current
            .restore_application_data(&data, &categories)
            .unwrap();
        let game = current.get_game("g").unwrap();
        assert_eq!(game.summary.title, "New manual name");
        assert!(game.summary.favorite);
        let mut empty = Database::in_memory().unwrap();
        empty.restore_application_data(&data, &categories).unwrap();
        assert_eq!(empty.get_game("g").unwrap().summary.title, "Original");
    }
    #[test]
    fn group_restore_respects_cover_category_selection() {
        let source = fixture();
        source.connection.execute("INSERT INTO collections(id,name,kind,cover_path) VALUES('c','Backup name','normal','backup.png')", []).unwrap();
        for (categories, name, cover) in [
            (vec!["groups".to_string()], "Backup name", "current.png"),
            (vec!["covers".to_string()], "Current name", "backup.png"),
            (
                vec!["groups".to_string(), "covers".to_string()],
                "Backup name",
                "backup.png",
            ),
        ] {
            let mut current = fixture();
            current.connection.execute("INSERT INTO collections(id,name,kind,cover_path) VALUES('c','Current name','normal','current.png')", []).unwrap();
            let data = source.export_application_data(&categories).unwrap();
            current
                .restore_application_data(&data, &categories)
                .unwrap();
            let actual: (String, String) = current
                .connection
                .query_row(
                    "SELECT name,cover_path FROM collections WHERE id='c'",
                    [],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .unwrap();
            assert_eq!(actual, (name.to_string(), cover.to_string()));
        }
    }
    #[test]
    fn full_restore_removes_post_backup_games_without_touching_recovery_state() {
        let source = fixture();
        let chosen = CATEGORIES.iter().map(|c| c.to_string()).collect::<Vec<_>>();
        let data = source.export_application_data(&chosen).unwrap();
        let mut current = fixture();
        current
            .connection
            .execute(
                "INSERT INTO games(id,title) VALUES('new','Added after backup')",
                [],
            )
            .unwrap();
        current
            .put_setting("backup.restore.pending", &"owned pending")
            .unwrap();
        current.restore_application_data(&data, &chosen).unwrap();
        assert_eq!(current.refresh_game_ids().unwrap(), vec!["g"]);
        assert_eq!(
            current
                .setting::<String>("backup.restore.pending")
                .unwrap()
                .as_deref(),
            Some("owned pending")
        );
    }
    #[test]
    fn invalid_rows_roll_back_every_selected_category() {
        let mut current = fixture();
        let chosen = vec!["personal".into()];
        let mut data = current.export_application_data(&chosen).unwrap();
        data.tables.get_mut("notes").unwrap()[0]
            .insert("game_id".into(), Value::String("missing".into()));
        assert!(current.restore_application_data(&data, &chosen).is_err());
        assert_eq!(
            current
                .connection
                .query_row("SELECT count(*) FROM notes", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            1
        );
        data.tables.get_mut("notes").unwrap()[0]
            .insert("evil_column); DROP TABLE games;--".into(), Value::Null);
        assert!(current.restore_application_data(&data, &chosen).is_err());
        assert!(current.get_game("g").is_ok());
    }
}
