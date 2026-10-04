use super::{library::unsigned, metadata::DISPLAY_TITLE_SQL, Database};
use crate::{
    backend::{self, playtime::*, Result, ServiceError},
    domain::{models::Paginated, protocol::ErrorCode},
};
use chrono::{Duration, Utc};
use rusqlite::{params, OptionalExtension};

pub(super) fn row_session(r: &rusqlite::Row<'_>) -> rusqlite::Result<PlaySession> {
    Ok(PlaySession {
        id: r.get(0)?,
        game_id: r.get(1)?,
        game_title: r.get(2)?,
        install_id: r.get(3)?,
        started_at: r.get(4)?,
        ended_at: r.get(5)?,
        duration_seconds: unsigned(r, 6)?,
        process_name: r.get(7)?,
        end_reason: r.get(8)?,
        corrected: false,
        correction_reason: None,
    })
}

pub(super) fn session_select() -> String {
    format!("SELECT p.id,p.game_id,{DISPLAY_TITLE_SQL},p.install_id,p.started_at,p.ended_at,p.duration_seconds,p.process_name,p.end_reason FROM play_sessions p JOIN games g ON g.id=p.game_id")
}
pub(super) const VALID_SESSION: &str = "(p.end_reason IS NULL OR p.end_reason!='launch_failed')";
impl Database {
    fn session(&self, id: &str) -> Result<PlaySession> {
        self.connection
            .query_row(
                &format!("{} WHERE p.id=?", session_select()),
                [id],
                row_session,
            )
            .optional()?
            .ok_or_else(backend::missing)
    }
    pub(super) fn annotate_session(&self, mut s: PlaySession) -> Result<PlaySession> {
        let history: Vec<Correction> = self
            .setting(&format!("playtime.corrections.{}", s.id))?
            .unwrap_or_default();
        s.corrected = !history.is_empty();
        s.correction_reason = history.last().map(|c| c.reason.clone());
        Ok(s)
    }
    pub fn list_play_sessions(&self, q: &SessionQuery) -> Result<Paginated<PlaySession>> {
        if q.page == 0 || !(1..=100).contains(&q.page_size) {
            return Err(backend::invalid("分页参数超出范围。"));
        }
        if let Some(game) = &q.game_id {
            self.get_game(game)?;
        }
        let filter = format!("{VALID_SESSION} AND (?1 IS NULL OR p.game_id=?1)");
        let total = self.connection.query_row(
            &format!("SELECT count(*) FROM play_sessions p WHERE {filter}"),
            [&q.game_id],
            |r| unsigned(r, 0),
        )?;
        let mut stmt = self.connection.prepare(&format!(
            "{} WHERE {filter} ORDER BY julianday(p.started_at) DESC,p.id DESC LIMIT ?2 OFFSET ?3",
            session_select()
        ))?;
        let rows = stmt.query_map(
            params![
                q.game_id,
                q.page_size,
                i64::from(q.page - 1) * i64::from(q.page_size)
            ],
            row_session,
        )?;
        let items = rows
            .map(|r| self.annotate_session(r?))
            .collect::<Result<Vec<_>>>()?;
        Ok(Paginated {
            page: q.page,
            page_size: q.page_size,
            total,
            items,
        })
    }
    pub fn correct_play_session(&mut self, q: &CorrectSessionRequest) -> Result<PlaySession> {
        validate_correction(q)?;
        let session = self.session(&q.session_id)?;
        if session.ended_at.is_none() || session.end_reason.as_deref() == Some("launch_failed") {
            return Err(ServiceError(
                ErrorCode::Conflict,
                "只能修正已结束的有效游玩会话。",
            ));
        }
        if session.duration_seconds != q.expected_duration_seconds {
            return Err(ServiceError(
                ErrorCode::Conflict,
                "时长已被修改，请刷新记录后重试。",
            ));
        }
        let key = format!("playtime.corrections.{}", q.session_id);
        let mut history: Vec<Correction> = self.setting(&key)?.unwrap_or_default();
        history.push(Correction {
            previous_seconds: session.duration_seconds,
            duration_seconds: q.duration_seconds,
            reason: q.reason.trim().into(),
            corrected_at: backend::now(),
        });
        if history.len() > 32 {
            history.remove(0);
        }
        let json = serde_json::to_string(&history)
            .map_err(|_| backend::invalid("修正记录无法序列化。"))?;
        let tx = self.connection.transaction()?;
        tx.execute(
            "UPDATE play_sessions SET duration_seconds=?1 WHERE id=?2",
            params![q.duration_seconds as i64, q.session_id],
        )?;
        tx.execute("INSERT INTO settings(id,key,value_json) VALUES(?1,?2,?3) ON CONFLICT(key) DO UPDATE SET value_json=excluded.value_json,updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now')",params![backend::id(),key,json])?;
        tx.commit()?;
        self.annotate_session(self.session(&q.session_id)?)
    }
    pub fn checkpoint_session(&mut self, id: &str, at: &str, duration: u64) -> Result<()> {
        let session = self.session(id)?;
        if session.ended_at.is_some() {
            return Err(ServiceError(ErrorCode::Conflict, "会话已结束。"));
        }
        let start = chrono::DateTime::parse_from_rfc3339(&session.started_at)
            .map_err(|_| backend::invalid("开始时间无效。"))?;
        let end = chrono::DateTime::parse_from_rfc3339(at)
            .map_err(|_| backend::invalid("检查点时间无效。"))?;
        if !at.ends_with('Z')
            || end < start
            || duration < session.duration_seconds
            || duration > MAX_DURATION
        {
            return Err(backend::invalid("检查点时间或时长无效。"));
        }
        let point = serde_json::to_string(&Checkpoint {
            occurred_at: at.into(),
            duration_seconds: duration,
        })
        .map_err(|_| backend::invalid("检查点无法序列化。"))?;
        let tx = self.connection.transaction()?;
        tx.execute(
            "UPDATE play_sessions SET duration_seconds=?1 WHERE id=?2 AND ended_at IS NULL",
            params![duration as i64, id],
        )?;
        tx.execute("INSERT INTO settings(id,key,value_json) VALUES(?1,?2,?3) ON CONFLICT(key) DO UPDATE SET value_json=excluded.value_json",params![backend::id(),format!("playtime.checkpoint.{id}"),point])?;
        tx.commit()?;
        Ok(())
    }
    pub fn recover_play_sessions(&mut self) -> Result<()> {
        let sessions = self
            .connection
            .prepare("SELECT id,started_at FROM play_sessions WHERE ended_at IS NULL")?
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let mut recovered = Vec::new();
        for (id, start) in sessions {
            let checkpoint: Option<Checkpoint> =
                self.setting(&format!("playtime.checkpoint.{id}"))?;
            let (at, duration) = checkpoint
                .map(|p| (p.occurred_at, p.duration_seconds))
                .unwrap_or((start, 0));
            let duration: i64 = duration
                .try_into()
                .map_err(|_| backend::invalid("检查点时长超出范围。"))?;
            recovered.push((id, at, duration));
        }
        let tx = self.connection.transaction()?;
        for (id, at, duration) in recovered {
            tx.execute("UPDATE play_sessions SET ended_at=?1,duration_seconds=?2,end_reason='app_interrupted' WHERE id=?3 AND ended_at IS NULL",params![at,duration,id])?;
        }
        tx.execute(
            "DELETE FROM settings WHERE key LIKE 'playtime.checkpoint.%'",
            [],
        )?;
        tx.commit()?;
        Ok(())
    }
    pub fn playtime_stats(&self, q: &StatsQuery) -> Result<PlaytimeStats> {
        if !(7..=90).contains(&q.days) {
            return Err(backend::invalid("统计天数必须为 7～90 天。"));
        }
        if let Some(game) = &q.game_id {
            self.get_game(game)?;
        }
        let today = Utc::now().date_naive();
        let start = today - Duration::days(i64::from(q.days) - 1);
        let until = today + Duration::days(1);
        let filter = format!("{VALID_SESSION} AND (?1 IS NULL OR p.game_id=?1)");
        let (total_seconds,session_count,active_session_count)=self.connection.query_row(&format!("SELECT coalesce(sum(p.duration_seconds),0),count(*),coalesce(sum(p.ended_at IS NULL),0) FROM play_sessions p WHERE {filter}"),[&q.game_id],|r|Ok((unsigned(r,0)?,unsigned(r,1)?,unsigned(r,2)?)))?;
        let period = format!("{filter} AND date(p.started_at)>=?2 AND date(p.started_at)<?3");
        let mut stmt=self.connection.prepare(&format!("SELECT date(p.started_at),sum(p.duration_seconds) FROM play_sessions p WHERE {period} GROUP BY date(p.started_at)"))?;
        let daily_map = stmt
            .query_map(
                params![q.game_id, start.to_string(), until.to_string()],
                |r| Ok((r.get::<_, String>(0)?, unsigned(r, 1)?)),
            )?
            .collect::<std::result::Result<std::collections::BTreeMap<_, _>, _>>()?;
        let daily: Vec<_> = (0..q.days)
            .map(|n| {
                let date = (start + Duration::days(i64::from(n))).to_string();
                let duration_seconds = *daily_map.get(&date).unwrap_or(&0);
                DailyPlaytime {
                    date,
                    duration_seconds,
                }
            })
            .collect();
        let period_seconds = daily.iter().map(|d| d.duration_seconds).sum();
        let mut stmt=self.connection.prepare(&format!("SELECT g.id,{DISPLAY_TITLE_SQL},sum(p.duration_seconds) FROM play_sessions p JOIN games g ON g.id=p.game_id WHERE {period} GROUP BY g.id HAVING sum(p.duration_seconds)>0 ORDER BY sum(p.duration_seconds) DESC,g.id LIMIT 10"))?;
        let games = stmt
            .query_map(
                params![q.game_id, start.to_string(), until.to_string()],
                |r| {
                    Ok(GamePlaytime {
                        game_id: r.get(0)?,
                        title: r.get(1)?,
                        duration_seconds: unsigned(r, 2)?,
                    })
                },
            )?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(PlaytimeStats {
            total_seconds,
            period_seconds,
            session_count,
            active_session_count,
            daily,
            games,
        })
    }
}

impl super::Database {
    pub(crate) fn live_playtime_extra(
        &self,
        active: &std::collections::HashMap<String, u64>,
    ) -> crate::backend::Result<std::collections::HashMap<String, u64>> {
        use rusqlite::OptionalExtension;
        let mut extra = std::collections::HashMap::new();
        for (session, seconds) in active {
            if let Some((game,saved))=self.connection.query_row("SELECT game_id,duration_seconds FROM play_sessions WHERE id=? AND ended_at IS NULL",[session],|r|Ok((r.get::<_,String>(0)?,super::library::unsigned(r,1)?))).optional()?{
                *extra.entry(game).or_insert(0)+=seconds.saturating_sub(saved);
            }
        }
        Ok(extra)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{models::LaunchSession, protocol::InstallSource};
    fn seed(db: &mut Database, title: &str, session_id: &str, start: &str) -> String {
        let game = db
            .import_installation(
                &format!("/fixture/{title}"),
                title,
                None,
                InstallSource::Manual,
                &[],
                "fixture",
            )
            .unwrap();
        let install_id = db
            .get_game(&game)
            .unwrap()
            .summary
            .installations
            .remove(0)
            .id;
        db.begin_session(
            &LaunchSession {
                session_id: session_id.into(),
                game_id: game.clone(),
                install_id,
                started_at: start.into(),
            },
            "game.exe",
        )
        .unwrap();
        game
    }
    #[test]
    fn live_overlay_is_read_only_and_excludes_missing_sessions() {
        let mut db = Database::in_memory().unwrap();
        let game = seed(&mut db, "Live", "live-session", "2026-10-03T00:00:00Z");
        let active = std::collections::HashMap::from([
            ("live-session".into(), 7),
            ("missing-session".into(), 100),
        ]);
        let extra = db.live_playtime_extra(&active).unwrap();
        assert_eq!(extra.get(&game), Some(&7));
        assert_eq!(
            db.get_game(&game).unwrap().summary.total_playtime_seconds,
            0
        );
    }
    fn correction(id: &str, old: u64, new: u64) -> CorrectSessionRequest {
        CorrectSessionRequest {
            session_id: id.into(),
            expected_duration_seconds: old,
            duration_seconds: new,
            reason: "补记".into(),
            confirmed: true,
        }
    }
    #[test]
    fn correction_updates_all_totals_preserves_timestamps_and_rejects_stale_edits() {
        let mut db = Database::in_memory().unwrap();
        let start = backend::now();
        let game = seed(&mut db, "A", "a", &start);
        db.end_session("a", &start, 120, "process_exit").unwrap();
        let edited = db
            .correct_play_session(&correction("a", 120, 3600))
            .unwrap();
        assert!(edited.corrected);
        assert_eq!(edited.correction_reason.as_deref(), Some("补记"));
        assert_eq!(edited.started_at, start);
        assert_eq!(edited.ended_at.as_deref(), Some(start.as_str()));
        assert_eq!(
            db.get_game(&game).unwrap().summary.total_playtime_seconds,
            3600
        );
        assert_eq!(db.home_summary().unwrap().week_playtime_seconds, 3600);
        let stats = db
            .playtime_stats(&StatsQuery {
                game_id: Some(game),
                days: 7,
            })
            .unwrap();
        assert_eq!(stats.period_seconds, 3600);
        assert_eq!(stats.daily.len(), 7);
        assert_eq!(stats.daily.last().unwrap().duration_seconds, 3600);
        assert_eq!(
            db.correct_play_session(&correction("a", 120, 1))
                .unwrap_err()
                .0,
            ErrorCode::Conflict
        );
        assert!(db
            .correct_play_session(&correction("a", 3600, MAX_DURATION + 1))
            .is_err());
        let mut q = correction("a", 3600, 0);
        q.reason = " ".into();
        assert!(db.correct_play_session(&q).is_err());
        q.reason = "确认".into();
        q.confirmed = false;
        assert!(db.correct_play_session(&q).is_err());
    }
    #[test]
    fn failed_audit_write_rolls_back_duration_and_checkpoint_write_is_atomic() {
        let mut db = Database::in_memory().unwrap();
        let start = backend::now();
        seed(&mut db, "A", "a", &start);
        db.connection.execute_batch("CREATE TRIGGER deny_settings BEFORE INSERT ON settings BEGIN SELECT RAISE(ABORT,'test'); END;").unwrap();
        assert!(db.checkpoint_session("a", &start, 15).is_err());
        assert_eq!(db.session("a").unwrap().duration_seconds, 0);
        db.end_session("a", &start, 60, "process_exit").unwrap();
        assert!(db.correct_play_session(&correction("a", 60, 120)).is_err());
        assert_eq!(db.session("a").unwrap().duration_seconds, 60);
        assert!(
            !db.annotate_session(db.session("a").unwrap())
                .unwrap()
                .corrected
        );
    }
    #[test]
    fn disk_restart_retains_checkpoint_without_counting_offline_gap() {
        let root = std::env::temp_dir().join(backend::id());
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("db.sqlite3");
        let mut db = Database::open(&path).unwrap();
        let start =
            (Utc::now() - Duration::hours(2)).to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        let checkpoint =
            (Utc::now() - Duration::hours(1)).to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        seed(&mut db, "A", "a", &start);
        db.checkpoint_session("a", &checkpoint, 3600).unwrap();
        assert!(db.checkpoint_session("a", &checkpoint, 3599).is_err());
        drop(db);
        let mut db = Database::open(&path).unwrap();
        db.recover_interrupted().unwrap();
        let s = db.session("a").unwrap();
        assert_eq!(s.duration_seconds, 3600);
        assert_eq!(s.ended_at.as_deref(), Some(checkpoint.as_str()));
        assert_eq!(s.end_reason.as_deref(), Some("app_interrupted"));
        assert!(db
            .setting::<Checkpoint>("playtime.checkpoint.a")
            .unwrap()
            .is_none());
        db.recover_interrupted().unwrap();
        assert_eq!(db.session("a").unwrap().duration_seconds, 3600);
        drop(db);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn filters_paginates_excludes_failed_launch_and_counts_running_sessions() {
        let mut db = Database::in_memory().unwrap();
        let start = backend::now();
        let a = seed(&mut db, "A", "a", &start);
        db.end_session("a", &start, 20, "process_exit").unwrap();
        let b = seed(&mut db, "B", "b", &start);
        db.checkpoint_session("b", &start, 15).unwrap();
        seed(&mut db, "Failed", "failed", &start);
        db.end_session("failed", &start, 0, "launch_failed")
            .unwrap();
        let list = db
            .list_play_sessions(&SessionQuery {
                game_id: None,
                page: 2,
                page_size: 1,
            })
            .unwrap();
        assert_eq!(list.total, 2);
        assert_eq!(list.items.len(), 1);
        assert_eq!(
            db.list_play_sessions(&SessionQuery {
                game_id: Some(a.clone()),
                page: 1,
                page_size: 20
            })
            .unwrap()
            .items[0]
                .id,
            "a"
        );
        assert_eq!(
            db.correct_play_session(&correction("b", 15, 20))
                .unwrap_err()
                .0,
            ErrorCode::Conflict
        );
        assert_eq!(
            db.correct_play_session(&correction("failed", 0, 20))
                .unwrap_err()
                .0,
            ErrorCode::Conflict
        );
        let stats = db
            .playtime_stats(&StatsQuery {
                game_id: None,
                days: 30,
            })
            .unwrap();
        assert_eq!(stats.total_seconds, 35);
        assert_eq!(stats.active_session_count, 1);
        assert_eq!(stats.session_count, 2);
        assert_eq!(stats.games[0].game_id, a);
        assert_eq!(stats.games[1].game_id, b);
        assert!(db
            .list_play_sessions(&SessionQuery {
                game_id: None,
                page: 0,
                page_size: 20
            })
            .is_err());
        assert!(db
            .playtime_stats(&StatsQuery {
                game_id: None,
                days: 100
            })
            .is_err());
        assert!(db
            .playtime_stats(&StatsQuery {
                game_id: Some("missing".into()),
                days: 7
            })
            .is_err());
    }
    #[test]
    fn correction_audit_is_bounded_and_removed_with_game() {
        let mut db = Database::in_memory().unwrap();
        let start = backend::now();
        let game = seed(&mut db, "A", "a", &start);
        db.end_session("a", &start, 0, "process_exit").unwrap();
        for n in 0..40 {
            db.correct_play_session(&correction("a", n, n + 1)).unwrap();
        }
        let history: Vec<Correction> = db.setting("playtime.corrections.a").unwrap().unwrap();
        assert_eq!(history.len(), 32);
        assert_eq!(history[0].previous_seconds, 8);
        db.remove_game_record(&game).unwrap();
        assert!(db
            .setting::<Vec<Correction>>("playtime.corrections.a")
            .unwrap()
            .is_none());
    }
    #[test]
    fn stats_uses_start_day_and_zero_fills_days_without_sessions() {
        let mut db = Database::in_memory().unwrap();
        let yesterday = Utc::now().date_naive() - Duration::days(1);
        let start = format!("{yesterday}T23:59:00Z");
        seed(&mut db, "A", "a", &start);
        db.end_session("a", &backend::now(), 120, "process_exit")
            .unwrap();
        let old =
            (Utc::now() - Duration::days(100)).to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        seed(&mut db, "B", "b", &old);
        db.end_session("b", &old, 1000, "process_exit").unwrap();
        let stats = db
            .playtime_stats(&StatsQuery {
                game_id: None,
                days: 7,
            })
            .unwrap();
        assert_eq!(stats.total_seconds, 1120);
        assert_eq!(stats.period_seconds, 120);
        assert_eq!(stats.daily[5].duration_seconds, 120);
        assert_eq!(stats.daily[6].duration_seconds, 0);
        assert_eq!(stats.daily[0].duration_seconds, 0);
    }
}
