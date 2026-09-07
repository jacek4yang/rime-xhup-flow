/**
 * 键位触感反馈:把产品设置(off/light/medium)如实映射到微信
 * `Taro.vibrateShort` 的物理档位(heavy/medium/light)。
 *
 * 语义事件(tap/wrong/success)遵循共享核心的 {@link HapticsAdapter}
 * 契约:环境不支持或用户关闭时静默无效,绝不假装档位不同——
 * 每次映射到的都是设备真实支持的震动类型。
 *
 * 纯映射逻辑在本文件(node 测试可运行);Taro 接线在 haptics-taro.ts。
 */

import type { HapticsAdapter, HapticsMode } from "@xhup/trainer-core";

/** 触感语义事件(与共享核心 HapticsAdapter 一致)。 */
export type HapticsKind = "tap" | "wrong" | "success";

/** 微信 vibrateShort 支持的物理档位。 */
export type VibrationType = "heavy" | "medium" | "light";

/**
 * 设置档位在物理档位序列中的下标(off 不参与,调用方先行短路)。
 * light → 0,medium → 1。
 */
const SETTING_LEVEL: Record<Exclude<HapticsMode, "off">, number> = {
  light: 0,
  medium: 1,
};

/** 物理档位从弱到强。 */
const VIBRATION_TYPES: readonly VibrationType[] = ["light", "medium", "heavy"];

/**
 * 语义事件 → 物理震动类型的诚实映射:
 * - tap / success:使用用户设置的档位本身;
 * - wrong:在设置档位上加强一档(真实存在的更强震动,不是时长伪装),
 *   已是 medium 时用 heavy。
 */
export function resolveVibrationType(
  mode: HapticsMode,
  kind: HapticsKind,
): VibrationType | null {
  if (mode === "off") return null;
  const base = SETTING_LEVEL[mode];
  const level = kind === "wrong" ? Math.min(base + 1, VIBRATION_TYPES.length - 1) : base;
  return VIBRATION_TYPES[level];
}

/** 实现 HapticsAdapter 的工厂;vibrate 由宿主注入(小程序 = Taro 封装)。 */
export function createHaptics(
  vibrate: (type: VibrationType) => void,
  getMode: () => HapticsMode,
): HapticsAdapter & { currentMode(): HapticsMode } {
  const fire = (kind: HapticsKind): void => {
    const type = resolveVibrationType(getMode(), kind);
    if (type === null) return;
    try {
      vibrate(type);
    } catch {
      // 设备/环境不支持时静默无效,绝不崩溃。
    }
  };
  return {
    tap: () => fire("tap"),
    wrong: () => fire("wrong"),
    success: () => fire("success"),
    currentMode: getMode,
  };
}
