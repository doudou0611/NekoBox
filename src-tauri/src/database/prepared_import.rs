//! Backend-owned snapshots; never accepts frontend metadata fields or SQL.
use super::Database;
use crate::{
    backend::{self, types::ExecutableCandidate, Result, ServiceError},
    domain::{models::GameDetail, protocol::ErrorCode},
};
use rusqlite::params;

#[derive(Clone)]
pub(crate) struct PreparedField {
    pub provider: String,
    pub remote_id: String,
    pub field: String,
    pub value_json: String,
    pub source_url: Option<String>,
    pub fetched_at: String,
}
#[derive(Clone)]
pub(crate) struct MetadataSnapshot {
    pub priority: Vec<String>,
    pub title: String,
    pub title_zh: Option<String>,
    pub title_ja: Option<String>,
    pub title_en: Option<String>,
    pub developer: Option<String>,
    pub release_date: Option<String>,
    pub description: Option<String>,
    pub cover_path: String,
    pub fields: Vec<PreparedField>,
    pub aliases: Vec<(String, String, String)>,
}
impl Database {
    pub(crate) fn prepared_metadata(&self, game_id: &str) -> Result<MetadataSnapshot> {
        let game = self.get_game(game_id)?;
        let mut statement = self.connection.prepare("SELECT provider,remote_id,field_name,value_json,source_url,fetched_at FROM metadata_records WHERE game_id=? AND remote_id IS NOT NULL AND is_user_edited=0")?;
        let fields = statement
            .query_map([game_id], |r| {
                Ok(PreparedField {
                    provider: r.get(0)?,
                    remote_id: r.get(1)?,
                    field: r.get(2)?,
                    value_json: r.get(3)?,
                    source_url: r.get(4)?,
                    fetched_at: r.get(5)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let mut statement = self
            .connection
            .prepare("SELECT alias,language,provider FROM aliases WHERE game_id=?")?;
        let aliases = statement
            .query_map([game_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let cover_path = self
            .connection
            .query_row("SELECT cover_path FROM games WHERE id=?", [game_id], |r| {
                r.get::<_, Option<String>>(0)
            })?
            .ok_or_else(|| backend::invalid("封面尚未保存到本地，不计入刮削成功。"))?;
        let priority = super::metadata::source_order(&self.connection, game_id)?;
        Ok(MetadataSnapshot {
            priority,
            title: game.summary.title,
            title_zh: game.summary.title_zh,
            title_ja: game.summary.title_ja,
            title_en: game.summary.title_en,
            developer: game.summary.developer,
            release_date: game.summary.release_date,
            description: game.description,
            cover_path,
            fields,
            aliases,
        })
    }

    /// All confirmed rows commit together. A conflict or failed insert leaves no half-game.
    pub(crate) fn import_prepared(
        &mut self,
        directory: &str,
        title: &str,
        executable: Option<&str>,
        candidates: &[ExecutableCandidate],
        snapshot: Option<&MetadataSnapshot>,
    ) -> Result<GameDetail> {
        self.import_prepared_with(directory, title, executable, candidates, snapshot, None)
    }

    pub(crate) fn import_prepared_steam(
        &mut self,
        directory: &str,
        title: &str,
        app_id: &str,
        snapshot: Option<&MetadataSnapshot>,
    ) -> Result<GameDetail> {
        self.import_prepared_with(directory, title, None, &[], snapshot, Some(app_id))
    }

    fn import_prepared_with(
        &mut self,
        directory: &str,
        title: &str,
        executable: Option<&str>,
        candidates: &[ExecutableCandidate],
        snapshot: Option<&MetadataSnapshot>,
        steam_app_id: Option<&str>,
    ) -> Result<GameDetail> {
        let title = snapshot.map_or(title, |value| value.title.as_str()).trim();
        if title.is_empty() || title.chars().count() > 200 {
            return Err(backend::invalid("作品名称需要 1～200 个字符。"));
        }
        let tx = self.connection.transaction()?;
        if tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM game_installations WHERE absolute_path=? COLLATE NOCASE)",
            [directory],
            |r| r.get::<_, bool>(0),
        )? {
            return Err(ServiceError(
                ErrorCode::Conflict,
                "该路径已在游戏库中，请刷新识别列表。",
            ));
        }
        if let Some(app_id) = steam_app_id {
            if tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM game_installations WHERE steam_app_id=?)",
                [app_id],
                |r| r.get::<_, bool>(0),
            )? {
                return Err(ServiceError(
                    ErrorCode::Conflict,
                    "该 Steam 游戏已在库中，请刷新列表。",
                ));
            }
        }
        if let Some(snapshot) = snapshot {
            for field in snapshot
                .fields
                .iter()
                .filter(|field| field.field != "community_binding")
            {
                if tx.query_row("SELECT EXISTS(SELECT 1 FROM metadata_records WHERE provider=?1 AND remote_id=?2)", params![field.provider,field.remote_id], |r| r.get::<_, bool>(0))? {
                    return Err(ServiceError(ErrorCode::Conflict,"该作品资料已经绑定到库内游戏，请排除重复项。"));
                }
            }
        }
        let game_id = backend::id();
        let install_id = backend::id();
        let folder = std::path::Path::new(directory)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy();
        let status = if executable.is_some() || steam_app_id.is_some() {
            "not_started"
        } else {
            "pending_confirmation"
        };
        tx.execute("INSERT INTO games(id,title,title_zh,title_ja,title_en,developer,release_date,description,cover_path,status) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)", params![game_id,title,snapshot.and_then(|s|s.title_zh.as_deref()),snapshot.and_then(|s|s.title_ja.as_deref()),snapshot.and_then(|s|s.title_en.as_deref()),snapshot.and_then(|s|s.developer.as_deref()),snapshot.and_then(|s|s.release_date.as_deref()),snapshot.and_then(|s|s.description.as_deref()),snapshot.map(|s|s.cover_path.as_str()),status])?;
        let working_directory = executable
            .and_then(|path| std::path::Path::new(path).parent())
            .and_then(|path| path.to_str());
        let evidence =
            serde_json::to_string(candidates).map_err(|_| backend::invalid("入口候选无效。"))?;
        tx.execute("INSERT INTO game_installations(id,game_id,absolute_path,folder_name,source,executable_path,working_directory,scan_status,scanned_at,product_name,company_name) VALUES(?1,?2,?3,?4,'manual',?5,?6,'scanned',?7,?8,?9)", params![install_id,game_id,directory,folder,executable,working_directory,backend::now(),candidates.first().and_then(|c|c.product_name.as_ref()),candidates.first().and_then(|c|c.company_name.as_ref())])?;
        if let Some(app_id) = steam_app_id {
            tx.execute("UPDATE game_installations SET source='steam',steam_app_id=?1,working_directory=?2,track_after_launcher_exit=1 WHERE id=?3", params![app_id, directory, install_id])?;
        }
        tx.execute(
            "INSERT INTO settings(id,key,value_json) VALUES(?1,?2,?3)",
            params![
                backend::id(),
                format!("scan.candidates.{install_id}"),
                evidence
            ],
        )?;
        if steam_app_id.is_some() && snapshot.is_none() {
            tx.execute(
                "INSERT INTO settings(id,key,value_json) VALUES(?1,?2,'true')",
                params![backend::id(), format!("metadata.generated_title.{game_id}")],
            )?;
        }
        if let Some(snapshot) = snapshot {
            tx.execute(
                "INSERT INTO settings(id,key,value_json) VALUES(?1,?2,'true')",
                params![backend::id(), format!("metadata.generated_title.{game_id}")],
            )?;
            tx.execute(
                "INSERT INTO settings(id,key,value_json) VALUES(?1,?2,?3)",
                params![
                    backend::id(),
                    format!("metadata.priority.{game_id}"),
                    serde_json::to_string(&snapshot.priority)
                        .map_err(|_| backend::invalid("来源顺序无效。"))?
                ],
            )?;
            for field in &snapshot.fields {
                tx.execute("INSERT INTO metadata_records(id,game_id,provider,remote_id,field_name,value_json,source_url,fetched_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)", params![backend::id(),game_id,field.provider,field.remote_id,field.field,field.value_json,field.source_url,field.fetched_at])?;
            }
            for (alias, language, provider) in &snapshot.aliases {
                tx.execute("INSERT INTO aliases(id,game_id,alias,language,provider) VALUES(?1,?2,?3,?4,?5) ON CONFLICT(game_id,alias,language) DO NOTHING", params![backend::id(),game_id,alias,language,provider])?;
            }
        }
        tx.commit()?;
        self.get_game(&game_id)
    }
}
