use super::{metadata::DISPLAY_TITLE_SQL, Database};
use crate::{
    backend::{self, types::*, Result, ServiceError},
    domain::{
        models::*,
        protocol::*,
        requests::{RecommendationQuery, SetRecommendationPreferenceRequest},
    },
};
use rusqlite::{params, params_from_iter, types::Value, OptionalExtension};
use std::path::Path;

pub(super) fn unsigned(row: &rusqlite::Row<'_>, index: usize) -> rusqlite::Result<u64> {
    let value: i64 = row.get(index)?;
    value.try_into().map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(
            index,
            rusqlite::types::Type::Integer,
            Box::new(e),
        )
    })
}

fn decode<T: serde::de::DeserializeOwned>(value: &str) -> Result<T> {
    serde_json::from_str(value)
        .map_err(|_| ServiceError(ErrorCode::DatabaseError, "数据库中的结构化记录无效。"))
}
fn encode(value: &impl serde::Serialize) -> Result<String> {
    serde_json::to_string(value).map_err(|_| backend::invalid("数据无法序列化。"))
}

fn metadata_publisher(fields: &[SourcedField]) -> Option<String> {
    fields
        .iter()
        .filter(|f| f.field == "publisher")
        .filter_map(|f| serde_json::from_str::<String>(&f.value).ok())
        .map(|s| s.trim().to_owned())
        .find(|s| !s.is_empty())
}
fn metadata_tags(
    fields: &[SourcedField],
    translation: &crate::backend::translation::Settings,
) -> Vec<String> {
    let translate = translation.enabled && translation.fields.iter().any(|f| f == "source_tags");
    let mut tags = Vec::new();
    for field in fields
        .iter()
        .filter(|f| f.field == "source_tags" || translate && f.field == "source_tags_translation")
    {
        if field.field == "source_tags"
            && translate
            && fields
                .iter()
                .any(|f| f.provider == field.provider && f.field == "source_tags_translation")
        {
            continue;
        }
        if let Ok(raw) = serde_json::from_str::<String>(&field.value) {
            for tag in raw.lines().map(str::trim).filter(|t| !t.is_empty()) {
                if !tags.iter().any(|t: &String| t.eq_ignore_ascii_case(tag)) {
                    tags.push(tag.to_owned());
                }
            }
        }
    }
    tags
}
fn metadata_rating(fields: &[SourcedField]) -> Option<f64> {
    let parse = |field: &SourcedField| {
        serde_json::from_str::<String>(&field.value)
            .ok()?
            .parse::<f64>()
            .ok()
            .filter(|n| n.is_finite() && (0.0..=10.0).contains(n))
    };
    if let Some(manual) = fields
        .iter()
        .find(|f| f.field == "source_rating" && (f.manually_edited || f.provider == "manual"))
    {
        return parse(manual);
    }
    fields
        .iter()
        .filter(|f| f.field == "source_rating")
        .find_map(parse)
}
pub fn validate_query(q: &GameQuery) -> Result<()> {
    if q.page == 0
        || !(1..=100).contains(&q.page_size)
        || q.search.len() > 1024
        || q.tag_ids.len() > 100
        || q.statuses.len() > 6
        || q.sources.len() > 4
    {
        return Err(backend::invalid("分页或筛选参数超出范围。"));
    }
    let f = &q.filters;
    if f.developer.as_ref().is_some_and(|s| s.len() > 1024)
        || f.release_year.is_some_and(|y| !(1900..=2200).contains(&y))
        || f.min_playtime_seconds
            .zip(f.max_playtime_seconds)
            .is_some_and(|(a, b)| a > b)
        || [f.min_playtime_seconds, f.max_playtime_seconds]
            .into_iter()
            .flatten()
            .any(|s| s > i64::MAX as u64)
        || f.played_after
            .as_ref()
            .is_some_and(|s| chrono::DateTime::parse_from_rfc3339(s).is_err())
    {
        return Err(backend::invalid("附加筛选参数无效。"));
    }
    Ok(())
}
fn default_query() -> GameQuery {
    GameQuery {
        page: 1,
        page_size: 100,
        search: String::new(),
        statuses: vec![],
        sources: vec![],
        tag_ids: vec![],
        favorite: None,
        collection_id: None,
        sort: GameSort::AddedAt,
        direction: SortDirection::Desc,
        filters: GameFilters::default(),
    }
}
fn replace_members_transaction(
    tx: &rusqlite::Transaction<'_>,
    collection_id: &str,
    game_ids: &[String],
) -> Result<()> {
    if game_ids.len() > 10000
        || game_ids
            .iter()
            .collect::<std::collections::HashSet<_>>()
            .len()
            != game_ids.len()
    {
        return Err(backend::invalid("分组成员不能重复，数量不能超过 10000。"));
    }
    for id in game_ids {
        if !tx.query_row("SELECT EXISTS(SELECT 1 FROM games WHERE id=?)", [id], |r| {
            r.get::<_, bool>(0)
        })? {
            return Err(backend::missing());
        }
    }
    tx.execute(
        "DELETE FROM collection_members WHERE collection_id=?",
        [collection_id],
    )?;
    for (position, id) in game_ids.iter().enumerate() {
        tx.execute(
            "INSERT INTO collection_members(id,collection_id,game_id,position) VALUES(?1,?2,?3,?4)",
            params![backend::id(), collection_id, id, position as i64],
        )?;
    }
    Ok(())
}
fn filters(q: &GameQuery, values: &mut Vec<Value>) -> String {
    let mut clauses = vec!["1=1".to_string()];
    if !q.search.trim().is_empty() {
        // LIKE escaping makes % and _ literal input, and all values are parameters.
        let search = q
            .search
            .trim()
            .replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_");
        clauses.push("(g.title LIKE ? ESCAPE '\\' OR coalesce(g.title_zh,'') LIKE ? ESCAPE '\\' OR coalesce(g.title_ja,'') LIKE ? ESCAPE '\\' OR coalesce(g.title_en,'') LIKE ? ESCAPE '\\' OR EXISTS(SELECT 1 FROM aliases a WHERE a.game_id=g.id AND a.alias LIKE ? ESCAPE '\\') OR EXISTS(SELECT 1 FROM game_tags gt JOIN tags t ON t.id=gt.tag_id WHERE gt.game_id=g.id AND t.name LIKE ? ESCAPE '\\') OR EXISTS(SELECT 1 FROM game_installations i WHERE i.game_id=g.id AND i.folder_name LIKE ? ESCAPE '\\'))".into());
        for _ in 0..7 {
            values.push(format!("%{search}%").into());
        }
    }
    for (column, list) in [
        (
            "g.status",
            q.statuses
                .iter()
                .map(backend::enum_text)
                .collect::<Vec<_>>(),
        ),
        (
            "i.source",
            q.sources.iter().map(backend::enum_text).collect(),
        ),
    ] {
        if list.is_empty() {
            continue;
        }
        let predicate = format!("{column} IN ({})", vec!["?"; list.len()].join(","));
        clauses.push(if column == "i.source" {
            format!(
                "EXISTS(SELECT 1 FROM game_installations i WHERE i.game_id=g.id AND {predicate})"
            )
        } else {
            predicate
        });
        values.extend(list.into_iter().map(Value::from));
    }
    if let Some(favorite) = q.favorite {
        clauses.push("g.is_favorite=?".into());
        values.push(i64::from(favorite).into());
    }
    for tag in &q.tag_ids {
        clauses
            .push("EXISTS(SELECT 1 FROM game_tags t WHERE t.game_id=g.id AND t.tag_id=?)".into());
        values.push(tag.clone().into());
    }
    let f = &q.filters;
    if let Some(developer) = &f.developer {
        clauses.push("g.developer=? COLLATE NOCASE".into());
        values.push(developer.clone().into());
    }
    if let Some(year) = f.release_year {
        clauses.push("substr(g.release_date,1,4)=?".into());
        values.push(year.to_string().into());
    }
    for (op, seconds) in [
        (">=", f.min_playtime_seconds),
        ("<=", f.max_playtime_seconds),
    ] {
        if let Some(seconds) = seconds {
            clauses.push(format!("(SELECT coalesce(sum(duration_seconds),0) FROM play_sessions WHERE game_id=g.id) {op} ?"));
            values.push((seconds as i64).into());
        }
    }
    if let Some(after) = &f.played_after {
        clauses.push("EXISTS(SELECT 1 FROM play_sessions WHERE game_id=g.id AND (end_reason IS NULL OR end_reason!='launch_failed') AND julianday(started_at)>=julianday(?))".into());
        values.push(after.clone().into());
    }
    for (condition, enabled) in [
        (
            "EXISTS(SELECT 1 FROM settings WHERE key='metadata.candidates.'||g.id)",
            f.metadata_pending,
        ),
        (
            "EXISTS(SELECT 1 FROM save_snapshots s JOIN save_profiles p ON p.id=s.profile_id WHERE p.game_id=g.id)",
            f.has_save_backup,
        ),
        (
            "(SELECT count(*) FROM game_installations WHERE game_id=g.id)>1",
            f.multiple_installations,
        ),
        (
            "(nullif(trim(g.cover_path),'') IS NULL OR nullif(trim(g.developer),'') IS NULL)",
            f.metadata_incomplete,
        ),
    ] {
        if let Some(enabled) = enabled {
            clauses.push(format!("({condition})=?"));
            values.push(i64::from(enabled).into());
        }
    }
    clauses.join(" AND ")
}

impl Database {
    pub fn snapshot_to(&self, path: &Path) -> Result<()> {
        self.connection
            .execute("VACUUM INTO ?", [backend::path_text(path)?])?;
        Ok(())
    }
    pub fn setting<T: serde::de::DeserializeOwned>(&self, key: &str) -> Result<Option<T>> {
        let value: Option<String> = self
            .connection
            .query_row("SELECT value_json FROM settings WHERE key=?", [key], |r| {
                r.get(0)
            })
            .optional()?;
        value.map(|s| decode(&s)).transpose()
    }
    pub fn put_setting(&self, key: &str, value: &impl serde::Serialize) -> Result<()> {
        self.connection.execute("INSERT INTO settings(id,key,value_json) VALUES(?1,?2,?3) ON CONFLICT(key) DO UPDATE SET value_json=excluded.value_json,updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now')",params![backend::id(),key,encode(value)?])?;
        if key.starts_with("scan.task.") {
            self.connection.execute("DELETE FROM settings WHERE id IN (SELECT id FROM settings WHERE key LIKE 'scan.task.%' AND key!=? ORDER BY julianday(updated_at) DESC,id DESC LIMIT -1 OFFSET 31)",[key])?;
        }
        Ok(())
    }
    pub fn delete_setting(&self, key: &str) -> Result<()> {
        self.connection
            .execute("DELETE FROM settings WHERE key=?", [key])?;
        Ok(())
    }
    pub fn has_metadata_candidates(&self, game_id: &str) -> Result<bool> {
        Ok(self.connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM settings WHERE key=?)",
            [format!("metadata.candidates.{game_id}")],
            |row| row.get(0),
        )?)
    }
    pub fn recover_interrupted(&mut self) -> Result<()> {
        // Recover only durably observed time; never count the offline interval.
        self.recover_play_sessions()?;
        let mut stmt = self
            .connection
            .prepare("SELECT key,value_json FROM settings WHERE key LIKE 'scan.task.%'")?;
        let rows = stmt
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        drop(stmt);
        for (key, value) in rows {
            let mut report: ScanReport = decode(&value)?;
            if matches!(
                report.progress.status,
                TaskStatus::Running | TaskStatus::Queued | TaskStatus::Paused
            ) {
                report.progress.status = TaskStatus::Failed;
                report.progress.phase = "interrupted".into();
                report.progress.message =
                    "应用退出导致扫描中断，已导入记录保留；请重新扫描。".into();
                self.put_setting(&key, &report)?;
            }
        }
        Ok(())
    }
    pub fn list_games(&self, q: &GameQuery) -> Result<Paginated<GameSummary>> {
        validate_query(q)?;
        let mut values = vec![];
        let mut predicate = filters(q, &mut values);
        if let Some(id) = &q.collection_id {
            let c = self.get_collection(id)?;
            if let Some(query) = c.query {
                predicate.push_str(&format!(" AND ({})", filters(&query, &mut values)));
            } else {
                predicate.push_str(" AND EXISTS(SELECT 1 FROM collection_members m WHERE m.game_id=g.id AND m.collection_id=?)");
                values.push(id.clone().into());
            }
        }
        let total: u64 = self.connection.query_row(
            &format!("SELECT count(*) FROM games g WHERE {predicate}"),
            params_from_iter(values.iter()),
            |r| unsigned(r, 0),
        )?;
        let title_sort = format!("{DISPLAY_TITLE_SQL} COLLATE NOCASE");
        let sort = match q.sort {
            GameSort::Title => &title_sort,
            GameSort::AddedAt => "julianday(g.created_at)",
            GameSort::LastPlayedAt => {
                "(SELECT max(julianday(started_at)) FROM play_sessions WHERE game_id=g.id AND (end_reason IS NULL OR end_reason!='launch_failed'))"
            }
            GameSort::Playtime => {
                "(SELECT coalesce(sum(duration_seconds),0) FROM play_sessions WHERE game_id=g.id)"
            }
        };
        let dir = match q.direction {
            SortDirection::Asc => "ASC",
            SortDirection::Desc => "DESC",
        };
        values.push(i64::from(q.page_size).into());
        values.push((i64::from(q.page - 1) * i64::from(q.page_size)).into());
        let mut stmt=self.connection.prepare(&format!("SELECT g.id FROM games g WHERE {predicate} ORDER BY {sort} {dir},g.id ASC LIMIT ? OFFSET ?"))?;
        let ids = stmt
            .query_map(params_from_iter(values.iter()), |r| r.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let items = ids
            .iter()
            .map(|id| self.get_game(id).map(|g| g.summary))
            .collect::<Result<Vec<_>>>()?;
        Ok(Paginated {
            items,
            page: q.page,
            page_size: q.page_size,
            total,
        })
    }
    pub fn get_game(&self, id: &str) -> Result<GameDetail> {
        let row=self.connection.query_row(&format!("SELECT {DISPLAY_TITLE_SQL},g.title_zh,g.title_ja,g.title_en,g.developer,g.release_date,g.status,g.is_favorite,g.is_hidden,g.user_rating,g.created_at,g.description,g.cover_path FROM games g WHERE g.id=?"),[id],|r|Ok((r.get::<_,String>(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get::<_,String>(6)?,r.get(7)?,r.get(8)?,r.get(9)?,r.get(10)?,r.get(11)?,r.get(12)?))).optional()?.ok_or_else(backend::missing)?;
        let mut stmt=self.connection.prepare("SELECT id,absolute_path,executable_path,source,steam_app_id FROM game_installations WHERE game_id=? ORDER BY created_at,id")?;
        let installations = stmt
            .query_map([id], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, Option<String>>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, Option<i64>>(4)?,
                ))
            })?
            .map(|r| {
                let (i, path, exe, source, app) = r?;
                Ok(GameInstallation {
                    id: i,
                    game_id: id.into(),
                    path_valid: Path::new(&path).is_dir()
                        && exe.as_ref().is_none_or(|e| Path::new(e).is_file()),
                    absolute_path: path,
                    executable_path: exe,
                    source: decode(&format!("\"{source}\""))?,
                    steam_app_id: app.map(|a| a.to_string()),
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let mut stmt=self.connection.prepare("SELECT t.id,t.name,t.color FROM tags t JOIN game_tags gt ON gt.tag_id=t.id WHERE gt.game_id=? ORDER BY t.name")?;
        let tags = stmt
            .query_map([id], |r| {
                Ok(Tag {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    color: r.get(2)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let(total_playtime_seconds,last_played_at)=self.connection.query_row("SELECT coalesce(sum(duration_seconds),0),(SELECT started_at FROM play_sessions WHERE game_id=?1 AND (end_reason IS NULL OR end_reason!='launch_failed') ORDER BY julianday(started_at) DESC LIMIT 1) FROM play_sessions WHERE game_id=?1",[id],|r|Ok((unsigned(r,0)?,r.get(1)?)))?;
        let mut stmt=self.connection.prepare("SELECT field_name,value_json,provider,fetched_at,is_user_edited,remote_id FROM metadata_records WHERE game_id=? ORDER BY is_user_edited DESC,field_name")?;
        let mut metadata = stmt
            .query_map([id], |r| {
                Ok(SourcedField {
                    remote_id: r.get(5)?,
                    field: r.get(0)?,
                    value: r.get(1)?,
                    provider: r.get(2)?,
                    fetched_at: r.get(3)?,
                    cached: false,
                    manually_edited: r.get(4)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let order = super::metadata::source_order(&self.connection, id)?;
        metadata.sort_by_key(|field| {
            (
                !field.manually_edited,
                order
                    .iter()
                    .position(|p| p == &field.provider)
                    .unwrap_or(99),
            )
        });
        let active_metadata = metadata
            .iter()
            .filter(|f| f.manually_edited || f.provider == "manual" || order.contains(&f.provider))
            .cloned()
            .collect::<Vec<_>>();
        let publisher = metadata_publisher(&active_metadata);
        let source_rating = metadata_rating(&active_metadata);
        let translation = self
            .setting::<crate::backend::translation::Settings>("metadata.translation")?
            .unwrap_or_default();
        let source_tags = metadata_tags(&active_metadata, &translation);
        let metadata_status = if self.has_metadata_candidates(id)? {
            MetadataStatus::PendingConfirmation
        } else if metadata
            .iter()
            .any(|m| m.provider != "local" && m.provider != "manual")
        {
            MetadataStatus::Synced
        } else {
            MetadataStatus::LocalOnly
        };
        Ok(GameDetail {
            metadata_locked: self.metadata_locked(id)?,
            summary: GameSummary {
                hikari_field: self.hf_entry(id)?,
                id: id.into(),
                title: row.0,
                title_zh: row.1,
                title_ja: row.2,
                title_en: row.3,
                developer: row.4,
                release_date: row.5,
                status: decode(&format!("\"{}\"", row.6))?,
                favorite: row.7,
                hidden: row.8,
                user_rating: row.9,
                added_at: row.10,
                cover_url: row.12,
                total_playtime_seconds,
                last_played_at,
                metadata_status,
                publisher,
                source_rating,
                source_tags,
                installations,
                tags,
                has_save_backup: self.connection.query_row(
                    "SELECT EXISTS(SELECT 1 FROM save_snapshots s JOIN save_profiles p ON p.id=s.profile_id WHERE p.game_id=?)",
                    [id],
                    |r| r.get(0),
                )?,
            },
            description: row.11,
            metadata,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn apply_vndb_match(
        &mut self,
        game_id: &str,
        remote_id: &str,
        title: &str,
        title_zh: Option<&str>,
        title_en: Option<&str>,
        alt_title: Option<&str>,
        developer: Option<&str>,
        release_date: Option<&str>,
        description: Option<&str>,
        description_zh: Option<&str>,
        fetched_at: &str,
    ) -> Result<MatchResult> {
        self.apply_vndb_match_with_details(
            game_id,
            remote_id,
            title,
            title_zh,
            title_en,
            alt_title,
            developer,
            None,
            release_date,
            None,
            &[],
            description,
            description_zh,
            fetched_at,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn apply_vndb_match_with_details(
        &mut self,
        game_id: &str,
        remote_id: &str,
        title: &str,
        title_zh: Option<&str>,
        title_en: Option<&str>,
        alt_title: Option<&str>,
        developer: Option<&str>,
        publisher: Option<&str>,
        release_date: Option<&str>,
        source_rating: Option<f64>,
        source_tags: &[String],
        description: Option<&str>,
        description_zh: Option<&str>,
        fetched_at: &str,
    ) -> Result<MatchResult> {
        if remote_id.trim().is_empty() || title.trim().is_empty() {
            return Err(backend::invalid("VNDB 匹配资料缺少作品标识或标题。"));
        }
        let rating = source_rating.map(|value| format!("{value:.2}"));
        let tags = (!source_tags.is_empty()).then(|| source_tags.join("\n"));
        let fields: Vec<_> = [
            ("title", Some(title)),
            ("title_zh", title_zh),
            ("title_en", title_en),
            ("title_alt", alt_title),
            ("developer", developer),
            ("publisher", publisher),
            ("release_date", release_date),
            ("description", description),
            ("description_zh", description_zh),
        ]
        .into_iter()
        .filter_map(|(field, value)| value.map(|value| (field.to_owned(), value.to_owned())))
        .collect();
        let mut fields = fields;
        if let Some(rating) = rating {
            fields.push(("source_rating".into(), rating));
        }
        if let Some(tags) = tags {
            fields.push(("source_tags".into(), tags));
        }
        let aliases: Vec<_> = [(Some(title), "und"), (title_en, "en"), (alt_title, "ja")]
            .into_iter()
            .filter_map(|(value, language)| {
                value.map(|value| (value.to_owned(), language.to_owned()))
            })
            .collect();
        self.apply_remote_fields(
            game_id, "vndb", remote_id, &fields, &aliases, fetched_at, true,
        )
    }

    pub fn apply_bangumi_cover(
        &mut self,
        game_id: &str,
        remote_id: &str,
        cover_url: &str,
        fetched_at: &str,
    ) -> Result<()> {
        if remote_id.trim().is_empty()
            || !remote_id.chars().all(|c| c.is_ascii_digit())
            || !cover_url.starts_with("https://")
        {
            return Err(backend::invalid("Bangumi 封面资料无效。"));
        }
        self.apply_remote_fields(
            game_id,
            "bangumi",
            remote_id,
            &[
                ("cover_url".into(), cover_url.into()),
                ("cover_path".into(), cover_url.into()),
            ],
            &[],
            fetched_at,
            false,
        )?;
        Ok(())
    }

    pub fn apply_cached_bangumi_cover(
        &mut self,
        game_id: &str,
        remote_id: &str,
        cover_url: &str,
        cached_path: &str,
        fetched_at: &str,
    ) -> Result<()> {
        self.apply_cached_cover(
            game_id,
            "bangumi",
            remote_id,
            cover_url,
            cached_path,
            fetched_at,
        )
    }

    pub fn apply_cached_cover(
        &mut self,
        game_id: &str,
        provider: &str,
        remote_id: &str,
        cover_url: &str,
        cached_path: &str,
        fetched_at: &str,
    ) -> Result<()> {
        let valid_remote_id = match provider {
            "bangumi" => remote_id.chars().all(|c| c.is_ascii_digit()),
            "vndb" => remote_id.starts_with('v') && remote_id.len() > 1,
            _ => false,
        };
        if remote_id.trim().is_empty()
            || !valid_remote_id
            || !cover_url.starts_with("https://")
            || !cached_path.starts_with("covers/")
        {
            return Err(backend::invalid("封面资料无效。"));
        }
        self.apply_remote_fields(
            game_id,
            provider,
            remote_id,
            &[
                ("cover_url".into(), cover_url.into()),
                ("cover_path".into(), cached_path.into()),
            ],
            &[],
            fetched_at,
            false,
        )?;
        Ok(())
    }

    pub fn metadata_cache_response(
        &self,
        provider: &str,
        query: &str,
    ) -> Result<Option<(String, String)>> {
        self.connection
            .query_row(
                "SELECT response_json,fetched_at FROM metadata_cache WHERE provider=?1 AND query=?2 AND status='success'",
                params![provider, query],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn latest_vndb_cache_for_remote_id(
        &self,
        remote_id: &str,
    ) -> Result<Option<(String, String)>> {
        let pattern = format!("%\"id\":\"{remote_id}\"%");
        self.connection
            .query_row(
                "SELECT response_json,fetched_at FROM metadata_cache WHERE provider='vndb' AND status='success' AND response_json LIKE ?1 ORDER BY fetched_at DESC LIMIT 1",
                [pattern],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn save_metadata_cache(
        &mut self,
        provider: &str,
        query: &str,
        response_json: Option<&str>,
        status: &str,
        error_message: Option<&str>,
        fetched_at: &str,
    ) -> Result<()> {
        if !matches!(status, "pending" | "success" | "error") {
            return Err(backend::invalid("资料缓存状态无效。"));
        }
        self.connection.execute(
            "INSERT INTO metadata_cache(id,provider,query,response_json,status,error_message,fetched_at) VALUES(?1,?2,?3,?4,?5,?6,?7) ON CONFLICT(provider,query) DO UPDATE SET response_json=excluded.response_json,status=excluded.status,error_message=excluded.error_message,fetched_at=excluded.fetched_at",
            params![backend::id(), provider, query, response_json, status, error_message, fetched_at],
        )?;
        Ok(())
    }
    pub fn import_installation(
        &mut self,
        directory: &str,
        title: &str,
        game_id: Option<&str>,
        source: InstallSource,
        candidates: &[ExecutableCandidate],
        fingerprint: &str,
    ) -> Result<String> {
        if title.trim().is_empty() || title.chars().count() > 200 {
            return Err(backend::invalid("作品名称需要 1～200 个字符。"));
        }
        let tx = self.connection.transaction()?;
        let existing: Option<(String, String)> = tx
            .query_row(
                "SELECT id,game_id FROM game_installations WHERE absolute_path=? COLLATE NOCASE",
                [directory],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        let (install_id, target_game) = if let Some((i, g)) = existing {
            if game_id.is_some_and(|id| id != g) {
                return Err(ServiceError(
                    ErrorCode::Conflict,
                    "目录已经属于另一作品；请先移除旧的库记录。",
                ));
            }
            // Incremental rescans only refresh evidence; manual title/launch config is retained.
            tx.execute("UPDATE game_installations SET fingerprint=?1,scanned_at=?2,scan_status='scanned' WHERE id=?3",params![fingerprint,backend::now(),i])?;
            (i, g)
        } else {
            let g = if let Some(g) = game_id {
                if !tx.query_row("SELECT EXISTS(SELECT 1 FROM games WHERE id=?)", [g], |r| {
                    r.get::<_, bool>(0)
                })? {
                    return Err(backend::missing());
                }
                g.to_string()
            } else {
                let g = backend::id();
                tx.execute(
                    "INSERT INTO games(id,title,status) VALUES(?1,?2,'pending_confirmation')",
                    params![g, title.trim()],
                )?;
                g
            };
            let i = backend::id();
            let folder = Path::new(directory)
                .file_name()
                .unwrap_or_default()
                .to_string_lossy();
            tx.execute("INSERT INTO game_installations(id,game_id,absolute_path,folder_name,source,fingerprint,scan_status,scanned_at,product_name,company_name) VALUES(?1,?2,?3,?4,?5,?6,'scanned',?7,?8,?9)",params![i,g,directory,folder,backend::enum_text(&source),fingerprint,backend::now(),candidates.first().and_then(|c|c.product_name.as_ref()),candidates.first().and_then(|c|c.company_name.as_ref())])?;
            (i, g)
        };
        tx.execute("INSERT INTO settings(id,key,value_json) VALUES(?1,?2,?3) ON CONFLICT(key) DO UPDATE SET value_json=excluded.value_json",params![backend::id(),format!("scan.candidates.{install_id}"),encode(&candidates)?])?;
        tx.commit()?;
        Ok(target_game)
    }
    pub fn scan_fingerprint(&self, directory: &str) -> Result<Option<String>> {
        Ok(self
            .connection
            .query_row(
                "SELECT fingerprint FROM game_installations WHERE absolute_path=? COLLATE NOCASE",
                [directory],
                |r| r.get(0),
            )
            .optional()?
            .flatten())
    }
    pub fn game_id_for_installation(&self, directory: &str) -> Result<Option<String>> {
        self.connection
            .query_row(
                "SELECT game_id FROM game_installations WHERE absolute_path=? COLLATE NOCASE",
                [directory],
                |row| row.get(0),
            )
            .optional()
            .map_err(Into::into)
    }
    pub fn installation_conflict(&self, directory: &str) -> Result<Option<String>> {
        let normalized = directory.trim_end_matches(['/', '\\']);
        self.connection
            .query_row(
                "SELECT game_id FROM game_installations WHERE absolute_path=?1 COLLATE NOCASE LIMIT 1",
                [normalized],
                |row| row.get(0),
            )
            .optional()
            .map_err(Into::into)
    }
    pub fn update_game(&self, r: &UpdateGameRequest) -> Result<GameDetail> {
        if r.title.trim().is_empty() {
            return Err(backend::invalid("作品名称需要 1～200 个字符。"));
        }
        if r.user_rating
            .is_some_and(|rating| !rating.is_finite() || !(0.0..=10.0).contains(&rating))
        {
            return Err(backend::invalid("评分需要在 0～10 之间。"));
        }
        let tx = self.connection.unchecked_transaction()?;
        let (display, stored): (String, String) = tx
            .query_row(
                &format!("SELECT {DISPLAY_TITLE_SQL},g.title FROM games g WHERE g.id=?"),
                [&r.game_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?
            .ok_or_else(backend::missing)?;
        let renamed = r.title.trim() != display.trim();
        if renamed {
            self.ensure_metadata_unlocked(&r.game_id)?;
        }
        if renamed && r.title.chars().count() > 200 {
            return Err(backend::invalid("作品名称需要 1～200 个字符。"));
        }
        let title = if renamed {
            r.title.trim()
        } else {
            stored.as_str()
        };
        tx.execute("UPDATE games SET title=?1,status=?2,is_favorite=?3,is_hidden=?4,user_rating=?5,updated_at=?6 WHERE id=?7",params![title,backend::enum_text(&r.status),r.favorite,r.hidden,r.user_rating,backend::now(),r.game_id])?;
        if renamed {
            tx.execute("INSERT INTO metadata_records(id,game_id,provider,field_name,value_json,is_user_edited,fetched_at) VALUES(?1,?2,'manual','title',?3,1,?4) ON CONFLICT(game_id,provider,field_name) DO UPDATE SET value_json=excluded.value_json,is_user_edited=1,fetched_at=excluded.fetched_at",params![backend::id(),r.game_id,encode(&title)?,backend::now()])?;
        }
        tx.commit()?;
        self.get_game(&r.game_id)
    }
    pub fn remove_game_record(&mut self, id: &str) -> Result<bool> {
        if self.connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM play_sessions WHERE game_id=? AND ended_at IS NULL)",
            [id],
            |r| r.get::<_, bool>(0),
        )? {
            return Err(ServiceError(
                ErrorCode::Conflict,
                "游戏正在运行，结束后再移除库记录。",
            ));
        }
        let tx = self.connection.transaction()?;
        tx.execute("DELETE FROM settings WHERE key IN (SELECT 'playtime.corrections.'||id FROM play_sessions WHERE game_id=?1) OR key IN (SELECT 'playtime.checkpoint.'||id FROM play_sessions WHERE game_id=?1)",[id])?;
        tx.execute("DELETE FROM settings WHERE key='hikarifield.game.'||?1 OR key='metadata.candidates.'||?1 OR key IN (SELECT 'scan.candidates.'||id FROM game_installations WHERE game_id=?2) OR key IN (SELECT 'launch.idle.'||id FROM game_installations WHERE game_id=?2)",params![id,id])?;
        tx.execute(
            "DELETE FROM settings WHERE key='metadata.locked.'||?1",
            [id],
        )?;
        let removed = tx.execute("DELETE FROM games WHERE id=?", [id])? != 0;
        tx.commit()?;
        Ok(removed)
    }
    pub fn installation(&self, id: &str) -> Result<InstallationDetails> {
        let r=self.connection.query_row("SELECT game_id,absolute_path,executable_path,source,steam_app_id,launch_arguments_json,working_directory,environment_json,main_process_name,track_after_launcher_exit FROM game_installations WHERE id=?",[id],|r|Ok((r.get::<_,String>(0)?,r.get(1)?,r.get(2)?,r.get::<_,String>(3)?,r.get::<_,Option<i64>>(4)?,r.get::<_,String>(5)?,r.get(6)?,r.get::<_,String>(7)?,r.get(8)?,r.get(9)?))).optional()?.ok_or_else(backend::missing)?;
        Ok(InstallationDetails {
            id: id.into(),
            game_id: r.0,
            absolute_path: r.1,
            executable_path: r.2,
            source: decode(&format!("\"{}\"", r.3))?,
            steam_app_id: r.4.map(|id: i64| id.to_string()),
            arguments: decode(&r.5)?,
            working_directory: r.6,
            environment: decode(&r.7)?,
            main_process_name: r.8,
            track_after_launcher_exit: r.9,
            idle_timeout_minutes: self.setting(&format!("launch.idle.{id}"))?.flatten(),
            use_locale_emulator: self
                .setting::<(Option<bool>, Option<bool>)>(&format!("launch.tools.{id}"))?
                .and_then(|v| v.0),
            use_magpie: self
                .setting::<(Option<bool>, Option<bool>)>(&format!("launch.tools.{id}"))?
                .and_then(|v| v.1),
            candidates: self
                .setting(&format!("scan.candidates.{id}"))?
                .unwrap_or_default(),
        })
    }
    pub fn save_installation(
        &mut self,
        r: &ConfigureInstallationRequest,
    ) -> Result<InstallationDetails> {
        let steam_app_id = r
            .steam_app_id
            .as_deref()
            .map(str::trim)
            .filter(|id| !id.is_empty())
            .map(|id| id.parse::<i64>())
            .transpose()
            .map_err(|_| backend::invalid("Steam AppID 必须是正整数。"))?;
        if steam_app_id.is_some_and(|id| id <= 0) {
            return Err(backend::invalid("Steam AppID 必须是正整数。"));
        }
        if r.idle_timeout_minutes
            .is_some_and(|minutes| !(1..=120).contains(&minutes))
        {
            return Err(backend::invalid("空闲暂停阈值必须在 1～120 分钟之间。"));
        }
        let tx = self.connection.transaction()?;
        if tx.execute("UPDATE game_installations SET executable_path=?1,steam_app_id=?2,launch_arguments_json=?3,working_directory=?4,environment_json=?5,updated_at=?6,main_process_name=?8,track_after_launcher_exit=?9 WHERE id=?7",params![r.executable_path,steam_app_id,encode(&r.arguments)?,r.working_directory,encode(&r.environment)?,backend::now(),r.install_id,r.main_process_name,r.track_after_launcher_exit])?==0{return Err(backend::missing());}
        tx.execute("UPDATE games SET status='not_started' WHERE id=(SELECT game_id FROM game_installations WHERE id=?) AND status='pending_confirmation'",[&r.install_id])?;
        tx.execute("INSERT INTO settings(id,key,value_json,updated_at) VALUES(?1,?2,?3,?4) ON CONFLICT(key) DO UPDATE SET value_json=excluded.value_json,updated_at=excluded.updated_at",params![backend::id(),format!("launch.idle.{}",r.install_id),encode(&r.idle_timeout_minutes)?,backend::now()])?;
        tx.execute("INSERT INTO settings(id,key,value_json,updated_at) VALUES(?1,?2,?3,?4) ON CONFLICT(key) DO UPDATE SET value_json=excluded.value_json,updated_at=excluded.updated_at",params![backend::id(),format!("launch.tools.{}",r.install_id),encode(&(r.use_locale_emulator,r.use_magpie))?,backend::now()])?;
        tx.commit()?;
        self.installation(&r.install_id)
    }
    pub fn get_collection(&self, id: &str) -> Result<CollectionDetail> {
        let r=self.connection.query_row("SELECT name,kind,icon,color,sort_order,is_hidden,smart_filter_id,cover_path FROM collections WHERE id=?",[id],|r|Ok((r.get(0)?,r.get::<_,String>(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get::<_,Option<String>>(6)?,r.get::<_,Option<String>>(7)?))).optional()?.ok_or_else(backend::missing)?;
        let query = if let Some(filter) = r.6 {
            let s: String = self.connection.query_row(
                "SELECT query_json FROM smart_filters WHERE id=?",
                [filter],
                |r| r.get(0),
            )?;
            let q: GameQuery = decode(&s)?;
            validate_query(&q)?;
            if q.collection_id.is_some() {
                return Err(backend::invalid("智能集合不能引用集合。"));
            }
            Some(q)
        } else {
            None
        };
        let mut stmt = self.connection.prepare(
            "SELECT game_id FROM collection_members WHERE collection_id=? ORDER BY position,id",
        )?;
        let member_ids = stmt
            .query_map([id], |r| r.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let count = if let Some(q) = &query {
            let mut values = vec![];
            let predicate = filters(q, &mut values);
            self.connection.query_row(
                &format!("SELECT count(*) FROM games g WHERE {predicate}"),
                params_from_iter(values.iter()),
                |r| unsigned(r, 0),
            )?
        } else {
            member_ids.len() as u64
        };
        Ok(CollectionDetail {
            summary: CollectionSummary {
                id: id.into(),
                name: r.0,
                kind: decode(&format!("\"{}\"", r.1))?,
                icon: r.2,
                color: r.3,
                position: r.4,
                hidden: r.5,
                cover_url: r.7,
                game_count: count,
            },
            query,
            member_ids,
        })
    }
    pub fn list_collections(&self) -> Result<Vec<CollectionSummary>> {
        let mut stmt = self
            .connection
            .prepare("SELECT id FROM collections ORDER BY sort_order,created_at,id")?;
        let ids = stmt
            .query_map([], |r| r.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        ids.iter()
            .map(|id| self.get_collection(id).map(|c| c.summary))
            .collect()
    }
    pub fn reorder_collections(&mut self, ids: &[String]) -> Result<bool> {
        let tx = self.connection.transaction()?;
        let count: i64 = tx.query_row("SELECT count(*) FROM collections", [], |r| r.get(0))?;
        if ids.len() as i64 != count
            || ids.len() > 10000
            || ids.iter().collect::<std::collections::HashSet<_>>().len() != ids.len()
        {
            return Err(backend::invalid("请提交每个集合一次的完整顺序。"));
        }
        for (position, id) in ids.iter().enumerate() {
            if tx.execute("UPDATE collections SET sort_order=?,updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=?", params![position as i64, id])? != 1 { return Err(backend::missing()); }
        }
        tx.commit()?;
        Ok(true)
    }
    pub fn save_collection(&mut self, r: &SaveCollectionRequest) -> Result<CollectionDetail> {
        let name = r.name.trim();
        if name.is_empty()
            || name.chars().count() > 60
            || name.chars().any(char::is_control)
            || ["收藏", "未分组"].contains(&name)
        {
            return Err(backend::invalid("分组名称无效或属于保留名称。"));
        }
        if r.icon
            .as_ref()
            .is_some_and(|s| !["games", "heart", "spark", "clock", "saves"].contains(&s.as_str()))
            || r.color.as_ref().is_some_and(|s| {
                s.len() != 7
                    || !s.starts_with('#')
                    || !s[1..].bytes().all(|b| b.is_ascii_hexdigit())
            })
        {
            return Err(backend::invalid("集合图标或颜色无效。"));
        }
        if let Some(cover) = &r.cover_url {
            if !self.connection.query_row(
                "SELECT EXISTS(SELECT 1 FROM games WHERE cover_path=?)",
                [cover],
                |r| r.get::<_, bool>(0),
            )? {
                return Err(backend::invalid("请选择已登记作品的封面。"));
            }
        }
        match (&r.kind, &r.query) {
            (CollectionKind::Normal, None) => {}
            (CollectionKind::Smart, Some(q)) => {
                validate_query(q)?;
                if q.collection_id.is_some() {
                    return Err(backend::invalid("智能集合不能引用集合。"));
                }
            }
            _ => return Err(backend::invalid("集合类型与筛选条件不一致。")),
        }
        if matches!(r.kind, CollectionKind::Smart) && r.member_ids.is_some() {
            return Err(backend::invalid("智能集合不能手动设置成员。"));
        }
        let id = r.id.clone().unwrap_or_else(backend::id);
        let tx = self.connection.transaction()?;
        if tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM collections WHERE name=? COLLATE NOCASE AND id!=?)",
            params![name, id],
            |r| r.get::<_, bool>(0),
        )? {
            return Err(ServiceError(ErrorCode::Conflict, "分组名称已存在。"));
        }
        let old: Option<(String, Option<String>)> = tx
            .query_row(
                "SELECT kind,smart_filter_id FROM collections WHERE id=?",
                [&id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        if r.id.is_some() && old.is_none() {
            return Err(backend::missing());
        }
        if old
            .as_ref()
            .is_some_and(|o| o.0 != backend::enum_text(&r.kind))
        {
            return Err(ServiceError(
                ErrorCode::Conflict,
                "请新建集合，不在原集合上切换类型。",
            ));
        }
        let filter = if let Some(q) = &r.query {
            let f = old.and_then(|o| o.1).unwrap_or_else(backend::id);
            tx.execute("INSERT INTO smart_filters(id,name,query_json) VALUES(?1,?2,?3) ON CONFLICT(id) DO UPDATE SET name=excluded.name,query_json=excluded.query_json",params![f,name,encode(q)?])?;
            Some(f)
        } else {
            None
        };
        tx.execute("INSERT INTO collections(id,name,kind,smart_filter_id,icon,color,sort_order,is_hidden,cover_path) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9) ON CONFLICT(id) DO UPDATE SET name=excluded.name,icon=excluded.icon,color=excluded.color,sort_order=excluded.sort_order,is_hidden=excluded.is_hidden,cover_path=excluded.cover_path,updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now')",params![id,name,backend::enum_text(&r.kind),filter,r.icon,r.color,r.position,r.hidden,r.cover_url])?;
        if let Some(members) = &r.member_ids {
            replace_members_transaction(&tx, &id, members)?;
        }
        tx.commit()?;
        self.get_collection(&id)
    }
    pub fn replace_members(
        &mut self,
        collection_id: &str,
        game_ids: &[String],
    ) -> Result<CollectionDetail> {
        let c = self.get_collection(collection_id)?;
        if !matches!(c.summary.kind, CollectionKind::Normal) {
            return Err(backend::invalid("智能集合不能手动设置成员。"));
        }
        let tx = self.connection.transaction()?;
        replace_members_transaction(&tx, collection_id, game_ids)?;
        tx.commit()?;
        self.get_collection(collection_id)
    }
    pub fn delete_collection_record(&mut self, id: &str) -> Result<bool> {
        let tx = self.connection.transaction()?;
        let filter: Option<String> = tx
            .query_row(
                "SELECT smart_filter_id FROM collections WHERE id=?",
                [id],
                |r| r.get(0),
            )
            .optional()?
            .flatten();
        let removed = tx.execute("DELETE FROM collections WHERE id=?", [id])? != 0;
        if let Some(filter) = filter {
            tx.execute("DELETE FROM smart_filters WHERE id=?", [filter])?;
        }
        tx.commit()?;
        Ok(removed)
    }
    pub fn home_summary(&self) -> Result<HomeSummary> {
        let(game_count,playing_count,completed_count)=self.connection.query_row("SELECT count(*),coalesce(sum(status='playing'),0),coalesce(sum(status='completed'),0) FROM games WHERE is_hidden=0",[],|r|Ok((unsigned(r,0)?,unsigned(r,1)?,unsigned(r,2)?)))?;
        let week_playtime_seconds=self.connection.query_row("SELECT coalesce(sum(duration_seconds),0) FROM play_sessions WHERE julianday(started_at)>=julianday('now','-7 days')",[],|r|unsigned(r,0))?;
        let mut q = default_query();
        q.sort = GameSort::LastPlayedAt;
        q.statuses = vec![GameStatus::Playing, GameStatus::Paused];
        let continue_game_ids = self
            .list_games(&q)?
            .items
            .into_iter()
            .filter(|g| {
                !g.hidden
                    && g.installations
                        .iter()
                        .any(|i| i.path_valid && i.executable_path.is_some())
            })
            .take(5)
            .map(|g| g.id)
            .collect();
        q.sort = GameSort::AddedAt;
        q.statuses.clear();
        let recent_game_ids = self
            .list_games(&q)?
            .items
            .into_iter()
            .filter(|g| !g.hidden)
            .take(8)
            .map(|g| g.id)
            .collect();
        let pending_match_count = self.connection.query_row(
            "SELECT count(*) FROM games g WHERE g.is_hidden=0 AND EXISTS(SELECT 1 FROM settings s WHERE s.key='metadata.candidates.'||g.id)",
            [],
            |row| unsigned(row, 0),
        )?;
        Ok(HomeSummary {
            dashboard: None,
            game_count,
            playing_count,
            completed_count,
            week_playtime_seconds,
            pending_match_count,
            save_issue_count: self.connection.query_row(
                "SELECT count(*) FROM save_profiles p JOIN games g ON g.id=p.game_id JOIN settings s ON s.key='saves.error.'||p.id WHERE g.is_hidden=0 AND s.value_json!='null'", [], |row| unsigned(row,0))?,
            continue_game_ids,
            recent_game_ids,
        })
    }
    pub fn recommendations(&self, request: &RecommendationQuery) -> Result<Vec<Recommendation>> {
        if request.limit == 0 || request.limit > 50 || request.excluded_game_ids.len() > 10000 {
            return Err(backend::invalid(
                "推荐数量需要在 1～50 之间，排除数量不能超过 10000。",
            ));
        }
        let mut excluded: std::collections::HashSet<String> =
            request.excluded_game_ids.iter().cloned().collect();
        let mut statement = self.connection.prepare("SELECT game_id FROM recommendation_preferences WHERE preference='not_interested' OR (preference='snoozed' AND julianday(expires_at)>julianday('now'))")?;
        let suppressed = statement
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        excluded.extend(suppressed);
        let mut query = default_query();
        let first = self.list_games(&query)?;
        let pages = first.total.div_ceil(u64::from(query.page_size));
        let mut games = first.items;
        for page in 2..=pages {
            query.page = page
                .try_into()
                .map_err(|_| backend::invalid("游戏库数量超出范围。"))?;
            games.extend(self.list_games(&query)?.items);
        }
        let seeds: Vec<_> = games
            .iter()
            .filter(|game| {
                !game.hidden
                    && game.status != GameStatus::Dropped
                    && (game.favorite || game.user_rating.is_some_and(|rating| rating >= 7.0))
            })
            .collect();
        let mut candidates: Vec<_> = games
            .iter()
            .filter(|game| {
                !game.hidden
                    && matches!(game.status, GameStatus::NotStarted | GameStatus::Paused)
                    && !excluded.contains(&game.id)
            })
            .map(|game| {
                let developer_match = game
                    .developer
                    .as_deref()
                    .filter(|value| !value.trim().is_empty())
                    .is_some_and(|developer| {
                        seeds.iter().any(|seed| {
                            seed.id != game.id
                                && seed
                                    .developer
                                    .as_deref()
                                    .is_some_and(|other| other.eq_ignore_ascii_case(developer))
                        })
                    });
                let matching_tags: Vec<_> = game
                    .tags
                    .iter()
                    .filter(|tag| {
                        seeds.iter().any(|seed| {
                            seed.id != game.id && seed.tags.iter().any(|other| other.id == tag.id)
                        })
                    })
                    .collect();
                let score = u64::from(game.favorite) * 100
                    + u64::from(developer_match) * 30
                    + matching_tags.len() as u64 * 10;
                let reason = if game.favorite {
                    "已收藏但尚未完成".to_owned()
                } else if developer_match {
                    "来自你收藏或高评分作品的开发商".to_owned()
                } else if let Some(tag) = matching_tags.first() {
                    format!("与你收藏或高评分作品共有「{}」标签", tag.name)
                } else if game.status == GameStatus::Paused {
                    "一段尚未结束的旅途".to_owned()
                } else {
                    "从你的未开始作品中推荐".to_owned()
                };
                (game, score, reason)
            })
            .collect();
        candidates.sort_by(|(a, score_a, _), (b, score_b, _)| {
            score_b
                .cmp(score_a)
                .then_with(|| b.added_at.cmp(&a.added_at))
                .then_with(|| a.id.cmp(&b.id))
        });
        let mut result = Vec::new();
        for (game, _, reason) in candidates.into_iter().take(request.limit as usize) {
            let installed = game
                .installations
                .iter()
                .any(|install| install.path_valid && install.executable_path.is_some());
            let official_url = self
                .list_external_sources(&game.id)?
                .into_iter()
                .next()
                .map(|source| source.url);
            let action = if installed {
                RecommendationAction::Launch
            } else if official_url.is_some() {
                RecommendationAction::OfficialPage
            } else {
                RecommendationAction::ViewDetails
            };
            result.push(Recommendation {
                game_id: game.id.clone(),
                reason,
                source: RecommendationSource::Local,
                is_installed: installed,
                action,
                official_url,
            });
        }
        Ok(result)
    }
    pub fn recommendation_preferences(&self) -> Result<Vec<RecommendationPreferenceEntry>> {
        let mut statement = self.connection.prepare(&format!("SELECT p.game_id,p.preference,p.expires_at,{DISPLAY_TITLE_SQL} AS display_title FROM recommendation_preferences p JOIN games g ON g.id=p.game_id ORDER BY display_title COLLATE NOCASE,p.game_id"))?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, String>(3)?,
            ))
        })?;
        rows.map(|row| {
            let (game_id, kind, expires_at, game_title) = row?;
            Ok(RecommendationPreferenceEntry {
                preference: PreferenceResult {
                    game_id,
                    preference: decode(&format!("\"{kind}\""))?,
                    expires_at,
                },
                game_title,
            })
        })
        .collect()
    }
    pub fn set_recommendation_preference(
        &mut self,
        request: &SetRecommendationPreferenceRequest,
    ) -> Result<PreferenceResult> {
        self.get_game(&request.game_id)?;
        let preference = match request.preference {
            RecommendationPreference::None => {
                if request.expires_at.is_some() {
                    return Err(backend::invalid("清除偏好不能设置过期时间。"));
                }
                None
            }
            RecommendationPreference::NotInterested => {
                if request.expires_at.is_some() {
                    return Err(backend::invalid("不感兴趣偏好不能设置过期时间。"));
                }
                Some(("not_interested", None))
            }
            RecommendationPreference::Snoozed => {
                let expires = request
                    .expires_at
                    .as_deref()
                    .filter(|value| !value.trim().is_empty())
                    .ok_or_else(|| backend::invalid("暂不推荐需要提供过期时间。"))?;
                let expiry = chrono::DateTime::parse_from_rfc3339(expires)
                    .map_err(|_| backend::invalid("暂不推荐的过期时间必须是有效 ISO8601 时间。"))?;
                if expiry <= chrono::Utc::now() {
                    return Err(backend::invalid("暂不推荐的过期时间必须晚于当前时间。"));
                }
                Some(("snoozed", Some(expires)))
            }
        };
        let tx = self.connection.transaction()?;
        tx.execute(
            "DELETE FROM recommendation_preferences WHERE game_id=?",
            [&request.game_id],
        )?;
        if let Some((kind, expires)) = preference {
            tx.execute("INSERT INTO recommendation_preferences(id,game_id,preference,expires_at) VALUES(?1,?2,?3,?4)", params![backend::id(), request.game_id, kind, expires])?;
        }
        tx.commit()?;
        Ok(PreferenceResult {
            game_id: request.game_id.clone(),
            preference: request.preference.clone(),
            expires_at: request.expires_at.clone(),
        })
    }
    pub fn begin_session(&self, s: &LaunchSession, process: &str) -> Result<()> {
        self.connection.execute("INSERT INTO play_sessions(id,game_id,install_id,started_at,process_name) VALUES(?1,?2,?3,?4,?5)",params![s.session_id,s.game_id,s.install_id,s.started_at,process])?;
        Ok(())
    }
    pub fn has_open_session(&self, game: &str) -> Result<bool> {
        Ok(self.connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM play_sessions WHERE game_id=? AND ended_at IS NULL)",
            [game],
            |r| r.get(0),
        )?)
    }
    pub fn end_session(&self, id: &str, ended_at: &str, duration: u64, reason: &str) -> Result<()> {
        let duration: i64 = duration
            .try_into()
            .map_err(|_| backend::invalid("会话时长超出范围。"))?;
        let tx = self.connection.unchecked_transaction()?;
        tx.execute("UPDATE play_sessions SET ended_at=?1,duration_seconds=?2,end_reason=?3 WHERE id=?4 AND ended_at IS NULL",params![ended_at,duration,reason,id])?;
        tx.execute(
            "DELETE FROM settings WHERE key=?",
            [format!("playtime.checkpoint.{id}")],
        )?;
        tx.commit()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod publisher_tests;
