import { open } from '@tauri-apps/plugin-dialog';
export async function chooseSaveDirectory(): Promise<string | null> {
  const path = await open({
    directory: true,
    multiple: false,
    title: '选择此安装版本的存档目录',
  });
  return typeof path === 'string' ? path : null;
}
export function snapshotReason(value: string): string {
  const reasons: Record<string, string> = {
    manual: '手动备份',
    before_launch: '启动前备份',
    after_exit: '退出后备份',
    safety_before_restore: '恢复前安全备份',
  };
  return reasons[value] ?? '历史备份';
}
export function formatBytes(value: number): string {
  return value >= 1024 * 1024
    ? `${(value / (1024 * 1024)).toFixed(1)} MiB`
    : `${(value / 1024).toFixed(1)} KiB`;
}
