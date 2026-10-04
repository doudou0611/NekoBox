use super::*;
use crate::{
    backend::playtime::CorrectSessionRequest,
    domain::{models::LaunchSession, protocol::InstallSource},
};
fn now() -> DateTime<Utc> {
    "2026-10-03T23:59:59Z".parse().unwrap()
}
fn query(range: ActivityRange) -> ActivityQuery {
    ActivityQuery {
        range,
        calendar_year: None,
        session_date: None,
        page: 1,
        page_size: 20,
    }
}
fn session(
    db: &mut Database,
    title: &str,
    id: &str,
    at: &str,
    end: Option<&str>,
    seconds: u64,
    reason: &str,
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
            started_at: at.into(),
        },
        "fixture.exe",
    )
    .unwrap();
    if let Some(end) = end {
        db.end_session(id, end, seconds, reason).unwrap();
    } else {
        db.checkpoint_session(id, "2026-10-03T23:55:00Z", seconds)
            .unwrap();
    }
    game
}
fn fixture() -> Database {
    let mut db = Database::in_memory().unwrap();
    for (id, title, start, end, seconds, reason) in [
        (
            "S1",
            "A",
            "2026-10-03T22:13:00Z",
            "2026-10-03T23:46:00Z",
            5580,
            "process_exit",
        ),
        (
            "S2",
            "B",
            "2026-10-02T20:41:00Z",
            "2026-10-02T22:07:00Z",
            5160,
            "process_exit",
        ),
        (
            "S3",
            "C",
            "2026-10-01T23:30:00Z",
            "2026-10-02T00:30:00Z",
            3600,
            "process_exit",
        ),
        (
            "S4",
            "A",
            "2026-09-28T21:00:00Z",
            "2026-09-28T22:00:00Z",
            3600,
            "process_exit",
        ),
        (
            "S5",
            "C",
            "2026-09-01T18:00:00Z",
            "2026-09-01T20:00:00Z",
            7200,
            "process_exit",
        ),
        (
            "S6",
            "A",
            "2026-10-03T10:00:00Z",
            "2026-10-03T10:01:00Z",
            0,
            "process_exit",
        ),
        (
            "S7",
            "B",
            "2026-10-03T09:00:00Z",
            "2026-10-03T09:01:00Z",
            60,
            "launch_failed",
        ),
        (
            "S8",
            "D",
            "2025-12-31T10:00:00Z",
            "2025-12-31T22:00:00Z",
            43200,
            "process_exit",
        ),
    ] {
        session(&mut db, title, id, start, Some(end), seconds, reason);
    }
    db.connection
        .execute(
            "UPDATE games SET status='completed' WHERE title IN ('A','C','D')",
            [],
        )
        .unwrap();
    db.correct_play_session(&CorrectSessionRequest {
        session_id: "S4".into(),
        expected_duration_seconds: 3600,
        duration_seconds: 7200,
        reason: "补记".into(),
        confirmed: true,
    })
    .unwrap();
    db
}
#[test]
fn all_five_ranges_match_acceptance_data_and_same_duration_fact() {
    let db = fixture();
    for (range, expected) in [
        (ActivityRange::Week, (21540, 3, 2, 4)),
        (ActivityRange::Days30, (21540, 3, 2, 4)),
        (ActivityRange::Month, (14340, 3, 2, 3)),
        (ActivityRange::Year, (28740, 3, 2, 5)),
        (ActivityRange::All, (71940, 4, 3, 6)),
    ] {
        let snap = db.activity_snapshot_at(&query(range), now()).unwrap();
        let s = &snap.summary;
        assert_eq!(
            (
                s.total_seconds,
                s.played_count,
                s.completed_count,
                s.active_days
            ),
            expected
        );
        assert_eq!(
            snap.trend.iter().map(|d| d.duration_seconds).sum::<u64>(),
            s.total_seconds
        );
        assert_eq!(
            snap.hours.iter().map(|h| h.duration_seconds).sum::<u64>(),
            s.total_seconds
        );
        assert_eq!(
            snap.games.iter().map(|g| g.duration_seconds).sum::<u64>(),
            s.total_seconds
        );
        assert_eq!(snap.hours.len(), 24);
        assert!(snap.sessions.items.iter().all(|s| s.id != "S7"));
        assert!(db.connection.is_autocommit());
    }
    let snap = db
        .activity_snapshot_at(&query(ActivityRange::Week), now())
        .unwrap();
    assert_eq!(snap.daily.len(), 365);
    assert_eq!(snap.sessions.total, 5);
    assert_eq!(snap.hours[0].duration_seconds, 0);
    assert_eq!(snap.hours[21].duration_seconds, 7200);
    assert_eq!(snap.hours[23].duration_seconds, 3600);
    assert_eq!(snap.games[0].title, "A");
    assert_eq!(snap.games[0].duration_seconds, 12780);
    assert!(
        snap.sessions
            .items
            .iter()
            .find(|s| s.id == "S4")
            .unwrap()
            .corrected
    );
}
#[test]
fn calendar_year_and_date_filter_only_change_local_windows_and_pagination() {
    let db = fixture();
    let mut q = query(ActivityRange::All);
    q.calendar_year = Some(2025);
    q.session_date = Some("2025-12-31".into());
    let snap = db.activity_snapshot_at(&q, now()).unwrap();
    assert_eq!(snap.summary.total_seconds, 71940);
    assert_eq!(snap.daily.len(), 365);
    assert_eq!(snap.daily.last().unwrap().duration_seconds, 43200);
    assert_eq!(snap.sessions.total, 1);
    assert_eq!(snap.sessions.items[0].id, "S8");
    q = query(ActivityRange::Week);
    q.session_date = Some("2026-10-03".into());
    q.page_size = 1;
    q.page = 2;
    let snap = db.activity_snapshot_at(&q, now()).unwrap();
    assert_eq!(snap.sessions.total, 2);
    assert_eq!(snap.sessions.items[0].id, "S6");
    assert_eq!(snap.summary.total_seconds, 21540);
    assert_eq!(snap.summary.active_days, 4);
}
#[test]
fn correction_to_zero_changes_all_aggregates_without_hiding_session() {
    let mut db = fixture();
    db.correct_play_session(&CorrectSessionRequest {
        session_id: "S2".into(),
        expected_duration_seconds: 5160,
        duration_seconds: 0,
        reason: "误记".into(),
        confirmed: true,
    })
    .unwrap();
    let snap = db
        .activity_snapshot_at(&query(ActivityRange::Week), now())
        .unwrap();
    assert_eq!(snap.summary.total_seconds, 16380);
    assert_eq!(snap.summary.played_count, 2);
    assert_eq!(snap.summary.active_days, 3);
    assert_eq!(snap.summary.session_count, 5);
    assert_eq!(snap.hours[20].duration_seconds, 0);
    assert!(
        snap.sessions
            .items
            .iter()
            .find(|s| s.id == "S2")
            .unwrap()
            .corrected
    );
    assert_eq!(
        snap.daily
            .iter()
            .find(|d| d.date == "2026-10-02")
            .unwrap()
            .session_count,
        1
    );
}
#[test]
fn running_uses_saved_checkpoint_and_future_launches_are_excluded() {
    let mut db = Database::in_memory().unwrap();
    session(
        &mut db,
        "running",
        "r",
        "2026-10-03T23:50:00Z",
        None,
        300,
        "",
    );
    session(
        &mut db,
        "future",
        "f",
        "2026-10-04T00:00:00Z",
        Some("2026-10-04T01:00:00Z"),
        3600,
        "process_exit",
    );
    let snap = db
        .activity_snapshot_at(&query(ActivityRange::Week), now())
        .unwrap();
    assert_eq!(snap.summary.total_seconds, 300);
    assert_eq!(snap.summary.active_session_count, 1);
    assert_eq!(snap.sessions.total, 1);
    assert_eq!(snap.hours[23].duration_seconds, 300);
}
#[test]
fn empty_month_year_start_leap_day_and_long_history_have_zero_filled_bounded_views() {
    let mut db = Database::in_memory().unwrap();
    let snap = db
        .activity_snapshot_at(&query(ActivityRange::All), now())
        .unwrap();
    assert_eq!(snap.summary.total_seconds, 0);
    assert_eq!(snap.calendar_years, vec![2026]);
    assert_eq!(snap.sessions.total, 0);
    let leap_now = "2024-03-01T23:59:59Z".parse().unwrap();
    session(
        &mut db,
        "leap",
        "leap",
        "2024-02-29T23:30:00Z",
        Some("2024-03-01T00:30:00Z"),
        3600,
        "process_exit",
    );
    let week = db
        .activity_snapshot_at(&query(ActivityRange::Week), leap_now)
        .unwrap();
    assert_eq!(
        week.daily
            .iter()
            .find(|d| d.date == "2024-02-29")
            .unwrap()
            .duration_seconds,
        3600
    );
    assert_eq!(week.daily.last().unwrap().duration_seconds, 0);
    let month = db
        .activity_snapshot_at(&query(ActivityRange::Month), leap_now)
        .unwrap();
    assert_eq!(month.summary.total_seconds, 0);
    assert_eq!(month.daily.len(), 366);
    session(
        &mut db,
        "old",
        "old",
        "1999-01-01T00:00:00Z",
        Some("1999-01-01T01:00:00Z"),
        3600,
        "process_exit",
    );
    let all = db
        .activity_snapshot_at(&query(ActivityRange::All), now())
        .unwrap();
    assert_eq!(all.trend_granularity, "year");
    assert_eq!(all.trend.len(), 28);
    assert_eq!(all.daily.len(), 365);
    assert_eq!(all.summary.total_seconds, 7200);
    let jan = db
        .activity_snapshot_at(
            &query(ActivityRange::Year),
            "2026-01-01T23:59:59Z".parse().unwrap(),
        )
        .unwrap();
    assert_eq!(jan.trend.len(), 1);
    assert_eq!(jan.trend[0].end_date, "2026-01-01");
}
#[test]
fn annual_calendar_keeps_real_activity_outside_short_ranges_without_changing_totals() {
    let db = fixture();
    let annual = db
        .activity_snapshot_at(&query(ActivityRange::Year), now())
        .unwrap();
    for range in [
        ActivityRange::Week,
        ActivityRange::Days30,
        ActivityRange::Month,
    ] {
        let snap = db.activity_snapshot_at(&query(range), now()).unwrap();
        assert_eq!(snap.daily.len(), 365);
        assert_eq!(snap.daily.first().unwrap().date, "2026-01-01");
        assert_eq!(snap.daily.last().unwrap().date, "2026-12-31");
        for (actual, expected) in snap.daily.iter().zip(&annual.daily) {
            assert_eq!(actual.date, expected.date);
            assert_eq!(actual.duration_seconds, expected.duration_seconds);
            assert_eq!(actual.session_count, expected.session_count);
        }
        assert_eq!(
            snap.daily
                .iter()
                .find(|d| d.date == "2026-09-01")
                .unwrap()
                .duration_seconds,
            7200
        );
        let mut q = query(range);
        q.session_date = Some("2026-09-01".into());
        assert!(db.activity_snapshot_at(&q, now()).is_err());
    }
    let month = db
        .activity_snapshot_at(
            &query(ActivityRange::Month),
            "2026-01-01T23:59:59Z".parse().unwrap(),
        )
        .unwrap();
    assert_eq!(month.summary.total_seconds, 0);
    assert!(month.daily.iter().all(|d| d.duration_seconds == 0));
}
#[test]
fn invalid_requests_reject_and_release_transaction() {
    let db = fixture();
    for q in [
        ActivityQuery {
            page: 0,
            ..query(ActivityRange::Week)
        },
        ActivityQuery {
            page_size: 101,
            ..query(ActivityRange::Week)
        },
        ActivityQuery {
            calendar_year: Some(2025),
            ..query(ActivityRange::Week)
        },
        ActivityQuery {
            calendar_year: Some(2027),
            ..query(ActivityRange::All)
        },
        ActivityQuery {
            session_date: Some("2026-10-04".into()),
            ..query(ActivityRange::Week)
        },
        ActivityQuery {
            session_date: Some("2026-9-28".into()),
            ..query(ActivityRange::Week)
        },
        ActivityQuery {
            calendar_year: Some(2025),
            session_date: Some("2026-10-03".into()),
            ..query(ActivityRange::All)
        },
    ] {
        assert!(db.activity_snapshot_at(&q, now()).is_err());
        assert!(db.connection.is_autocommit());
    }
}
#[test]
fn more_than_one_page_is_aggregated_and_same_game_installations_are_deduplicated() {
    let mut db = Database::in_memory().unwrap();
    let game = session(
        &mut db,
        "A",
        "first",
        "2026-10-03T10:00:00Z",
        Some("2026-10-03T10:01:00Z"),
        60,
        "process_exit",
    );
    let id = db
        .import_installation(
            "/fixture/A-second",
            "A",
            Some(&game),
            InstallSource::Manual,
            &[],
            "fixture",
        )
        .unwrap();
    assert_eq!(id, game);
    let installs = db.get_game(&game).unwrap().summary.installations;
    let other = installs
        .iter()
        .find(|i| i.absolute_path == "/fixture/A-second")
        .unwrap();
    db.begin_session(
        &LaunchSession {
            session_id: "other-install".into(),
            game_id: game.clone(),
            install_id: other.id.clone(),
            started_at: "2026-10-03T11:00:00Z".into(),
        },
        "fixture.exe",
    )
    .unwrap();
    db.end_session(
        "other-install",
        "2026-10-03T12:00:00Z",
        3600,
        "process_exit",
    )
    .unwrap();
    for n in 0..105 {
        session(
            &mut db,
            "A",
            &format!("extra-{n}"),
            "2026-10-03T09:00:00Z",
            Some("2026-10-03T09:01:00Z"),
            60,
            "process_exit",
        );
    }
    let snap = db
        .activity_snapshot_at(&query(ActivityRange::Week), now())
        .unwrap();
    assert_eq!(snap.sessions.total, 107);
    assert_eq!(snap.sessions.items.len(), 20);
    assert_eq!(snap.summary.played_count, 1);
    assert_eq!(snap.summary.total_seconds, 9960);
    assert_eq!(snap.games.len(), 1);
}
