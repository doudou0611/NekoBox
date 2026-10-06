use super::Database;
use crate::backend::{
    self,
    hikarifield::{App, LibraryEntry, Ownership},
    Result,
};
use rusqlite::{params, OptionalExtension};
impl Database {
    pub fn hf_ownership(&self, game_id: &str) -> Result<Ownership> {
        self.get_game(game_id)?;
        self.setting(&format!("hikarifield.game.{game_id}"))?
            .ok_or_else(backend::missing)
    }
    pub fn hf_entry(&self, game_id: &str) -> Result<Option<LibraryEntry>> {
        Ok(self
            .setting::<Ownership>(&format!("hikarifield.game.{game_id}"))?
            .map(|o| LibraryEntry {
                app_id: o.app.id,
                released: o.app.released == 1,
            }))
    }
    fn hf_game_id(&self, app_id: u64) -> Result<Option<String>> {
        Ok(self.connection.query_row("SELECT substr(key,18) FROM settings WHERE key LIKE 'hikarifield.game.%' AND json_extract(value_json,'$.app.id')=? LIMIT 1",[i64::try_from(app_id).map_err(|_|backend::invalid("游戏 ID 超出范围。"))?],|r|r.get(0)).optional()?)
    }
    pub fn import_hf_app(
        &mut self,
        account_id: u64,
        app: &App,
        cover: Option<&str>,
    ) -> Result<bool> {
        let mapped = self.hf_game_id(app.id)?;
        let existing = if mapped.is_none() {
            let mut stmt = self
                .connection
                .prepare("SELECT id FROM games WHERE trim(title)=? OR trim(title_zh)=?")?;
            let ids = stmt
                .query_map(params![app.name.trim(), app.name.trim()], |r| {
                    r.get::<_, String>(0)
                })?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            if ids.len() == 1 {
                ids.into_iter().next()
            } else {
                None
            }
        } else {
            mapped
        };
        let existing = existing.filter(|id| {
            self.setting::<Ownership>(&format!("hikarifield.game.{id}"))
                .ok()
                .flatten()
                .is_none_or(|o| o.app.id == app.id)
        });
        let is_new = existing.is_none();
        let game_id = existing.unwrap_or_else(backend::id);
        let mut ownership = self
            .setting::<Ownership>(&format!("hikarifield.game.{game_id}"))?
            .unwrap_or(Ownership {
                owners: vec![],
                app: app.clone(),
            });
        if !ownership.owners.contains(&account_id) {
            ownership.owners.push(account_id);
        }
        ownership.app = app.clone();
        let tx = self.connection.transaction()?;
        tx.execute("INSERT INTO games(id,title,cover_path,status) VALUES(?1,?2,?3,'not_started') ON CONFLICT(id) DO NOTHING",params![game_id,app.name,cover])?;
        if is_new {
            tx.execute("INSERT INTO settings(id,key,value_json) VALUES(?1,?2,'true') ON CONFLICT(key) DO NOTHING",params![backend::id(),format!("metadata.generated_title.{game_id}")])?;
        }
        tx.execute("INSERT INTO settings(id,key,value_json) VALUES(?1,?2,?3) ON CONFLICT(key) DO UPDATE SET value_json=excluded.value_json",params![backend::id(),format!("hikarifield.game.{game_id}"),serde_json::to_string(&ownership).map_err(|_|backend::invalid("无法保存已购游戏。"))?])?;
        let locked = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM settings WHERE key=? AND value_json='true')",
            [format!("metadata.locked.{game_id}")],
            |r| r.get::<_, bool>(0),
        )?;
        if !locked {
            tx.execute("INSERT INTO metadata_records(id,game_id,provider,remote_id,field_name,value_json,source_url,fetched_at) VALUES(?1,?2,'hikarifield',?3,'title',?4,'https://store.hikarifield.co.jp/',?5) ON CONFLICT(game_id,provider,field_name) DO UPDATE SET value_json=excluded.value_json,remote_id=excluded.remote_id,fetched_at=excluded.fetched_at WHERE metadata_records.is_user_edited=0", params![backend::id(),game_id,app.id.to_string(),serde_json::to_string(&app.name).map_err(|_|backend::invalid("游戏名称无效。"))?,backend::now()])?;
        }
        tx.commit()?;
        Ok(is_new)
    }
    pub fn hf_needs_cover(&self, app_id: u64) -> Result<bool> {
        let Some(game) = self.hf_game_id(app_id)? else {
            return Ok(false);
        };
        Ok(self
            .connection
            .query_row(
                "SELECT NOT EXISTS(SELECT 1 FROM metadata_records WHERE game_id=games.id AND provider='hikarifield' AND field_name='cover_path') AND NOT EXISTS(SELECT 1 FROM settings WHERE key='metadata.locked.'||games.id AND value_json='true') FROM games WHERE id=?",
                [game],
                |r| r.get(0),
            )
            .optional()?
            .unwrap_or(false))
    }
    pub fn set_hf_cover(&mut self, app_id: u64, cover: &str) -> Result<()> {
        if let Some(game) = self.hf_game_id(app_id)? {
            if self.metadata_locked(&game)? {
                return Ok(());
            }
            self.apply_remote_fields(
                &game,
                "hikarifield",
                &app_id.to_string(),
                &[("cover_path".into(), cover.into())],
                &[],
                &backend::now(),
                false,
            )?;
        }
        Ok(())
    }
    pub fn attach_hf_installation(&mut self, game_id: &str, path: &str, exe: &str) -> Result<()> {
        self.get_game(game_id)?;
        let existing: Option<(String, String)> = self
            .connection
            .query_row(
                "SELECT id,game_id FROM game_installations WHERE absolute_path=? COLLATE NOCASE",
                [path],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        if existing.as_ref().is_some_and(|(_, g)| g != game_id) {
            return Err(backend::invalid(
                "此安装目录已关联其他游戏，请先检查库记录。",
            ));
        }
        let install_id = existing.map(|(id, _)| id).unwrap_or_else(backend::id);
        let folder = std::path::Path::new(path)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy();
        self.connection.execute("INSERT INTO game_installations(id,game_id,absolute_path,folder_name,executable_path,source,scan_status) VALUES(?1,?2,?3,?4,?5,'local','scanned') ON CONFLICT(id) DO UPDATE SET executable_path=excluded.executable_path",params![install_id,game_id,path,folder,exe])?;
        Ok(())
    }
}
