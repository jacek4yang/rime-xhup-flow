/**
 * 训练数据的不可变索引:加载后构建一次,供练习/错题/键位视图共享。
 *
 * 26753 条规范条目属于不可变运行时数据;题目选择不在每次渲染时
 * 重新过滤/排序全量数据。
 */

import type { TrainerDataset, TrainerEntry } from "./trainer-data";
import { itemId } from "./trainer-data";

export type CodeLength = 2 | 3 | 4;

export type TrainerIndex = {
  dataset: TrainerDataset;
  /** `${char}:${code}` → 条目 */
  byId: Map<string, TrainerEntry>;
  /** 按码长分组(生成顺序,即频率证据的降序序列化顺序) */
  byLength: Record<CodeLength, TrainerEntry[]>;
  /** 按频率证据排序的池:frequencyScore 降序 → rimeWeight 降序 → (char, code) 决胜 */
  frequencySorted: Record<CodeLength, TrainerEntry[]>;
};

/** 频率证据排序:不使用 locale 相关比较,决胜键为 (char, code)。 */
export function compareByFrequency(a: TrainerEntry, b: TrainerEntry): number {
  if (a.frequencyScore !== b.frequencyScore) {
    return b.frequencyScore - a.frequencyScore;
  }
  if (a.rimeWeight !== b.rimeWeight) {
    return b.rimeWeight - a.rimeWeight;
  }
  if (a.char !== b.char) return a.char < b.char ? -1 : 1;
  return a.code < b.code ? -1 : a.code > b.code ? 1 : 0;
}

/** 加载校验后构建一次;之后视为不可变。 */
export function buildTrainerIndex(dataset: TrainerDataset): TrainerIndex {
  const byId = new Map<string, TrainerEntry>();
  const byLength: Record<CodeLength, TrainerEntry[]> = { 2: [], 3: [], 4: [] };
  for (const entry of dataset.entries) {
    byId.set(itemId(entry), entry);
    byLength[entry.length].push(entry);
  }
  const frequencySorted = {
    2: [...byLength[2]].sort(compareByFrequency),
    3: [...byLength[3]].sort(compareByFrequency),
    4: [...byLength[4]].sort(compareByFrequency),
  } as Record<CodeLength, TrainerEntry[]>;
  return { dataset, byId, byLength, frequencySorted };
}

export type Difficulty = "beginner" | "daily" | "full";

/** 各难度的候选池上限(按频率证据取前 N 条;池不足时取整池)。 */
export const DIFFICULTY_POOL_LIMIT: Record<Exclude<Difficulty, "full">, number> = {
  beginner: 800,
  daily: 3000,
};

/** 按码长与难度取候选池;返回新数组,不修改索引。 */
export function selectPool(
  index: TrainerIndex,
  length: CodeLength,
  difficulty: Difficulty,
): TrainerEntry[] {
  const sorted = index.frequencySorted[length];
  if (difficulty === "full") return [...sorted];
  return sorted.slice(0, DIFFICULTY_POOL_LIMIT[difficulty]);
}
