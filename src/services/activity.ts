import type {
  ActivityDay,
  ActivityRange,
  ActivitySnapshot,
} from '../types/activity';
export const ACTIVITY_RANGES: { id: ActivityRange; label: string }[] = [
  { id: 'week', label: '近 7 天' },
  { id: 'days30', label: '30 天' },
  { id: 'month', label: '本月' },
  { id: 'year', label: '今年' },
  { id: 'all', label: '全部' },
];
export const rangeLabel = (range: ActivityRange) =>
  ACTIVITY_RANGES.find((r) => r.id === range)!.label;
export function compactDuration(seconds: number): string {
  if (!seconds) return '0h';
  if (seconds < 60) return '<1m';
  const h = Math.floor(seconds / 3600),
    m = Math.floor((seconds % 3600) / 60);
  return h ? `${h}h${m ? ` ${m}m` : ''}` : `${m}m`;
}
export function heatLevel(seconds: number): number {
  return seconds <= 0
    ? 0
    : seconds <= 1800
      ? 1
      : seconds <= 7200
        ? 2
        : seconds <= 14400
          ? 3
          : 4;
}
export const utcDay = (value: string | Date) =>
  new Date(value).toISOString().slice(0, 10);
export const addDays = (date: string, amount: number) =>
  utcDay(new Date(Date.parse(`${date}T00:00:00Z`) + amount * 86400000));
export function rangeBounds(
  range: ActivityRange,
  now: Date,
  first = utcDay(now),
) {
  const today = utcDay(now);
  return {
    start:
      range === 'week'
        ? addDays(today, -6)
        : range === 'days30'
          ? addDays(today, -29)
          : range === 'month'
            ? `${today.slice(0, 7)}-01`
            : range === 'year'
              ? `${today.slice(0, 4)}-01-01`
              : first,
    end: addDays(today, 1),
  };
}
export function fullDateLabel(date: string): string {
  return new Intl.DateTimeFormat('zh-CN', {
    timeZone: 'UTC',
    year: 'numeric',
    month: 'long',
    day: 'numeric',
    weekday: 'long',
  }).format(new Date(`${date}T00:00:00Z`));
}
export function dateLabel(date: string, asOf: string): string {
  const today = utcDay(asOf);
  return date === today
    ? '今天'
    : date === addDays(today, -1)
      ? '昨天'
      : new Intl.DateTimeFormat('zh-CN', {
          timeZone: 'UTC',
          year: 'numeric',
          month: 'long',
          day: 'numeric',
          weekday: 'short',
        }).format(new Date(`${date}T00:00:00Z`));
}
export function utcTime(value: string): string {
  return new Intl.DateTimeFormat('zh-CN', {
    timeZone: 'UTC',
    hour: '2-digit',
    minute: '2-digit',
    hourCycle: 'h23',
  }).format(new Date(value));
}
export function sessionEnd(start: string, end: string | null): string {
  if (!end) return '进行中';
  const first = utcDay(start),
    last = utcDay(end);
  return `${first === last ? '' : last === addDays(first, 1) ? '次日 ' : `${last} `}${utcTime(end)}`;
}
export interface CalendarCell extends ActivityDay {
  valid: boolean;
  future: boolean;
  level: number;
}
export function calendarCells(
  snapshot: ActivitySnapshot,
): (CalendarCell | null)[] {
  if (!snapshot.daily.length) return [];
  const leading =
    (new Date(`${snapshot.daily[0].date}T00:00:00Z`).getUTCDay() + 6) % 7;
  const cells: (CalendarCell | null)[] = Array.from(
    { length: leading },
    () => null,
  );
  for (const d of snapshot.daily)
    cells.push({
      ...d,
      valid: d.date >= snapshot.range_start && d.date < snapshot.range_end,
      future: d.date > utcDay(snapshot.as_of),
      level: heatLevel(d.duration_seconds),
    });
  while (cells.length % 7) cells.push(null);
  return cells;
}
/** Horizontal tangents keep each segment inside its real endpoint range. */
export function trendGeometry(values: number[], width = 800, height = 190) {
  const max = Math.max(1, ...values);
  const points = values.map((v, i) => ({
    x: values.length > 1 ? (i * width) / (values.length - 1) : width / 2,
    y: height - (Math.max(0, v) / max) * (height - 12),
  }));
  let line = points.length ? `M ${points[0].x},${points[0].y}` : '';
  for (let i = 1; i < points.length; i++) {
    const a = points[i - 1],
      b = points[i],
      middle = (a.x + b.x) / 2;
    line += ` C ${middle},${a.y} ${middle},${b.y} ${b.x},${b.y}`;
  }
  return {
    points,
    max,
    line,
    area: points.length
      ? `${line} L ${points.at(-1)!.x},${height} L ${points[0].x},${height} Z`
      : '',
  };
}
