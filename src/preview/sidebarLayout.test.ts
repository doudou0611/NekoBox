import { describe, expect, it } from 'vitest';
import { sidebarWidthBounds, sidebarPartition } from './sidebarLayout';

describe('侧栏拖动边界', () => {
  it('给最小 Windows 窗口的正文保留 480px，宽窗口不超过 460px', () => {
    expect(sidebarWidthBounds(800)).toEqual({ min: 240, max: 320 });
    expect(sidebarWidthBounds(1280)).toEqual({ min: 240, max: 460 });
  });
  it('上下分区保留最小可用空间，越界比例被限制', () => {
    expect(sidebarPartition(612, -1).top).toBe(136);
    expect(sidebarPartition(612, 2).top).toBe(456);
    expect(sidebarPartition(612, 0.38).top).toBe(228);
  });
  it('极短窗口仍得到有限且有序的分割尺寸', () => {
    for (const height of [0, 12, 100]) {
      const result = sidebarPartition(height, 0.38);
      expect(result.top).toBeGreaterThanOrEqual(result.min);
      expect(result.top).toBeLessThanOrEqual(result.max);
      expect(result.available - result.top).toBeGreaterThan(0);
    }
  });
});
