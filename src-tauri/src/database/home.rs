use super::{library::unsigned, playtime::VALID_SESSION, Database};
use crate::{
    backend::{self, Result},
    domain::{home::*, models::HomeSummary},
};
use chrono::{DateTime, Datelike, Duration, NaiveDate, Utc};
use rusqlite::params;
use std::collections::BTreeMap;

const VISIBLE: &str = "g.is_hidden=0";
fn utc(value: &str) -> Result<DateTime<Utc>> {
    if value.len() > 32 || !value.ends_with('Z') {
        return Err(backend::invalid("首页时间必须是 UTC ISO-8601。"));
    }
    DateTime::parse_from_rfc3339(value)
        .map(|d| d.with_timezone(&Utc))
        .map_err(|_| backend::invalid("首页日期范围无效。"))
}
fn range(r: &HomeRange, minimum: i64, maximum: i64) -> Result<()> {
    let seconds = (utc(&r.end_at)? - utc(&r.start_at)?).num_seconds();
    if seconds < minimum || seconds > maximum {
        return Err(backend::invalid("首页日期范围超出限制。"));
    }
    Ok(())
}
fn validate(w: &HomeWindow) -> Result<()> {
    if w.time_zone.is_empty()
        || w.time_zone.len() > 128
        || !w
            .time_zone
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"/_+-:".contains(&c))
        || w.week_days.len() != 7
    {
        return Err(backend::invalid("首页时区或七日范围无效。"));
    }
    range(&w.today, 22 * 3600, 26 * 3600)?;
    range(&w.week, 6 * 86400, 8 * 86400)?;
    let mut previous_end = utc(&w.week.start_at)?;
    let mut previous_date: Option<NaiveDate> = None;
    let mut today_date = None;
    for day in &w.week_days {
        let date = NaiveDate::parse_from_str(&day.date, "%Y-%m-%d")
            .map_err(|_| backend::invalid("首页本地日期无效。"))?;
        if day.date != date.to_string()
            || previous_date.is_none() && date.weekday().num_days_from_monday() != 0
            || previous_date.is_some_and(|old| old + Duration::days(1) != date)
        {
            return Err(backend::invalid("首页七日必须从周一连续排列。"));
        }
        range(
            &HomeRange {
                start_at: day.start_at.clone(),
                end_at: day.end_at.clone(),
            },
            22 * 3600,
            26 * 3600,
        )?;
        if utc(&day.start_at)? != previous_end {
            return Err(backend::invalid("首页七日范围必须首尾相接。"));
        }
        if utc(&day.start_at)? == utc(&w.today.start_at)?
            && utc(&day.end_at)? == utc(&w.today.end_at)?
        {
            today_date = Some(date);
        }
        previous_end = utc(&day.end_at)?;
        previous_date = Some(date);
    }
    if previous_end != utc(&w.week.end_at)? {
        return Err(backend::invalid("首页七日未覆盖整周。"));
    }
    let today = today_date.ok_or_else(|| backend::invalid("今日必须是本周的一天。"))?;
    if let Some(memory) = &w.memory_day {
        range(memory, 22 * 3600, 26 * 3600)?;
        let expected = today
            .with_year(today.year() - 1)
            .ok_or_else(|| backend::invalid("上一年不存在该日期。"))?;
        if (utc(&memory.start_at)?.date_naive() - expected)
            .num_days()
            .abs()
            > 1
        {
            return Err(backend::invalid("回忆范围必须是上一年对应日期。"));
        }
    }
    Ok(())
}
impl Database {
    pub fn home_summary_window(&self, q: &HomeQuery) -> Result<HomeSummary> {
        let Some(w) = &q.dashboard_window else {
            return self.home_summary();
        };
        validate(w)?;
        let transaction = self.connection.unchecked_transaction()?;
        let mut summary = self.home_summary()?;
        summary.dashboard = Some(self.home_dashboard(w)?);
        transaction.commit()?;
        Ok(summary)
    }
    fn home_seconds(&self, r: &HomeRange) -> Result<u64> {
        Ok(self.connection.query_row(&format!("SELECT coalesce(sum(p.duration_seconds),0) FROM play_sessions p JOIN games g ON g.id=p.game_id WHERE {VISIBLE} AND {VALID_SESSION} AND julianday(p.started_at)>=julianday(?1) AND julianday(p.started_at)<julianday(?2)"), params![r.start_at,r.end_at], |r| unsigned(r,0))?)
    }
    fn home_ranking(&self, r: &HomeRange, limit: i64) -> Result<Vec<HomeDuration>> {
        let mut stmt = self.connection.prepare(&format!("SELECT p.game_id,sum(p.duration_seconds) FROM play_sessions p JOIN games g ON g.id=p.game_id WHERE {VISIBLE} AND {VALID_SESSION} AND julianday(p.started_at)>=julianday(?1) AND julianday(p.started_at)<julianday(?2) GROUP BY p.game_id HAVING sum(p.duration_seconds)>0 ORDER BY sum(p.duration_seconds) DESC,p.game_id LIMIT ?3"))?;
        let result = stmt
            .query_map(params![r.start_at, r.end_at, limit], |r| {
                Ok(HomeDuration {
                    game_id: r.get(0)?,
                    duration_seconds: unsigned(r, 1)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(result)
    }
    fn home_dashboard(&self, w: &HomeWindow) -> Result<HomeDashboard> {
        let mut status_counts: BTreeMap<String, u64> = [
            "not_started",
            "playing",
            "paused",
            "completed",
            "dropped",
            "pending_confirmation",
            "total",
        ]
        .into_iter()
        .map(|s| (s.into(), 0))
        .collect();
        let mut stmt = self
            .connection
            .prepare("SELECT status,count(*) FROM games WHERE is_hidden=0 GROUP BY status")?;
        for row in stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, unsigned(r, 1)?)))? {
            let (status, count) = row?;
            *status_counts.get_mut("total").unwrap() += count;
            status_counts.insert(status, count);
        }
        let ids = |active: bool| -> Result<Vec<String>> {
            let mut stmt = self.connection.prepare(&format!("SELECT p.game_id FROM play_sessions p JOIN games g ON g.id=p.game_id WHERE {VISIBLE} AND {VALID_SESSION} {} GROUP BY p.game_id ORDER BY max(julianday(p.started_at)) DESC,p.game_id {}", if active {"AND p.ended_at IS NULL"} else {""}, if active {""} else {"LIMIT 6"}))?;
            let rows = stmt
                .query_map([], |r| r.get(0))?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(rows)
        };
        let week_daily = w
            .week_days
            .iter()
            .map(|day| {
                Ok(HomeDaily {
                    date: day.date.clone(),
                    duration_seconds: self.home_seconds(&HomeRange {
                        start_at: day.start_at.clone(),
                        end_at: day.end_at.clone(),
                    })?,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let week_seconds = week_daily.iter().map(|d| d.duration_seconds).sum();
        let week_games = self.home_ranking(&w.week, 3)?;
        let other_seconds =
            week_seconds - week_games.iter().map(|g| g.duration_seconds).sum::<u64>();
        let mut memory_games = Vec::new();
        if let Some(day) = &w.memory_day {
            for game in self.home_ranking(day, 2)? {
                let mut stmt = self.connection.prepare(&format!("SELECT p.id,p.started_at,p.ended_at,p.duration_seconds FROM play_sessions p JOIN games g ON g.id=p.game_id WHERE {VISIBLE} AND {VALID_SESSION} AND p.game_id=?1 AND julianday(p.started_at)>=julianday(?2) AND julianday(p.started_at)<julianday(?3) ORDER BY julianday(p.started_at),p.id LIMIT 10"))?;
                let sessions = stmt
                    .query_map(params![game.game_id, day.start_at, day.end_at], |r| {
                        Ok(HomeSession {
                            id: r.get(0)?,
                            started_at: r.get(1)?,
                            ended_at: r.get(2)?,
                            duration_seconds: unsigned(r, 3)?,
                        })
                    })?
                    .collect::<rusqlite::Result<Vec<_>>>()?;
                memory_games.push(HomeMemory {
                    game_id: game.game_id,
                    duration_seconds: game.duration_seconds,
                    sessions,
                });
            }
        }
        Ok(HomeDashboard {
            status_counts,
            active_game_ids: ids(true)?,
            recently_played_game_ids: ids(false)?,
            today_seconds: self.home_seconds(&w.today)?,
            week_seconds,
            week_daily,
            week_games,
            other_seconds,
            memory_games,
            queried_at: backend::now(),
            window: w.clone(),
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{models::LaunchSession, protocol::InstallSource};
    fn window() -> HomeWindow {
        serde_json::from_value(serde_json::json!({"time_zone":"Asia/Shanghai","today":{"start_at":"2026-10-02T16:00:00Z","end_at":"2026-10-03T16:00:00Z"},"week":{"start_at":"2026-09-27T16:00:00Z","end_at":"2026-10-04T16:00:00Z"},"week_days":(0..7).map(|n| {let start=utc("2026-09-27T16:00:00Z").unwrap()+Duration::days(n);serde_json::json!({"date":(NaiveDate::from_ymd_opt(2026,9,28).unwrap()+Duration::days(n)).to_string(),"start_at":start.to_rfc3339_opts(chrono::SecondsFormat::Secs,true),"end_at":(start+Duration::days(1)).to_rfc3339_opts(chrono::SecondsFormat::Secs,true)})}).collect::<Vec<_>>(),"memory_day":{"start_at":"2025-10-02T16:00:00Z","end_at":"2025-10-03T16:00:00Z"}})).unwrap()
    }
    fn seed(
        db: &mut Database,
        title: &str,
        id: &str,
        start: &str,
        seconds: u64,
        end: Option<&str>,
    ) -> String {
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
        let install_id = db.get_game(&game).unwrap().summary.installations[0]
            .id
            .clone();
        db.begin_session(
            &LaunchSession {
                session_id: id.into(),
                game_id: game.clone(),
                install_id,
                started_at: start.into(),
            },
            "game.exe",
        )
        .unwrap();
        if let Some(reason) = end {
            db.end_session(id, start, seconds, reason).unwrap();
        } else {
            db.checkpoint_session(id, start, seconds).unwrap();
        }
        game
    }
    #[test]
    fn dashboard_has_local_boundaries_valid_sessions_hidden_filter_and_history_without_writes() {
        let mut db = Database::in_memory().unwrap();
        let a = seed(
            &mut db,
            "A",
            "a",
            "2026-10-02T16:00:00Z",
            120,
            Some("process_exit"),
        );
        let b = seed(&mut db, "B", "b", "2026-10-03T15:59:59Z", 60, None);
        seed(
            &mut db,
            "End",
            "end",
            "2026-10-03T16:00:00Z",
            999,
            Some("process_exit"),
        );
        let hidden = seed(
            &mut db,
            "Hidden",
            "hidden",
            "2026-10-02T17:00:00Z",
            500,
            Some("process_exit"),
        );
        db.connection
            .execute("UPDATE games SET is_hidden=1 WHERE id=?", [hidden])
            .unwrap();
        seed(
            &mut db,
            "Failed",
            "failed",
            "2026-10-02T18:00:00Z",
            999,
            Some("launch_failed"),
        );
        let memory = seed(
            &mut db,
            "Memory",
            "memory",
            "2025-10-03T01:00:00Z",
            3600,
            Some("process_exit"),
        );
        let before = db.connection.total_changes();
        let summary = db
            .home_summary_window(&HomeQuery {
                dashboard_window: Some(window()),
            })
            .unwrap();
        let d = summary.dashboard.unwrap();
        assert_eq!(d.today_seconds, 180);
        assert_eq!(d.week_seconds, 1179);
        assert_eq!(d.week_daily[5].duration_seconds, 180);
        assert_eq!(d.week_daily[6].duration_seconds, 999);
        assert_eq!(d.active_game_ids, vec![b.clone()]);
        assert_eq!(d.memory_games[0].game_id, memory);
        assert_eq!(d.memory_games[0].duration_seconds, 3600);
        assert!(d.recently_played_game_ids.contains(&a));
        assert!(!d
            .recently_played_game_ids
            .iter()
            .any(|g| db.get_game(g).unwrap().summary.title == "Failed"));
        assert_eq!(d.status_counts["total"], 5);
        assert_eq!(db.connection.total_changes(), before);
        assert!(db
            .home_summary_window(&serde_json::from_str::<HomeQuery>("{}").unwrap())
            .unwrap()
            .dashboard
            .is_none());
        db.correct_play_session(&crate::backend::playtime::CorrectSessionRequest {
            session_id: "a".into(),
            expected_duration_seconds: 120,
            duration_seconds: 600,
            reason: "补记".into(),
            confirmed: true,
        })
        .unwrap();
        assert_eq!(
            db.home_summary_window(&HomeQuery {
                dashboard_window: Some(window())
            })
            .unwrap()
            .dashboard
            .unwrap()
            .today_seconds,
            660
        );
    }
    #[test]
    fn rejects_gaps_overlaps_wrong_dates_unbounded_windows_and_wrong_memory() {
        let w = window();
        assert!(validate(&w).is_ok());
        let mut bad = w.clone();
        bad.week_days[2].start_at = bad.week_days[1].start_at.clone();
        assert!(validate(&bad).is_err());
        let mut bad = w.clone();
        bad.today.end_at = bad.week.end_at.clone();
        assert!(validate(&bad).is_err());
        let mut bad = w.clone();
        bad.week_days[0].date = "2026-09-29".into();
        assert!(validate(&bad).is_err());
        let mut bad = w.clone();
        bad.memory_day.as_mut().unwrap().start_at = "2024-10-02T16:00:00Z".into();
        assert!(validate(&bad).is_err());
        let mut bad = w;
        bad.week_days.truncate(6);
        assert!(validate(&bad).is_err());
    }
    #[test]
    fn permits_variable_day_length_at_dst_boundary() {
        let mut w = window();
        w.time_zone = "America/New_York".into();
        // Shift one boundary and all later boundaries by an hour: one 23h day.
        for day in &mut w.week_days[3..] {
            day.start_at = (utc(&day.start_at).unwrap() - Duration::hours(1))
                .to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
            day.end_at = (utc(&day.end_at).unwrap() - Duration::hours(1))
                .to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        }
        w.week_days[2].end_at = w.week_days[3].start_at.clone();
        w.week.end_at = w.week_days[6].end_at.clone();
        w.today.start_at = w.week_days[5].start_at.clone();
        w.today.end_at = w.week_days[5].end_at.clone();
        assert!(validate(&w).is_ok());
    }
}
