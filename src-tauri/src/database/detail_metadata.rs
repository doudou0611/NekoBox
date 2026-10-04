use super::Database;
use crate::{
    backend::{self, detail_metadata::EditRequest, Result, ServiceError},
    domain::{models::GameDetail, protocol::ErrorCode},
};
use rusqlite::params;
use serde_json::json;
impl Database {
    pub fn record_selected_process(&self, session: &str, name: &str) -> Result<()> {
        if self.connection.execute(
            "UPDATE play_sessions SET process_name=?1 WHERE id=?2 AND ended_at IS NULL",
            params![name, session],
        )? != 1
        {
            return Err(backend::invalid("当前游玩会话已结束。"));
        }
        Ok(())
    }
    pub fn freeze_metadata_order(&self, game: &str) -> Result<()> {
        let order = super::metadata::source_order(&self.connection, game)?;
        self.put_setting(&format!("metadata.priority.{game}"), &order)
    }
    pub fn metadata_locked(&self, game: &str) -> Result<bool> {
        Ok(self
            .setting::<bool>(&format!("metadata.locked.{game}"))?
            .unwrap_or(false))
    }
    pub fn ensure_metadata_unlocked(&self, game: &str) -> Result<()> {
        if self.metadata_locked(game)? {
            return Err(ServiceError(
                ErrorCode::Conflict,
                "元数据已锁定，请先解锁资料。",
            ));
        }
        Ok(())
    }
    pub fn edit_metadata(&mut self, q: &EditRequest) -> Result<GameDetail> {
        self.ensure_metadata_unlocked(&q.game_id)?;
        let detail = self.get_game(&q.game_id)?;
        let before = [
            ("title", json!(detail.summary.title)),
            ("cover_path", json!(detail.summary.cover_url)),
            ("developer", json!(detail.summary.developer)),
            ("source_rating", json!(detail.summary.source_rating)),
            ("release_date", json!(detail.summary.release_date)),
            ("description", json!(detail.description)),
        ]
        .into_iter()
        .collect::<std::collections::BTreeMap<_, _>>();
        if q.changes.is_empty() || q.changes.len() > 6 {
            return Err(backend::invalid("请选择要修改的资料。"));
        }
        let mut edits = Vec::new();
        for (field, value) in &q.changes {
            let old = before
                .get(field.as_str())
                .ok_or_else(|| backend::invalid("未知的资料字段。"))?;
            if q.expected.get(field) != Some(old) {
                return Err(ServiceError(
                    ErrorCode::Conflict,
                    "资料已更新，请重新载入后再保存；当前编辑内容已保留。",
                ));
            }
            let text = if field == "source_rating" {
                if value.is_null() {
                    String::new()
                } else {
                    let number = value
                        .as_f64()
                        .filter(|n| n.is_finite() && (0.0..=10.0).contains(n))
                        .ok_or_else(|| backend::invalid("来源评分需要在 0～10 之间。"))?;
                    number.to_string()
                }
            } else {
                let text = if value.is_null() {
                    ""
                } else {
                    value
                        .as_str()
                        .ok_or_else(|| backend::invalid("资料内容格式无效。"))?
                }
                .trim();
                let limit = match field.as_str() {
                    "title" => 200,
                    "developer" => 1024,
                    "cover_path" => 4096,
                    "description" => 100_000,
                    _ => 10,
                };
                if text.chars().count() > limit
                    || text.contains('\0')
                    || field == "title" && text.is_empty()
                {
                    return Err(backend::invalid("资料超出长度限制或名称为空。"));
                }
                if field == "release_date"
                    && !text.is_empty()
                    && chrono::NaiveDate::parse_from_str(text, "%Y-%m-%d").is_err()
                {
                    return Err(backend::invalid("发行日期请使用 YYYY-MM-DD 格式。"));
                }
                text.to_owned()
            };
            edits.push((field, text));
        }
        let tx = self.connection.transaction()?;
        for (field, text) in edits {
            tx.execute("INSERT INTO metadata_records(id,game_id,provider,field_name,value_json,is_user_edited,fetched_at) VALUES(?1,?2,'manual',?3,?4,1,?5) ON CONFLICT(game_id,provider,field_name) DO UPDATE SET value_json=excluded.value_json,is_user_edited=1,fetched_at=excluded.fetched_at", params![backend::id(),q.game_id,field,serde_json::to_string(&text).map_err(|_| backend::invalid("资料无法保存。"))?,backend::now()])?;
            if field != "source_rating" {
                // Only validated, fixed field names enter this SQL fragment.
                tx.execute(
                    &format!("UPDATE games SET {field}=?1,updated_at=?2 WHERE id=?3"),
                    params![
                        if text.is_empty() { None } else { Some(&text) },
                        backend::now(),
                        q.game_id
                    ],
                )?;
            }
        }
        tx.commit()?;
        self.get_game(&q.game_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::protocol::InstallSource;
    fn fixture() -> (Database, String) {
        let mut db = Database::in_memory().unwrap();
        let game = db
            .import_installation(
                "/fixture/detail-workspace",
                "本地名称",
                None,
                InstallSource::Manual,
                &[],
                "fixture",
            )
            .unwrap();
        db.apply_remote_fields(
            &game,
            "bangumi",
            "12",
            &[
                ("description".into(), "旧简介".into()),
                ("developer".into(), "旧开发商".into()),
                ("source_rating".into(), "8.5".into()),
            ],
            &[],
            &backend::now(),
            true,
        )
        .unwrap();
        (db, game)
    }
    fn edit(
        db: &mut Database,
        game: &str,
        field: &str,
        value: serde_json::Value,
        expected: serde_json::Value,
    ) -> Result<GameDetail> {
        db.edit_metadata(&EditRequest {
            game_id: game.into(),
            changes: [(field.into(), value)].into_iter().collect(),
            expected: [(field.into(), expected)].into_iter().collect(),
        })
    }
    #[test]
    fn explicit_edits_and_clearing_survive_new_scrapes_without_freezing_other_fields() {
        let (mut db, game) = fixture();
        edit(
            &mut db,
            &game,
            "description",
            json!("我的简介"),
            json!("旧简介"),
        )
        .unwrap();
        edit(&mut db, &game, "developer", json!(null), json!("旧开发商")).unwrap();
        edit(&mut db, &game, "source_rating", json!(null), json!(8.5)).unwrap();
        db.apply_remote_fields(
            &game,
            "bangumi",
            "12",
            &[
                ("description".into(), "新简介".into()),
                ("developer".into(), "新开发商".into()),
                ("source_rating".into(), "9".into()),
                ("release_date".into(), "2026-10-04".into()),
            ],
            &[],
            &backend::now(),
            true,
        )
        .unwrap();
        let value = db.get_game(&game).unwrap();
        assert_eq!(value.description.as_deref(), Some("我的简介"));
        assert_eq!(value.summary.developer, None);
        assert_eq!(value.summary.source_rating, None);
        assert_eq!(value.summary.release_date.as_deref(), Some("2026-10-04"));
        assert!(!value
            .metadata
            .iter()
            .any(|f| f.provider == "manual" && f.field == "title"));
        assert!(value
            .metadata
            .iter()
            .any(|f| f.remote_id.as_deref() == Some("12")));
    }
    #[test]
    fn lock_blocks_manual_legacy_and_staged_writes_and_is_excluded_from_refresh() {
        let (mut db, game) = fixture();
        let mut stage = Database::in_memory().unwrap();
        stage.restore_verified_snapshot(&db).unwrap();
        let expected = serde_json::to_string(&db.get_game(&game).unwrap().metadata).unwrap();
        db.put_setting(&format!("metadata.locked.{game}"), &true)
            .unwrap();
        assert!(db.get_game(&game).unwrap().metadata_locked);
        assert!(db.refresh_game_ids().unwrap().is_empty());
        assert!(edit(
            &mut db,
            &game,
            "description",
            json!("覆盖"),
            json!("旧简介")
        )
        .is_err());
        assert!(db
            .apply_remote_fields(&game, "bangumi", "99", &[], &[], &backend::now(), true)
            .is_err());
        assert!(db
            .commit_metadata_refresh(&game, &expected, &stage, &["bangumi".into()])
            .is_err());
        let current = db.get_game(&game).unwrap();
        let mut req = crate::backend::types::UpdateGameRequest {
            game_id: game.clone(),
            title: "改名".into(),
            status: current.summary.status,
            favorite: true,
            hidden: false,
            user_rating: None,
        };
        assert!(db.update_game(&req).is_err());
        req.title = current.summary.title;
        assert!(db.update_game(&req).unwrap().summary.favorite);
        db.put_setting(&format!("metadata.locked.{game}"), &false)
            .unwrap();
        edit(
            &mut db,
            &game,
            "description",
            json!("解锁编辑"),
            json!("旧简介"),
        )
        .unwrap();
    }
    #[test]
    fn stale_expected_values_and_invalid_dates_are_rejected_without_changes() {
        let (mut db, game) = fixture();
        assert!(edit(
            &mut db,
            &game,
            "description",
            json!("覆盖"),
            json!("过期版本")
        )
        .is_err());
        assert!(edit(
            &mut db,
            &game,
            "release_date",
            json!("2026-02-31"),
            json!(null)
        )
        .is_err());
        assert!(edit(&mut db, &game, "source_rating", json!(11), json!(8.5)).is_err());
        assert_eq!(
            db.get_game(&game).unwrap().description.as_deref(),
            Some("旧简介")
        );
    }
}
