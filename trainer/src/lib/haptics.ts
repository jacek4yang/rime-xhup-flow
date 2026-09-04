/**
 * 键位触感反馈:Android WebView 的 navigator.vibrate 封装。
 *
 * 模式由用户设置(off/light/medium);环境不支持时静默返回 false,
 * 绝不崩溃。桌面/无振动设备自动无效。反馈节奏刻意短促,避免打扰。
 */

export type HapticsKind = "key" | "wrong" | "success";

/** 各反馈的基础时长(毫秒;刻意克制)。 */
const BASE_MS: Record<HapticsKind, number> = {
  key: 8,
  wrong: 22,
  success: 14,
};

function vibrateSupported(): boolean {
  return typeof navigator !== "undefined" && typeof navigator.vibrate === "function";
}

/**
 * 触发一次触感反馈;返回是否真的触发(测试/调试用)。
 * medium 相对 light 增强 50%;off 永不触发。
 */
export function haptic(
  mode: "off" | "light" | "medium",
  kind: HapticsKind,
): boolean {
  if (mode === "off" || !vibrateSupported()) return false;
  const scale = mode === "medium" ? 1.5 : 1;
  const ms = Math.round(BASE_MS[kind] * scale);
  if (ms <= 0) return false;
  try {
    return navigator.vibrate(ms);
  } catch {
    return false;
  }
}
