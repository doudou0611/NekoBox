export function durationText(seconds: number): string {
  const value = Math.max(0, Math.floor(seconds));
  const hours = Math.floor(value / 3600);
  const minutes = Math.floor((value % 3600) / 60);
  const rest = value % 60;
  return hours
    ? `${hours} 小时 ${minutes} 分 ${rest} 秒`
    : minutes
      ? `${minutes} 分 ${rest} 秒`
      : `${rest} 秒`;
}
export function correctionSeconds(
  hours: number,
  minutes: number,
  seconds: number,
): number | null {
  if (
    ![hours, minutes, seconds].every(Number.isSafeInteger) ||
    hours < 0 ||
    minutes < 0 ||
    minutes > 59 ||
    seconds < 0 ||
    seconds > 59
  )
    return null;
  const value = hours * 3600 + minutes * 60 + seconds;
  return value <= 315_360_000 ? value : null;
}
export function sessionReason(reason: string | null): string {
  const labels: Record<string, string> = {
    process_exit: '正常结束',
    tracked_process_exit: '游戏进程已结束',
    process_error: '进程异常退出',
    monitor_error: '监控中断',
    app_interrupted: '客户端中断 · 已保留检查点',
  };
  return reason === null ? '游玩中' : (labels[reason] ?? '已结束');
}
