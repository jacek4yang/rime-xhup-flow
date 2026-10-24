/**
 * 低频刷新的当前时间:页面可见时每分钟走一格,窗口重新可见/聚焦时立即
 * 校正(跨复习到期、跨零点、系统休眠恢复)。不可见时不占用计时器,
 * 避免持续高频刷新;派生 UI(到期徽标、章节状态)用它取代一次性快照。
 */

import { useEffect, useState } from "react";

export function useNow(intervalMs = 60_000): number {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const update = () => setNow(Date.now());
    const onVisibility = () => {
      if (document.visibilityState === "visible") update();
    };
    document.addEventListener("visibilitychange", onVisibility);
    window.addEventListener("focus", update);
    const timer = window.setInterval(() => {
      if (document.visibilityState === "visible") update();
    }, intervalMs);
    return () => {
      document.removeEventListener("visibilitychange", onVisibility);
      window.removeEventListener("focus", update);
      window.clearInterval(timer);
    };
  }, [intervalMs]);
  return now;
}
