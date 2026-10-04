import { describe, expect, it } from 'vitest';
import {
  addDays,
  calendarCells,
  compactDuration,
  dateLabel,
  heatLevel,
  rangeBounds,
  sessionEnd,
  trendGeometry,
} from './activity';
import { previewActivity } from '../preview/activity';
const now = new Date('2026-10-03T23:59:59Z');
const request = {
  range: 'days30' as const,
  calendar_year: null,
  session_date: null,
  page: 1,
  page_size: 20,
};
describe('活动页面数据呈现', () => {
  it('uses inclusive UTC natural days at month, year and leap boundaries', () => {
    expect(rangeBounds('week', now)).toEqual({
      start: '2026-09-27',
      end: '2026-10-04',
    });
    expect(rangeBounds('days30', now).start).toBe('2026-09-04');
    expect(rangeBounds('month', now).start).toBe('2026-10-01');
    expect(rangeBounds('year', now).start).toBe('2026-01-01');
    expect(addDays('2024-03-01', -1)).toBe('2024-02-29');
    expect(dateLabel('2026-10-03', now.toISOString())).toBe('今天');
    expect(dateLabel('2026-10-02', now.toISOString())).toBe('昨天');
    expect(sessionEnd('2026-10-01T23:30:00Z', '2026-10-02T00:30:00Z')).toBe(
      '次日 00:30',
    );
  });
  it('keeps subminute play visible and fixed heat thresholds exact', () => {
    expect([0, 1, 1800, 1801, 7200, 7201, 14400, 14401].map(heatLevel)).toEqual(
      [0, 1, 1, 2, 2, 3, 3, 4],
    );
    expect([0, 1, 59, 60, 3600, 21540].map(compactDuration)).toEqual([
      '0h',
      '<1m',
      '<1m',
      '1m',
      '1h',
      '5h 59m',
    ]);
  });
  it('aligns Monday columns with padding and distinguishes future dates', () => {
    const snap = previewActivity({ ...request, range: 'year' }, now);
    const cells = calendarCells(snap);
    expect(cells.length % 7).toBe(0);
    expect(cells.slice(0, 3)).toEqual([null, null, null]);
    expect(cells[3]?.date).toBe('2026-01-01');
    expect(cells.find((c) => c?.date === '2026-10-04')?.valid).toBe(false);
    expect(cells.find((c) => c?.date === '2026-10-04')?.future).toBe(true);
  });
  it('shows the same full annual activity across ranges and keeps selection within the range', () => {
    const annual = previewActivity({ ...request, range: 'year' }, now);
    for (const range of ['week', 'days30', 'month', 'year', 'all'] as const) {
      const snap = previewActivity({ ...request, range }, now);
      expect(snap.daily).toEqual(annual.daily);
      expect(snap.daily).toHaveLength(365);
      expect(snap.daily[0].date).toBe('2026-01-01');
      expect(snap.daily.at(-1)?.date).toBe('2026-12-31');
    }
    const cells = calendarCells(previewActivity(request, now));
    const outside = cells.find((c) => c?.date === '2026-09-01');
    expect(outside?.duration_seconds).toBeGreaterThan(0);
    expect(outside?.level).toBeGreaterThan(0);
    expect(outside?.valid).toBe(false);
    expect(outside?.future).toBe(false);
    const leap = previewActivity(request, new Date('2024-03-01T23:59:59Z'));
    expect(leap.daily).toHaveLength(366);
    expect(
      leap.daily.find((d) => d.date === '2024-02-29')?.duration_seconds,
    ).toBeGreaterThan(0);
  });
  it('keeps preview isolated, full-history sums honest and date selection local', () => {
    const all = previewActivity({ ...request, range: 'all' }, now);
    expect(all.trend.reduce((sum, d) => sum + d.duration_seconds, 0)).toBe(
      all.summary.total_seconds,
    );
    expect(all.hours.reduce((sum, h) => sum + h.duration_seconds, 0)).toBe(
      all.summary.total_seconds,
    );
    const selected = previewActivity(
      { ...request, session_date: '2026-10-01' },
      now,
    );
    expect(selected.summary).toEqual(previewActivity(request, now).summary);
    expect(
      selected.sessions.items.every((s) =>
        s.started_at.startsWith('2026-10-01'),
      ),
    ).toBe(true);
    expect(
      selected.sessions.items.every((s) => s.id.startsWith('preview-')),
    ).toBe(true);
    const prior = previewActivity(
      { ...request, range: 'all', calendar_year: 2025 },
      now,
    );
    expect(prior.summary).toEqual(all.summary);
    expect(prior.daily.every((d) => d.date.startsWith('2025'))).toBe(true);
  });
  it('creates finite bounded paths for empty, zero, singleton and spiky data', () => {
    for (const values of [[], [0], [0, 0, 0], [1], [0, 100000, 0, 1]]) {
      const g = trendGeometry(values);
      expect(g.line).not.toMatch(/NaN|Infinity/);
      expect(g.points.every((p) => p.y >= 0 && p.y <= 190)).toBe(true);
      expect(g.points.length).toBe(values.length);
    }
    expect(trendGeometry([1]).points[0].x).toBe(400);
  });
});
