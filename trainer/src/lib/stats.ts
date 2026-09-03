/**
 * 统计计算工具(纯函数):准确率、KPM、本地日期键、时长格式化。
 */

/** 本地日历日键,格式 YYYY-MM-DD。不能用 toISOString(那是 UTC)。 */
export function localDateKey(date: Date = new Date()): string {
  const year = date.getFullYear();
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");
  return `${year}-${month}-${day}`;
}

/** 键级准确率:正确键事件 / 总键事件。零输入时返回 null(不显示假数据)。 */
export function accuracy(
  keystrokes: number,
  wrongKeyEvents: number,
): number | null {
  if (keystrokes <= 0) return null;
  return (keystrokes - wrongKeyEvents) / keystrokes;
}

/** 每分钟键数。零时长返回 null。 */
export function kpm(keystrokes: number, activeMs: number): number | null {
  if (keystrokes <= 0 || activeMs <= 0) return null;
  return keystrokes / (activeMs / 60_000);
}

/** 把毫秒格式化为简洁中文时长,如 "45 秒" / "12 分钟" / "1 小时 3 分钟"。 */
export function formatDuration(ms: number): string {
  const totalSeconds = Math.round(ms / 1000);
  if (totalSeconds < 60) return `${totalSeconds} 秒`;
  const totalMinutes = Math.floor(totalSeconds / 60);
  if (totalMinutes < 60) return `${totalMinutes} 分钟`;
  const hours = Math.floor(totalMinutes / 60);
  const minutes = totalMinutes % 60;
  return minutes === 0 ? `${hours} 小时` : `${hours} 小时 ${minutes} 分钟`;
}

/** 把 0..1 的比例格式化为百分比文本,如 "92%"。 */
export function formatPercent(ratio: number | null): string {
  if (ratio === null) return "—";
  return `${Math.round(ratio * 100)}%`;
}

/** 每分钟汉字数(CPM);时长不足 1s 或无汉字返回 null。 */
export function cpm(chars: number, elapsedMs: number): number | null {
  if (chars <= 0 || elapsedMs < 1000) return null;
  return chars / (elapsedMs / 60000);
}
