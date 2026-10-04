import { afterEach, expect, it, vi } from 'vitest';
import { isScanActive, scanNotification } from './scanFeedback';
import {
  preview,
  notify,
  dismissToast,
  clearPreviewTimers,
} from '../preview/store';
afterEach(() => {
  clearPreviewTimers();
  vi.useRealTimers();
});
it('shows only active scans, including paused scans, and hides restored terminal reports', () => {
  for (const status of ['queued', 'running', 'paused'] as const)
    expect(isScanActive(status)).toBe(true);
  for (const status of ['completed', 'cancelled', 'failed'] as const)
    expect(isScanActive(status)).toBe(false);
});
it('uses the shared dismissible and expiring notification for the scan summary', () => {
  vi.useFakeTimers();
  clearPreviewTimers();
  const message = scanNotification({
    message: '目录扫描已结束',
    imported: 10,
    unchanged: 2,
    issue_count: 1,
    truncated: true,
  });
  notify(message);
  expect(preview.toasts[0]?.message).toContain(
    '新增 10，未变更 2，问题 1（扫描范围已截断）',
  );
  vi.advanceTimersByTime(5200);
  expect(preview.toasts).toHaveLength(0);
  notify(message);
  dismissToast(preview.toasts[0]!.toast_id);
  expect(preview.toasts).toHaveLength(0);
});
