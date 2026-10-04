use super::Database;
use crate::{
    backend::{self, Result},
    domain::models::MatchResult,
};
use rusqlite::{params, Transaction};
use std::collections::BTreeMap;

type Projection = BTreeMap<String, Option<String>>;
// Keep the imported name in storage; all UI-facing queries share this projection.
// Older explicit names had no provenance: a name unlike every installation folder
// is retained as a manual name. New edits are explicitly recorded below.
pub(super) const DISPLAY_TITLE_SQL: &str = r#"CASE WHEN EXISTS(
    SELECT 1 FROM metadata_records m WHERE m.game_id=g.id AND m.provider='manual'
    AND m.field_name='title' AND m.is_user_edited=1
) OR (NOT EXISTS(SELECT 1 FROM settings s WHERE s.key='metadata.generated_title.'||g.id)
    AND EXISTS(SELECT 1 FROM game_installations i WHERE i.game_id=g.id)
    AND NOT EXISTS(SELECT 1 FROM game_installations i WHERE i.game_id=g.id AND trim(i.folder_name)=trim(g.title) COLLATE NOCASE)
) THEN g.title ELSE coalesce((
    SELECT trim(json_extract(m.value_json,'$')) FROM metadata_records m
    LEFT JOIN json_each(coalesce(
        (SELECT nullif(value_json,'[]') FROM settings WHERE key='metadata.priority.'||g.id),
        (SELECT (SELECT json_group_array(json_extract(j.value,'$.provider'))
            FROM json_each(s.value_json,'$.sources') j WHERE json_extract(j.value,'$.enabled')=1)
            FROM settings s WHERE s.key='metadata.sources'),
        '["hikarinagi","bangumi","vndb"]'
    )) o ON o.value=m.provider
    WHERE m.game_id=g.id AND (m.field_name IN ('title','title_zh','title_en','title_ja','title_alt') OR (m.field_name='title_zh_translation' AND coalesce((SELECT json_extract(value_json,'$.enabled') FROM settings WHERE key='metadata.translation'),0)=1 AND EXISTS(SELECT 1 FROM settings ts,json_each(ts.value_json,'$.fields') tf WHERE ts.key='metadata.translation' AND tf.value='title')))
    AND json_type(m.value_json)='text' AND trim(json_extract(m.value_json,'$'))!=''
    AND (m.is_user_edited=1 OR m.provider='manual' OR o.value IS NOT NULL)
    ORDER BY m.is_user_edited DESC, CASE WHEN m.provider='manual' THEN -1 ELSE o.key END,
        CASE WHEN m.field_name='title_zh_translation' THEN -1 WHEN m.field_name='title' THEN 0 ELSE 1 END, m.field_name
    LIMIT 1
),g.title) END"#;

pub(super) fn source_order(connection: &rusqlite::Connection, game: &str) -> Result<Vec<String>> {
    use rusqlite::OptionalExtension;
    let saved: Option<String> = connection
        .query_row(
            "SELECT value_json FROM settings WHERE key=?",
            [format!("metadata.priority.{game}")],
            |r| r.get(0),
        )
        .optional()?;
    if let Some(saved) = saved {
        let order: Vec<String> =
            serde_json::from_str(&saved).map_err(|_| backend::invalid("来源顺序无效。"))?;
        // Old prepared imports saved an empty priority; use the current source settings.
        if !order.is_empty() {
            return Ok(order);
        }
    }
    let global: Option<String> = connection
        .query_row(
            "SELECT value_json FROM settings WHERE key='metadata.sources'",
            [],
            |r| r.get(0),
        )
        .optional()?;
    // Historical records without saved preferences/priority retain their existing projection.
    // This is a read-only fallback, matching DISPLAY_TITLE_SQL; it never enables network sources.
    let Some(global) = global else {
        return Ok(["hikarinagi", "bangumi", "vndb"]
            .into_iter()
            .map(str::to_owned)
            .collect());
    };
    let config: backend::metadata_sources::Config =
        serde_json::from_str(&global).map_err(|_| backend::invalid("来源设置无效。"))?;
    config.validate()?;
    Ok(config.enabled())
}
const COLUMNS: [&str; 7] = [
    "title_zh",
    "title_ja",
    "title_en",
    "developer",
    "release_date",
    "description",
    "cover_path",
];
pub(super) fn current(tx: &Transaction<'_>, game: &str) -> Result<Projection> {
    Ok(tx.query_row("SELECT title_zh,title_ja,title_en,developer,release_date,description,cover_path FROM games WHERE id=?", [game], |r| {
        let mut values = BTreeMap::new();
        for (index, column) in COLUMNS.iter().enumerate() { values.insert((*column).to_owned(), r.get::<_, Option<String>>(index)?); }
        Ok(values)
    })?)
}
pub(super) fn sourced(
    tx: &Transaction<'_>,
    game: &str,
    legacy_cover: Option<&str>,
) -> Result<Projection> {
    let mut result = Projection::new();
    let mut ranks = BTreeMap::new();
    let order = source_order(tx, game)?;
    let mut statement = tx.prepare("SELECT provider,field_name,value_json,is_user_edited FROM metadata_records WHERE game_id=? ORDER BY is_user_edited DESC,field_name")?;
    let rows = statement.query_map([game], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, bool>(3)?,
        ))
    })?;
    let translation: backend::translation::Settings = tx
        .query_row(
            "SELECT value_json FROM settings WHERE key='metadata.translation'",
            [],
            |r| r.get::<_, String>(0),
        )
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    for row in rows {
        let (provider, field, value, manually_edited) = row?;
        let value = serde_json::from_str::<String>(&value).ok();
        let allowed_translation = match field.as_str() {
            "title_zh_translation" => translation.fields.iter().any(|f| f == "title"),
            "description_zh_translation" => translation.fields.iter().any(|f| f == "description"),
            _ => false,
        };
        if (field.ends_with("_translation") && !(translation.enabled && allowed_translation))
            || (!manually_edited && provider != "manual" && !order.contains(&provider))
        {
            continue;
        }
        let base = field.strip_suffix("_translation").unwrap_or(&field);
        let column = match (provider.as_str(), base) {
            ("vndb" | "hikarinagi", "title") => "title_en",
            ("bangumi", "title") | (_, "title_alt") => "title_ja",
            (_, "description_zh") => "description",
            (_, "cover_url") => {
                if let Some(path) = legacy_cover.filter(|path| path.starts_with("covers/")) {
                    result
                        .entry("cover_path".into())
                        .or_insert(Some(path.into()));
                }
                continue;
            }
            (_, name) => name,
        };
        if COLUMNS.contains(&column)
            && (manually_edited || value.as_ref().is_some_and(|value| !value.trim().is_empty()))
        {
            let source = if provider == "manual" {
                0
            } else {
                order
                    .iter()
                    .position(|p| p == &provider)
                    .map_or(99, |i| i + 1)
            };
            let rank = (
                !manually_edited,
                source,
                // Source order stays first. Within Hikarinagi, its authoritative
                // trans_intro field wins without guessing the prose's language again.
                if provider == "hikarinagi" && field == "description_zh" {
                    0
                } else if field.ends_with("_translation") {
                    1
                } else if field == "description" {
                    2
                } else {
                    3
                },
            );
            if ranks.get(column).is_none_or(|previous| rank < *previous) {
                ranks.insert(column.to_owned(), rank);
                result.insert(column.into(), value.filter(|v| !v.trim().is_empty()));
            }
        }
    }
    Ok(result)
}
pub(super) fn is_automatic_description(
    tx: &Transaction<'_>,
    game: &str,
    before: &Projection,
) -> Result<bool> {
    let Some(value) = before.get("description").and_then(Option::as_ref) else {
        return Ok(false);
    };
    let encoded = serde_json::to_string(value).map_err(|_| backend::invalid("简介字段无效。"))?;
    Ok(tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM metadata_records WHERE game_id=?1 AND field_name IN ('description','description_zh','description_zh_translation') AND is_user_edited=0 AND value_json=?2) AND NOT EXISTS(SELECT 1 FROM metadata_records WHERE game_id=?1 AND field_name IN ('description','description_zh','description_zh_translation') AND is_user_edited=1)",
        params![game, encoded],
        |row| row.get(0),
    )?)
}
pub(super) fn merge(
    tx: &Transaction<'_>,
    game: &str,
    before: &Projection,
    old: &Projection,
    new: &Projection,
    automatic_description: bool,
) -> Result<()> {
    for column in COLUMNS {
        let value = before.get(column).cloned().flatten();
        let old_value = old.get(column).cloned().flatten();
        // A value that does not equal the old provider projection belongs to local/manual data.
        if value.is_some()
            && value != old_value
            && !(column == "description" && automatic_description)
        {
            continue;
        }
        let next = new.get(column).cloned().flatten();
        tx.execute(
            &format!("UPDATE games SET {column}=?1,updated_at=?2 WHERE id=?3"),
            params![next, backend::now(), game],
        )?;
    }
    Ok(())
}
impl Database {
    pub(crate) fn save_translation_config(
        &mut self,
        settings: &backend::translation::Settings,
    ) -> Result<()> {
        let games = {
            let mut stmt = self.connection.prepare("SELECT id FROM games")?;
            let values = stmt
                .query_map([], |r| r.get::<_, String>(0))?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            values
        };
        let tx = self.connection.transaction()?;
        let mut previous = Vec::new();
        for game in games {
            if tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM settings WHERE key=? AND value_json='true')",
                [format!("metadata.locked.{game}")],
                |r| r.get::<_, bool>(0),
            )? {
                continue;
            }
            let before = current(&tx, &game)?;
            let old = sourced(
                &tx,
                &game,
                before.get("cover_path").and_then(|p| p.as_deref()),
            )?;
            previous.push((game, before, old));
        }
        tx.execute("INSERT INTO settings(id,key,value_json) VALUES(?1,'metadata.translation',?2) ON CONFLICT(key) DO UPDATE SET value_json=excluded.value_json", params![backend::id(),serde_json::to_string(settings).map_err(|_|backend::invalid("翻译配置无效。"))?])?;
        for (game, before, old) in previous {
            let new = sourced(&tx, &game, None)?;
            let automatic = is_automatic_description(&tx, &game, &before)?;
            merge(&tx, &game, &before, &old, &new, automatic)?;
        }
        tx.commit()?;
        Ok(())
    }

    pub(crate) fn limit_source_tags(&mut self, game: &str, limit: usize) -> Result<()> {
        self.ensure_metadata_unlocked(game)?;
        let order = source_order(&self.connection, game)?;
        let tx = self.connection.transaction()?;
        let mut seen = std::collections::HashSet::new();
        for provider in order {
            let mut stmt = tx.prepare("SELECT id,value_json FROM metadata_records WHERE game_id=?1 AND provider=?2 AND field_name='source_tags' AND is_user_edited=0")?;
            let rows = stmt
                .query_map(params![game, provider], |r| {
                    Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
                })?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            for (id, raw) in rows {
                let raw: String = serde_json::from_str(&raw).unwrap_or_default();
                let tags = raw
                    .lines()
                    .map(str::trim)
                    .filter(|t| !t.is_empty())
                    .filter(|t| seen.len() < limit && seen.insert(t.to_lowercase()))
                    .collect::<Vec<_>>()
                    .join("\n");
                tx.execute(
                    "UPDATE metadata_records SET value_json=?1 WHERE id=?2",
                    params![
                        serde_json::to_string(&tags).map_err(|_| backend::invalid("标签无效。"))?,
                        id
                    ],
                )?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    pub(crate) fn metadata_binding_revision(
        &self,
        game: &str,
        provider: &str,
        remote: &str,
    ) -> Result<Vec<(String, String, String)>> {
        let mut stmt=self.connection.prepare("SELECT field_name,fetched_at,value_json FROM metadata_records WHERE game_id=?1 AND provider=?2 AND remote_id=?3 AND is_user_edited=0 ORDER BY field_name")?;
        let values = stmt
            .query_map(params![game, provider, remote], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(values)
    }

    pub(crate) fn cover_is_referenced(&self, path: &str) -> Result<bool> {
        Ok(self.connection.query_row("SELECT EXISTS(SELECT 1 FROM games WHERE cover_path=?1) OR EXISTS(SELECT 1 FROM metadata_records WHERE field_name='cover_path' AND value_json=?2)",params![path,serde_json::to_string(path).map_err(|_|backend::invalid("缓存路径无效。"))?],|r|r.get(0))?)
    }

    pub(crate) fn set_metadata_priority(&mut self, game: &str, order: &[String]) -> Result<()> {
        self.ensure_metadata_unlocked(game)?;
        let tx = self.connection.transaction()?;
        let before = current(&tx, game)?;
        let old = sourced(
            &tx,
            game,
            before.get("cover_path").and_then(|v| v.as_deref()),
        )?;
        tx.execute("INSERT INTO settings(id,key,value_json,updated_at) VALUES(?4,?1,?2,?3) ON CONFLICT(key) DO UPDATE SET value_json=excluded.value_json,updated_at=excluded.updated_at",params![format!("metadata.priority.{game}"),serde_json::to_string(order).map_err(|_|backend::invalid("排序无效。"))?,backend::now(),backend::id()])?;
        let new = sourced(&tx, game, None)?;
        merge(&tx, game, &before, &old, &new, false)?;
        tx.commit()?;
        Ok(())
    }

    pub(crate) fn translation_revision(&self, game: &str) -> Result<Vec<(String, Option<String>)>> {
        let mut statement = self
            .connection
            .prepare("SELECT id,remote_id FROM metadata_records WHERE game_id=? ORDER BY id")?;
        let rows = statement.query_map([game], |row| Ok((row.get(0)?, row.get(1)?)))?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }
    /// Persist a complete translation batch atomically, keeping each original binding and URL.
    pub(crate) fn apply_translation_fields(
        &mut self,
        game: &str,
        fields: &[(String, String, String, String)],
    ) -> Result<()> {
        self.ensure_metadata_unlocked(game)?;
        let tx = self.connection.transaction()?;
        let before = current(&tx, game)?;
        let old = sourced(
            &tx,
            game,
            before.get("cover_path").and_then(|value| value.as_deref()),
        )?;
        let automatic_description = is_automatic_description(&tx, game, &before)?;
        for (provider, original_field, output_field, value) in fields {
            tx.execute("INSERT INTO metadata_records(id,game_id,provider,remote_id,field_name,value_json,source_url,fetched_at) SELECT ?1,game_id,provider,remote_id,?2,?3,source_url,?4 FROM metadata_records WHERE game_id=?5 AND provider=?6 AND field_name=?7 AND is_user_edited=0 AND remote_id IS NOT NULL ON CONFLICT(game_id,provider,field_name) DO UPDATE SET remote_id=excluded.remote_id,value_json=excluded.value_json,source_url=excluded.source_url,fetched_at=excluded.fetched_at WHERE metadata_records.is_user_edited=0", params![backend::id(),output_field,serde_json::to_string(value).map_err(|_|backend::invalid("译文字段无效。"))?,backend::now(),game,provider,original_field])?;
        }
        let new = sourced(
            &tx,
            game,
            before.get("cover_path").and_then(|value| value.as_deref()),
        )?;
        merge(&tx, game, &before, &old, &new, automatic_description)?;
        tx.commit()?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn apply_remote_fields(
        &mut self,
        game: &str,
        provider: &str,
        remote_id: &str,
        fields: &[(String, String)],
        aliases: &[(String, String)],
        fetched_at: &str,
        replace: bool,
    ) -> Result<MatchResult> {
        self.get_game(game)?;
        self.ensure_metadata_unlocked(game)?;
        let tx = self.connection.transaction()?;
        let before = current(&tx, game)?;
        let old = sourced(
            &tx,
            game,
            before.get("cover_path").and_then(|v| v.as_deref()),
        )?;
        let automatic_description = is_automatic_description(&tx, game, &before)?;
        let rebound = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM metadata_records WHERE game_id=?1 AND provider=?2 AND is_user_edited=0 AND remote_id IS NOT NULL AND remote_id!=?3)",
            params![game, provider, remote_id],
            |row| row.get::<_, bool>(0),
        )?;
        if replace || rebound {
            tx.execute("DELETE FROM metadata_records WHERE game_id=?1 AND provider=?2 AND is_user_edited=0", params![game,provider])?;
            tx.execute(
                "DELETE FROM aliases WHERE game_id=?1 AND provider=?2",
                params![game, provider],
            )?;
        }
        let url = match provider {
            "vndb" => format!("https://vndb.org/{remote_id}"),
            "bangumi" => format!("https://bgm.tv/subject/{remote_id}"),
            "hikarinagi" => format!("https://www.hikarinagi.org/galgames/{remote_id}"),
            _ => format!("https://www.hikarinagi.org/galgames/{remote_id}"),
        };
        for (field, value) in fields.iter().filter(|(_, value)| !value.trim().is_empty()) {
            let translation = match field.as_str() {
                "title" | "title_zh" | "title_en" | "title_ja" | "title_alt" => {
                    Some("title_zh_translation")
                }
                "description" | "description_zh" => Some("description_zh_translation"),
                "developer" => Some("developer_translation"),
                "publisher" => Some("publisher_translation"),
                "source_tags" => Some("source_tags_translation"),
                _ => None,
            };
            if let Some(translation) = translation {
                tx.execute("DELETE FROM metadata_records WHERE game_id=?1 AND provider=?2 AND field_name=?3 AND is_user_edited=0 AND EXISTS(SELECT 1 FROM metadata_records original WHERE original.game_id=?1 AND original.provider=?2 AND original.field_name=?4 AND original.value_json!=?5)", params![game,provider,translation,field,serde_json::to_string(value).map_err(|_|backend::invalid("资料字段无效。"))?])?;
            }
            tx.execute("INSERT INTO metadata_records(id,game_id,provider,remote_id,field_name,value_json,source_url,fetched_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8) ON CONFLICT(game_id,provider,field_name) DO UPDATE SET remote_id=excluded.remote_id,value_json=excluded.value_json,source_url=excluded.source_url,fetched_at=excluded.fetched_at WHERE metadata_records.is_user_edited=0", params![backend::id(),game,provider,remote_id,field,serde_json::to_string(value).map_err(|_|backend::invalid("资料字段无效。"))?,url,fetched_at])?;
        }
        for (alias, language) in aliases.iter().filter(|(alias, _)| !alias.trim().is_empty()) {
            tx.execute("INSERT INTO aliases(id,game_id,alias,language,provider) VALUES(?1,?2,?3,?4,?5) ON CONFLICT(game_id,alias,language) DO NOTHING", params![backend::id(),game,alias,language,provider])?;
        }
        let new = sourced(&tx, game, None)?;
        merge(&tx, game, &before, &old, &new, automatic_description)?;
        tx.execute(
            "DELETE FROM settings WHERE key=?",
            [format!("metadata.candidates.{game}")],
        )?;
        tx.commit()?;
        Ok(MatchResult {
            game_id: game.into(),
            provider: provider.into(),
            remote_id: remote_id.into(),
            matched_at: fetched_at.into(),
            translation_message: None,
            supplementation_message: None,
            cover_message: None,
        })
    }
    pub fn unbind_metadata(&mut self, game: &str, provider: &str) -> Result<()> {
        self.get_game(game)?;
        self.ensure_metadata_unlocked(game)?;
        let tx = self.connection.transaction()?;
        let before = current(&tx, game)?;
        let old = sourced(
            &tx,
            game,
            before.get("cover_path").and_then(|v| v.as_deref()),
        )?;
        let automatic_description = is_automatic_description(&tx, game, &before)?;
        // Retain hand-edited provenance, but detach its remote identity.
        tx.execute("UPDATE metadata_records SET remote_id=NULL,source_url=NULL WHERE game_id=?1 AND provider=?2 AND is_user_edited=1", params![game,provider])?;
        tx.execute(
            "DELETE FROM metadata_records WHERE game_id=?1 AND provider=?2 AND is_user_edited=0",
            params![game, provider],
        )?;
        tx.execute(
            "DELETE FROM aliases WHERE game_id=?1 AND provider=?2",
            params![game, provider],
        )?;
        let new = sourced(&tx, game, None)?;
        merge(&tx, game, &before, &old, &new, automatic_description)?;
        tx.execute(
            "DELETE FROM settings WHERE key=?",
            [format!("metadata.candidates.{game}")],
        )?;
        if provider == "bangumi" {
            tx.execute(
                "DELETE FROM settings WHERE key=?",
                [format!("bangumi.cover.{game}")],
            )?;
        }
        tx.commit()?;
        Ok(())
    }
}
