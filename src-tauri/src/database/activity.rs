use super::{library::unsigned, metadata::DISPLAY_TITLE_SQL, playtime, Database};
use crate::backend::{self, activity::*, Result};
use chrono::{DateTime, Datelike, Duration, NaiveDate, Utc};
use rusqlite::params;
use std::collections::BTreeMap;

fn day(year: i32, month: u32, date: u32) -> Result<NaiveDate> {
    NaiveDate::from_ymd_opt(year, month, date).ok_or_else(|| backend::invalid("日历年份无效。"))
}
fn parse_day(value: &str) -> Result<NaiveDate> {
    let parsed = NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .map_err(|_| backend::invalid("日期应为 YYYY-MM-DD。"))?;
    if parsed.to_string() != value {
        return Err(backend::invalid("日期格式无效。"));
    }
    Ok(parsed)
}
fn trend(
    start: NaiveDate,
    end: NaiveDate,
    range: ActivityRange,
    values: &BTreeMap<String, ActivityDay>,
) -> Result<(String, Vec<ActivityTrend>)> {
    let granularity = match range {
        ActivityRange::Year => "week",
        ActivityRange::All if end.year() - start.year() > 20 => "year",
        ActivityRange::All if (end - start).num_days() > 90 => "month",
        _ => "day",
    };
    let mut points = Vec::new();
    let mut cursor = start;
    while cursor < end {
        let next = match granularity {
            "week" => {
                cursor + Duration::days(7 - i64::from(cursor.weekday().num_days_from_monday()))
            }
            "month" if cursor.month() == 12 => day(cursor.year() + 1, 1, 1)?,
            "month" => day(cursor.year(), cursor.month() + 1, 1)?,
            "year" => day(cursor.year() + 1, 1, 1)?,
            _ => cursor + Duration::days(1),
        }
        .min(end);
        let seconds = values
            .range(cursor.to_string()..next.to_string())
            .map(|(_, value)| value.duration_seconds)
            .sum();
        points.push(ActivityTrend {
            date: cursor.to_string(),
            end_date: (next - Duration::days(1)).to_string(),
            duration_seconds: seconds,
        });
        cursor = next;
    }
    Ok((granularity.into(), points))
}

impl Database {
    pub fn activity_snapshot(&self, q: &ActivityQuery) -> Result<ActivitySnapshot> {
        self.activity_snapshot_at(q, Utc::now())
    }
    pub(super) fn activity_snapshot_at(
        &self,
        q: &ActivityQuery,
        now: DateTime<Utc>,
    ) -> Result<ActivitySnapshot> {
        if q.page == 0 || !(1..=100).contains(&q.page_size) {
            return Err(backend::invalid("分页参数超出范围。"));
        }
        let today = now.date_naive();
        let until = today + Duration::days(1);
        let as_of = now.to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        // All SELECTs share a read transaction, including pagination and corrections.
        let tx = self.connection.unchecked_transaction()?;
        let valid = format!(
            "{} AND julianday(p.started_at)<=julianday(?1)",
            playtime::VALID_SESSION
        );
        let first: Option<String> = tx.query_row(
            &format!("SELECT min(date(p.started_at)) FROM play_sessions p WHERE {valid}"),
            [&as_of],
            |r| r.get(0),
        )?;
        let start = match q.range {
            ActivityRange::Week => today - Duration::days(6),
            ActivityRange::Days30 => today - Duration::days(29),
            ActivityRange::Month => day(today.year(), today.month(), 1)?,
            ActivityRange::Year => day(today.year(), 1, 1)?,
            ActivityRange::All => first
                .as_deref()
                .map(parse_day)
                .transpose()?
                .unwrap_or(today),
        };
        let filter = format!("{valid} AND date(p.started_at)>=?2 AND date(p.started_at)<?3");
        let from = start.to_string();
        let to = until.to_string();
        let mut daily_stmt = tx.prepare(&format!(
            "SELECT date(p.started_at),sum(p.duration_seconds),count(*) FROM play_sessions p WHERE {filter} GROUP BY date(p.started_at)"
        ))?;
        let values = daily_stmt
            .query_map(params![as_of, from, to], |r| {
                Ok(ActivityDay {
                    date: r.get(0)?,
                    duration_seconds: unsigned(r, 1)?,
                    session_count: unsigned(r, 2)?,
                })
            })?
            .map(|r| r.map(|v| (v.date.clone(), v)))
            .collect::<std::result::Result<BTreeMap<_, _>, _>>()?;
        let mut summary = ActivitySummary {
            total_seconds: values.values().map(|d| d.duration_seconds).sum(),
            active_days: values.values().filter(|d| d.duration_seconds > 0).count() as u64,
            session_count: values.values().map(|d| d.session_count).sum(),
            ..Default::default()
        };
        let (played, completed) = tx.query_row(
            &format!("SELECT count(*),coalesce(sum(status='completed'),0) FROM (SELECT g.id,g.status FROM play_sessions p JOIN games g ON g.id=p.game_id WHERE {filter} GROUP BY g.id HAVING sum(p.duration_seconds)>0)"),
            params![as_of, from, to],
            |r| Ok((unsigned(r, 0)?, unsigned(r, 1)?)),
        )?;
        summary.played_count = played;
        summary.completed_count = completed;
        summary.active_session_count = tx.query_row(
            &format!("SELECT count(*) FROM play_sessions p WHERE {filter} AND p.ended_at IS NULL"),
            params![as_of, from, to],
            |r| unsigned(r, 0),
        )?;
        let mut game_stmt = tx.prepare(&format!(
            "SELECT g.id,{DISPLAY_TITLE_SQL},g.cover_path,sum(p.duration_seconds) FROM play_sessions p JOIN games g ON g.id=p.game_id WHERE {filter} GROUP BY g.id HAVING sum(p.duration_seconds)>0 ORDER BY sum(p.duration_seconds) DESC,g.id LIMIT 10"
        ))?;
        let games = game_stmt
            .query_map(params![as_of, from, to], |r| {
                Ok(ActivityGame {
                    game_id: r.get(0)?,
                    title: r.get(1)?,
                    cover_url: r.get(2)?,
                    duration_seconds: unsigned(r, 3)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let mut hour_stmt = tx.prepare(&format!(
            "SELECT cast(strftime('%H',p.started_at) as integer),sum(p.duration_seconds) FROM play_sessions p WHERE {filter} GROUP BY strftime('%H',p.started_at)"
        ))?;
        let hour_values = hour_stmt
            .query_map(params![as_of, from, to], |r| {
                Ok((r.get::<_, u32>(0)?, unsigned(r, 1)?))
            })?
            .collect::<std::result::Result<BTreeMap<_, _>, _>>()?;
        let hours = (0..24)
            .map(|hour| ActivityHour {
                hour,
                duration_seconds: *hour_values.get(&hour).unwrap_or(&0),
            })
            .collect();
        let mut years_stmt = tx.prepare(&format!(
            "SELECT DISTINCT cast(strftime('%Y',p.started_at) as integer) FROM play_sessions p WHERE {valid} ORDER BY 1 DESC"
        ))?;
        let mut calendar_years = years_stmt
            .query_map([&as_of], |r| r.get::<_, i32>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        if !calendar_years.contains(&today.year()) {
            calendar_years.insert(0, today.year());
        }
        let calendar_year = q.calendar_year.unwrap_or(today.year());
        if q.range != ActivityRange::All && q.calendar_year.is_some()
            || !calendar_years.contains(&calendar_year)
        {
            return Err(backend::invalid("该统计范围没有所选日历年份。"));
        }
        let (calendar_start, calendar_end) =
            (day(calendar_year, 1, 1)?, day(calendar_year + 1, 1, 1)?);
        // The calendar always shows real annual activity, independently of the main range.
        let calendar_values = daily_stmt
            .query_map(
                params![as_of, calendar_start.to_string(), calendar_end.to_string()],
                |r| {
                    Ok(ActivityDay {
                        date: r.get(0)?,
                        duration_seconds: unsigned(r, 1)?,
                        session_count: unsigned(r, 2)?,
                    })
                },
            )?
            .map(|r| r.map(|v| (v.date.clone(), v)))
            .collect::<std::result::Result<BTreeMap<_, _>, _>>()?;
        let daily = (0..(calendar_end - calendar_start).num_days())
            .map(|offset| {
                let date = (calendar_start + Duration::days(offset)).to_string();
                calendar_values.get(&date).cloned().unwrap_or(ActivityDay {
                    date,
                    duration_seconds: 0,
                    session_count: 0,
                })
            })
            .collect();
        let selected = q.session_date.as_deref().map(parse_day).transpose()?;
        if selected
            .is_some_and(|d| d < start || d >= until || d < calendar_start || d >= calendar_end)
        {
            return Err(backend::invalid("所选日期不在当前统计或日历范围内。"));
        }
        let sessions_filter = format!("{filter} AND (?4 IS NULL OR date(p.started_at)=?4)");
        let total = tx.query_row(
            &format!("SELECT count(*) FROM play_sessions p WHERE {sessions_filter}"),
            params![as_of, from, to, q.session_date],
            |r| unsigned(r, 0),
        )?;
        let mut sessions_stmt = tx.prepare(&format!(
            "{} WHERE {sessions_filter} ORDER BY julianday(p.started_at) DESC,p.id DESC LIMIT ?5 OFFSET ?6",
            playtime::session_select()
        ))?;
        let sessions = sessions_stmt
            .query_map(
                params![
                    as_of,
                    from,
                    to,
                    q.session_date,
                    q.page_size,
                    i64::from(q.page - 1) * i64::from(q.page_size)
                ],
                playtime::row_session,
            )?
            .map(|r| self.annotate_session(r?))
            .collect::<Result<Vec<_>>>()?;
        let (trend_granularity, trend) = trend(start, until, q.range, &values)?;
        drop((daily_stmt, game_stmt, hour_stmt, years_stmt, sessions_stmt));
        tx.commit()?;
        Ok(ActivitySnapshot {
            as_of,
            range: q.range,
            range_start: from,
            range_end: to,
            summary,
            daily,
            trend_granularity,
            trend,
            games,
            hours,
            calendar_year,
            calendar_years,
            sessions: crate::domain::models::Paginated {
                page: q.page,
                page_size: q.page_size,
                total,
                items: sessions,
            },
            session_date: q.session_date.clone(),
        })
    }
}

#[cfg(test)]
mod tests;
