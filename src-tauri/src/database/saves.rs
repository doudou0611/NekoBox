use super::{library::unsigned, Database};
use crate::{
    backend::{
        self,
        saves::types::{RecycledProfile, SaveProfile},
        Result,
    },
    domain::models::SaveSnapshot,
};
use rusqlite::{params, OptionalExtension};

fn profile(row: &rusqlite::Row<'_>) -> rusqlite::Result<SaveProfile> {
    Ok(SaveProfile {
        id: row.get(0)?,
        game_id: row.get(1)?,
        install_id: row.get(2)?,
        source_path: row.get(3)?,
        backup_before_launch: row.get(4)?,
        backup_after_exit: row.get(5)?,
        retention_count: row.get(6)?,
        source_available: false,
        last_error: None,
    })
}
fn snapshot(row: &rusqlite::Row<'_>) -> rusqlite::Result<SaveSnapshot> {
    Ok(SaveSnapshot {
        id: row.get(0)?,
        save_profile_id: row.get(1)?,
        game_id: row.get(2)?,
        install_id: row.get(3)?,
        created_at: row.get(4)?,
        size_bytes: unsigned(row, 5)?,
        sha256: row.get(6)?,
        label: row.get(7)?,
        note: row.get(8)?,
        file_count: unsigned(row, 9)?,
        creation_reason: row.get(10)?,
    })
}
const PROFILE_SQL: &str = "SELECT id,game_id,install_id,source_path,backup_before_launch,backup_after_exit,retention_count FROM save_profiles";
const SNAPSHOT_SQL: &str = "SELECT s.id,s.profile_id,p.game_id,p.install_id,s.created_at,s.size_bytes,s.sha256,s.label,s.note,s.file_count,s.creation_reason FROM save_snapshots s JOIN save_profiles p ON p.id=s.profile_id";
impl Database {
    pub fn save_profiles(&self, game_id: &str) -> Result<Vec<SaveProfile>> {
        self.get_game(game_id)?;
        let mut statement = self.connection.prepare(&format!(
            "{PROFILE_SQL} WHERE game_id=? ORDER BY created_at,id"
        ))?;
        let profiles = statement
            .query_map([game_id], profile)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        profiles
            .into_iter()
            .map(|mut p| {
                p.last_error = self.setting(&format!("saves.error.{}", p.id))?;
                Ok(p)
            })
            .collect()
    }
    pub fn save_profile(&self, id: &str) -> Result<SaveProfile> {
        self.connection
            .query_row(&format!("{PROFILE_SQL} WHERE id=?"), [id], profile)
            .optional()?
            .ok_or_else(backend::missing)
    }
    pub fn store_save_profile(&self, p: &SaveProfile) -> Result<()> {
        self.connection.execute("INSERT INTO save_profiles(id,game_id,install_id,source_path,target_path,backup_before_launch,backup_after_exit,retention_count) VALUES(?1,?2,?3,?4,?5,?6,?7,?8) ON CONFLICT(id) DO UPDATE SET source_path=excluded.source_path,backup_before_launch=excluded.backup_before_launch,backup_after_exit=excluded.backup_after_exit,retention_count=excluded.retention_count,updated_at=?9",
            params![p.id,p.game_id,p.install_id,p.source_path,format!("save-backups/{}",p.id),p.backup_before_launch,p.backup_after_exit,p.retention_count,backend::now()])?;
        Ok(())
    }
    pub fn save_snapshots(&self, game_id: &str) -> Result<Vec<SaveSnapshot>> {
        self.get_game(game_id)?;
        let mut statement = self.connection.prepare(&format!(
            "{SNAPSHOT_SQL} WHERE p.game_id=? ORDER BY s.created_at DESC,s.id DESC"
        ))?;
        let snapshots = statement
            .query_map([game_id], snapshot)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(snapshots)
    }
    pub fn save_snapshot(&self, id: &str) -> Result<SaveSnapshot> {
        self.connection
            .query_row(&format!("{SNAPSHOT_SQL} WHERE s.id=?"), [id], snapshot)
            .optional()?
            .ok_or_else(backend::missing)
    }
    pub fn store_save_snapshot(&self, s: &SaveSnapshot) -> Result<()> {
        self.connection.execute("INSERT INTO save_snapshots(id,profile_id,archive_path,sha256,size_bytes,file_count,label,note,creation_reason,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
            params![s.id,s.save_profile_id,format!("save-backups/{}/{}.zip",s.save_profile_id,s.id),s.sha256,s.size_bytes as i64,s.file_count as i64,s.label,s.note,s.creation_reason,s.created_at])?;
        Ok(())
    }
    pub fn forget_save_snapshot(&self, id: &str) -> Result<()> {
        self.connection
            .execute("DELETE FROM save_snapshots WHERE id=?", [id])?;
        Ok(())
    }
    pub fn forget_save_profile(&mut self, id: &str) -> Result<()> {
        let mut profile = self.save_profile(id)?;
        profile.last_error = self.setting(&format!("saves.error.{id}"))?;
        let snapshots = self
            .save_snapshots(&profile.game_id)?
            .into_iter()
            .filter(|s| s.save_profile_id == id)
            .collect();
        let recycled = RecycledProfile {
            profile,
            snapshots,
            deleted_at: backend::now(),
            backup_directory: format!("save-backups/{id}"),
        };
        let value = serde_json::to_string(&recycled)
            .map_err(|_| backend::invalid("配置回收信息无法保存，未删除存档配置。"))?;
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "INSERT INTO settings(id,key,value_json) VALUES(?1,?2,?3)",
            params![backend::id(), format!("saves.deleted-profile.{id}"), value],
        )?;
        transaction.execute("DELETE FROM save_profiles WHERE id=?", [id])?;
        transaction.execute(
            "DELETE FROM settings WHERE key=?",
            [format!("saves.error.{id}")],
        )?;
        transaction.commit()?;
        Ok(())
    }
}
