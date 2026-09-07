/**
 * 练习模式可用性(纯逻辑):按规范数据分片的池内容判断哪些模式
 * 真正可练。池为空的模式在 UI 上禁用并标注「数据未加载」,绝不
 * 让空池会话崩溃(createSession 对空池返回 null,这里前置拦截)。
 */

import { MODE_POOL_ROTATION } from "@xhup/trainer-core";
import type { I18nKey } from "@xhup/trainer-core";
import type { PracticeMode, TrainerIndex } from "@xhup/trainer-core";

/** 每个模式 → 轮换中第一个非空池是否存在的映射。 */
export function computeModeAvailability(
  index: TrainerIndex,
): Record<PracticeMode, boolean> {
  const result = {} as Record<PracticeMode, boolean>;
  for (const mode of Object.keys(MODE_POOL_ROTATION) as PracticeMode[]) {
    result[mode] = MODE_POOL_ROTATION[mode].some(
      (poolId) => index.pools[poolId].length > 0,
    );
  }
  return result;
}

/** 单个模式是否可练。 */
export function isModeAvailable(
  index: TrainerIndex,
  mode: PracticeMode,
): boolean {
  return MODE_POOL_ROTATION[mode].some(
    (poolId) => index.pools[poolId].length > 0,
  );
}

/** UI 分组展示用的模式清单(顺序即展示顺序;与核心模式全集一致)。 */
export const MODE_GROUPS: readonly {
  labelKey: I18nKey;
  modes: readonly PracticeMode[];
}[] = [
  { labelKey: "practice.group.chars", modes: ["double", "sound-shape", "full", "mixed"] },
  { labelKey: "practice.group.shortcuts", modes: ["level1", "two-key-word", "zero-regression", "fixed-first", "mixed-shortcut"] },
  { labelKey: "practice.group.words", modes: ["fixed-word"] },
  { labelKey: "practice.group.sentences", modes: ["sentence"] },
  { labelKey: "practice.group.mixed", modes: ["mixed-all"] },
];
