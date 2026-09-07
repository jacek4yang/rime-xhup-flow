/**
 * 触感反馈的 Taro 接线:把页面语义事件接到本机设置 + vibrateShort。
 * 页面统一经 haptic(kind) 调用;设置变化即时生效。
 */

import Taro from "@tarojs/taro";
import { createHaptics } from "./haptics";
import { appStore } from "./store";

function vibrate(type: "heavy" | "medium" | "light"): void {
  Taro.vibrateShort({ type });
}

const adapter = createHaptics(vibrate, () => appStore.getState().settings.haptics);

/** 键位触感反馈入口(页面语义事件)。 */
export const haptic = {
  tap: () => adapter.tap(),
  wrong: () => adapter.wrong(),
  success: () => adapter.success(),
};
