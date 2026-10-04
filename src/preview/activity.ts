/** Explicit, isolated in-memory journal. This module is dynamically loaded only outside Tauri. */
import { PREVIEW_GAMES } from './data';
import { addDays, rangeBounds, utcDay } from '../services/activity';
import type {
  ActivityDay,
  ActivityQuery,
  ActivitySnapshot,
} from '../types/activity';
import type { PlaySession } from '../types/playtime';
interface DemoSession extends PlaySession {
  cover_url: string;
  completed: boolean;
}
export function previewSessions(now: Date): DemoSession[] {
  const today = utcDay(now);
  const sessions: DemoSession[] = [];
  for (let offset = 0; offset < 430; offset++) {
    if (offset % 7 === 3 || offset % 7 === 5 || offset % 17 === 0) continue;
    const game =
      PREVIEW_GAMES[
        (offset * 3 + Math.floor(offset / 7)) % PREVIEW_GAMES.length
      ];
    const hour = 18 + (offset % 6);
    const start = `${addDays(today, -offset)}T${String(hour).padStart(2, '0')}:${offset % 2 ? '13' : '41'}:00Z`;
    const seconds = (35 + ((offset * 37 + 58) % 180)) * 60;
    const end = new Date(Date.parse(start) + seconds * 1000).toISOString();
    if (Date.parse(end) > now.getTime()) continue;
    sessions.push({
      id: `preview-session-${offset}`,
      game_id: game.game_id,
      game_title: game.title,
      install_id: 'preview-only',
      started_at: start,
      ended_at: end,
      duration_seconds: seconds,
      process_name: null,
      end_reason: 'process_exit',
      corrected: offset === 2,
      correction_reason: offset === 2 ? '隔离演示中的手动修正示例' : null,
      cover_url: game.cover_url,
      completed: game.status === 'completed',
    });
  }
  return sessions;
}
export function previewActivity(
  q: ActivityQuery,
  now = new Date(),
): ActivitySnapshot {
  const all = previewSessions(now);
  const { start, end } = rangeBounds(
    q.range,
    now,
    utcDay(all.at(-1)?.started_at ?? now),
  );
  const records = all.filter(
    (s) => utcDay(s.started_at) >= start && utcDay(s.started_at) < end,
  );
  const byDay = new Map<string, ActivityDay>();
  const byGame = new Map<string, ActivitySnapshot['games'][number]>();
  const hours = Array.from({ length: 24 }, (_, hour) => ({
    hour,
    duration_seconds: 0,
  }));
  for (const s of records) {
    const date = utcDay(s.started_at);
    const d = byDay.get(date) ?? {
      date,
      duration_seconds: 0,
      session_count: 0,
    };
    d.duration_seconds += s.duration_seconds;
    d.session_count++;
    byDay.set(date, d);
    const g = byGame.get(s.game_id) ?? {
      game_id: s.game_id,
      title: s.game_title,
      cover_url: s.cover_url,
      duration_seconds: 0,
    };
    g.duration_seconds += s.duration_seconds;
    byGame.set(g.game_id, g);
    hours[new Date(s.started_at).getUTCHours()].duration_seconds +=
      s.duration_seconds;
  }
  const year = q.calendar_year ?? now.getUTCFullYear();
  const calendarStart = `${year}-01-01`;
  const calendarEnd = `${year + 1}-01-01`;
  const calendarDays = new Map<string, ActivityDay>();
  for (const s of all) {
    const date = utcDay(s.started_at);
    if (date < calendarStart || date >= calendarEnd) continue;
    const d = calendarDays.get(date) ?? {
      date,
      duration_seconds: 0,
      session_count: 0,
    };
    d.duration_seconds += s.duration_seconds;
    d.session_count++;
    calendarDays.set(date, d);
  }
  const daily: ActivityDay[] = [];
  for (let d = calendarStart; d < calendarEnd; d = addDays(d, 1))
    daily.push(
      calendarDays.get(d) ?? { date: d, duration_seconds: 0, session_count: 0 },
    );
  const granularity =
    q.range === 'year' ? 'week' : q.range === 'all' ? 'month' : 'day';
  const trend: ActivitySnapshot['trend'] = [];
  for (let d = start; d < end;) {
    let next =
      granularity === 'day'
        ? addDays(d, 1)
        : granularity === 'week'
          ? addDays(d, 7 - ((new Date(`${d}T00:00:00Z`).getUTCDay() + 6) % 7))
          : utcDay(
              new Date(
                Date.UTC(Number(d.slice(0, 4)), Number(d.slice(5, 7)), 1),
              ),
            );
    if (next > end) next = end;
    trend.push({
      date: d,
      end_date: addDays(next, -1),
      duration_seconds: [...byDay.values()]
        .filter((v) => v.date >= d && v.date < next)
        .reduce((sum, v) => sum + v.duration_seconds, 0),
    });
    d = next;
  }
  const filtered = records.filter(
    (s) => !q.session_date || utcDay(s.started_at) === q.session_date,
  );
  return {
    as_of: now.toISOString(),
    range: q.range,
    range_start: start,
    range_end: end,
    summary: {
      total_seconds: records.reduce((sum, s) => sum + s.duration_seconds, 0),
      played_count: byGame.size,
      completed_count: new Set(
        records.filter((s) => s.completed).map((s) => s.game_id),
      ).size,
      active_days: [...byDay.values()].filter((d) => d.duration_seconds > 0)
        .length,
      session_count: records.length,
      active_session_count: 0,
    },
    daily,
    trend_granularity: granularity,
    trend,
    games: [...byGame.values()]
      .sort(
        (a, b) =>
          b.duration_seconds - a.duration_seconds ||
          a.game_id.localeCompare(b.game_id),
      )
      .slice(0, 10),
    hours,
    calendar_year: year,
    calendar_years: [
      ...new Set([
        now.getUTCFullYear(),
        ...all.map((s) => new Date(s.started_at).getUTCFullYear()),
      ]),
    ].sort((a, b) => b - a),
    sessions: {
      page: q.page,
      page_size: q.page_size,
      total: filtered.length,
      items: filtered.slice((q.page - 1) * q.page_size, q.page * q.page_size),
    },
    session_date: q.session_date,
  };
}
