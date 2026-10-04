import type { TaskStatus } from '../types/domain';
import type { ScanReport } from '../types/local';

export function isScanActive(status: TaskStatus): boolean {
  return status === 'queued' || status === 'running' || status === 'paused';
}
export function scanNotification(
  report: Pick<
    ScanReport,
    'message' | 'imported' | 'unchanged' | 'issue_count' | 'truncated'
  >,
): string {
  return `${report.message} · 新增 ${report.imported}，未变更 ${report.unchanged}，问题 ${report.issue_count}${report.truncated ? '（扫描范围已截断）' : ''}`;
}
