import type { HomeWindow } from '../types/home';
import type { PreviewGame } from '../preview/data';
import { GAME_STATUSES } from '../types/domain';
export function localDate(date: Date): string {
  return `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, '0')}-${String(date.getDate()).padStart(2, '0')}`;
}
function midnight(date: Date, delta = 0): Date {
  return new Date(date.getFullYear(), date.getMonth(), date.getDate() + delta);
}
function dayRange(date: Date) {
  return {
    start_at: midnight(date).toISOString(),
    end_at: midnight(date, 1).toISOString(),
  };
}
export function homeWindow(now = new Date()): HomeWindow {
  const start = midnight(now, -((now.getDay() + 6) % 7));
  const memory = new Date(now.getFullYear() - 1, now.getMonth(), now.getDate());
  const week_days = Array.from({ length: 7 }, (_, n) => {
    const day = midnight(start, n);
    return { date: localDate(day), ...dayRange(day) };
  });
  return {
    time_zone: Intl.DateTimeFormat().resolvedOptions().timeZone || 'UTC',
    today: dayRange(now),
    week: { start_at: week_days[0]!.start_at, end_at: week_days[6]!.end_at },
    week_days,
    memory_day:
      memory.getMonth() === now.getMonth() && memory.getDate() === now.getDate()
        ? dayRange(memory)
        : null,
  };
}
export function greeting(hour: number): string {
  return hour >= 5 && hour < 11
    ? '早上好'
    : hour < 14 && hour >= 11
      ? '中午好'
      : hour >= 14 && hour < 18
        ? '下午好'
        : hour >= 18 && hour < 23
          ? '晚上好'
          : '夜深了';
}
export function compactTime(seconds: number): string {
  const total = Math.max(0, Math.floor(seconds));
  const h = Math.floor(total / 3600),
    m = Math.floor((total % 3600) / 60);
  return h
    ? `${h} 小时${m ? ` ${m} 分` : ''}`
    : m
      ? `${m} 分钟`
      : total
        ? `${total} 秒`
        : '0 分钟';
}
export function statusCounts(games: PreviewGame[]) {
  const result = Object.fromEntries(GAME_STATUSES.map((s) => [s, 0])) as Record<
    (typeof GAME_STATUSES)[number],
    number
  > & { total: number };
  result.total = games.length;
  for (const game of games) result[game.status]++;
  return result;
}
export function selectHomeHero(
  games: PreviewGame[],
  active: string[],
  playable: (game: PreviewGame) => boolean,
): PreviewGame | undefined {
  const running = active
    .map((id) => games.find((g) => g.game_id === id))
    .find(Boolean);
  if (running) return running;
  const stable = (a: PreviewGame, b: PreviewGame) =>
    b.last_played_order - a.last_played_order ||
    b.added_order - a.added_order ||
    a.game_id.localeCompare(b.game_id);
  return (
    [...games]
      .filter(
        (g) =>
          ['playing', 'paused'].includes(g.status) &&
          g.last_played_order > 0 &&
          playable(g),
      )
      .sort(stable)[0] ??
    [...games]
      .filter((g) => g.status === 'not_started' && playable(g))
      .sort(
        (a, b) =>
          b.added_order - a.added_order || a.game_id.localeCompare(b.game_id),
      )[0]
  );
}
export function randomCandidate(
  games: PreviewGame[],
  previous: Set<string>,
  random = Math.random,
): PreviewGame | undefined {
  if (!games.length) return;
  let unseen = games.filter((g) => !previous.has(g.game_id));
  if (!unseen.length) {
    const last = [...previous].at(-1);
    previous.clear();
    unseen = games.length > 1 ? games.filter((g) => g.game_id !== last) : games;
  }
  const result =
    unseen[Math.min(unseen.length - 1, Math.floor(random() * unseen.length))]!;
  previous.add(result.game_id);
  return result;
}
export interface HomeCalendarEvent {
  key: string;
  game_id: string;
  date: string;
  label: string;
  release_date: string;
}
export function calendarEvents(
  games: PreviewGame[],
  now = new Date(),
): HomeCalendarEvent[] {
  const today = midnight(now),
    end = midnight(today, 30);
  const events: HomeCalendarEvent[] = [];
  for (const game of games) {
    const raw = game.release_date;
    if (!raw || !/^\d{4}-\d{2}-\d{2}$/.test(raw)) continue;
    const [y, m, d] = raw.split('-').map(Number) as [number, number, number];
    const release = new Date(y, m - 1, d);
    if (localDate(release) !== raw) continue;
    let date = release,
      label = '预计发售';
    if (release < today) {
      date = new Date(today.getFullYear(), m - 1, d);
      if (date < today) date = new Date(today.getFullYear() + 1, m - 1, d);
      if (date.getMonth() !== m - 1 || date.getDate() !== d) continue;
      const years = date.getFullYear() - y;
      if (years < 1) continue;
      label = `发行 ${years} 周年`;
    }
    if (date >= today && date < end)
      events.push({
        key: `${game.game_id}:${localDate(date)}:${label}`,
        game_id: game.game_id,
        date: localDate(date),
        label,
        release_date: raw,
      });
  }
  return events.sort(
    (a, b) =>
      a.date.localeCompare(b.date) ||
      a.label.localeCompare(b.label) ||
      a.game_id.localeCompare(b.game_id),
  );
}
export function relativePlayed(value: number, now = new Date()): string {
  if (value < 1e11) return '演示记录';
  const date = new Date(value);
  if (!Number.isFinite(date.getTime())) return '时间待确认';
  const days = Math.round(
    (Date.UTC(now.getFullYear(), now.getMonth(), now.getDate()) -
      Date.UTC(date.getFullYear(), date.getMonth(), date.getDate())) /
      86400000,
  );
  return days === 0
    ? '今天'
    : days === 1
      ? '昨天'
      : days > 1 && days < 8
        ? `${days} 天前`
        : date.toLocaleDateString('zh-CN');
}
