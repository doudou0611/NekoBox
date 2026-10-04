import { describe, it, expect } from 'vitest';
import {
  homeWindow,
  greeting,
  statusCounts,
  selectHomeHero,
  randomCandidate,
  calendarEvents,
} from './homeDashboard';
import { createPreviewGames } from '../preview/data';
describe('首页事实派生', () => {
  it('周一边界、日历年回忆与闰日不溢出', () => {
    const w = homeWindow(new Date(2026, 9, 3, 22));
    expect(w.week_days.map((d) => d.date)).toEqual([
      '2026-09-28',
      '2026-09-29',
      '2026-09-30',
      '2026-10-01',
      '2026-10-02',
      '2026-10-03',
      '2026-10-04',
    ]);
    expect(w.today.start_at).toBe(w.week_days[5]!.start_at);
    expect(homeWindow(new Date(2024, 1, 29)).memory_day).toBeNull();
    expect(homeWindow(new Date(2025, 2, 1)).memory_day?.start_at).toBe(
      new Date(2024, 2, 1).toISOString(),
    );
  });
  it('用户 playing 状态不是运行事实，运行与可启动回退区分', () => {
    const games = createPreviewGames();
    const playing = games.find((g) => g.status === 'playing')!;
    expect(selectHomeHero(games, [], () => false)).toBeUndefined();
    expect(selectHomeHero(games, [playing.game_id], () => false)?.game_id).toBe(
      playing.game_id,
    );
    expect(selectHomeHero(games, [], () => true)?.game_id).toBe(
      playing.game_id,
    );
    expect(Object.values(statusCounts(games)).reduce((a, b) => a + b, 0)).toBe(
      games.length * 2,
    );
  });
  it('随机遍历完整候选且跨轮避免马上重复', () => {
    const games = createPreviewGames().slice(0, 3),
      seen = new Set<string>();
    const ids = Array.from(
      { length: 3 },
      () => randomCandidate(games, seen, () => 0)!.game_id,
    );
    expect(new Set(ids).size).toBe(3);
    expect(randomCandidate(games, seen, () => 0)!.game_id).not.toBe(ids[2]);
    expect(randomCandidate([], seen)).toBeUndefined();
  });
  it('只采用完整真实日期，周年和发售区分，不挪动闰日', () => {
    const games = createPreviewGames().slice(0, 4);
    games[0]!.release_date = '2026-10-06';
    games[1]!.release_date = '2023-10-09';
    games[2]!.release_date = '2026-10';
    games[3]!.release_date = '2023-02-29';
    const events = calendarEvents(games, new Date(2026, 9, 3));
    expect(events.map((e) => e.label)).toEqual(['预计发售', '发行 3 周年']);
    games[0]!.release_date = '2020-02-29';
    expect(calendarEvents(games.slice(0, 1), new Date(2026, 1, 20))).toEqual(
      [],
    );
  });
  it('问候按本地时段切换', () => {
    expect([4, 5, 11, 14, 18, 23].map(greeting)).toEqual([
      '夜深了',
      '早上好',
      '中午好',
      '下午好',
      '晚上好',
      '夜深了',
    ]);
  });
});
