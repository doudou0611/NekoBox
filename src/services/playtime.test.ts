import { describe, expect, it } from 'vitest';
import { correctionSeconds, durationText, sessionReason } from './playtime';
describe('游玩记录', () => {
  it('keeps correction input exact and rejects overflow and invalid fields', () => {
    expect(correctionSeconds(1, 2, 3)).toBe(3723);
    expect(correctionSeconds(0, 0, 0)).toBe(0);
    expect(correctionSeconds(87600, 0, 0)).toBe(315360000);
    for (const fields of [
      [-1, 0, 0],
      [0, 60, 0],
      [0, 0, 60],
      [1.5, 0, 0],
      [NaN, 0, 0],
      [87600, 0, 1],
    ])
      expect(correctionSeconds(fields[0]!, fields[1]!, fields[2]!)).toBeNull();
  });
  it('shows subminute sessions instead of losing them to rounding', () => {
    expect(durationText(0)).toBe('0 秒');
    expect(durationText(45)).toBe('45 秒');
    expect(durationText(65)).toBe('1 分 5 秒');
    expect(durationText(3723)).toBe('1 小时 2 分 3 秒');
  });
  it('distinguishes interrupted sessions from normal exits', () => {
    expect(sessionReason(null)).toBe('游玩中');
    expect(sessionReason('app_interrupted')).toContain('检查点');
    expect(sessionReason('process_error')).toBe('进程异常退出');
  });
});
