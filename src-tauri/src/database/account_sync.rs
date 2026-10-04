use super::Database;
use crate::{backend::Result, domain::models::GameDetail};
impl Database {
    pub fn account_sync_game_count(&self) -> Result<usize> {
        Ok(self
            .connection
            .query_row("SELECT count(*) FROM games", [], |row| row.get::<_, i64>(0))?
            as usize)
    }
    pub fn account_sync_games(&self, provider: &str) -> Result<Vec<(GameDetail, String)>> {
        let mut stmt=self.connection.prepare("SELECT game_id,min(remote_id) FROM metadata_records WHERE provider=?1 AND remote_id IS NOT NULL AND trim(remote_id)!='' GROUP BY game_id HAVING count(DISTINCT remote_id)=1 ORDER BY game_id")?;
        let ids = stmt
            .query_map([provider], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        ids.into_iter()
            .map(|(id, remote)| self.get_game(&id).map(|game| (game, remote)))
            .collect()
    }
}
