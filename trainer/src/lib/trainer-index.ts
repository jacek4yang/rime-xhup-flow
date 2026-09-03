/**
 * 训练数据的不可变索引:加载后构建一次,供练习/错题/键位视图共享。
 *
 * V2:除单字外,还索引固定词、一级简码、三个生产简码层与组句 fixtures,
 * 并把全部条目投影为统一的 {@link TrainingItem} 域模型(练习引擎只消费
 * 该抽象,不为任何模式单写一套调度)。
 */

import type {
  TrainerDataset,
  TrainerEntry,
  TrainerShortcut,
  TrainerShortcutLayer,
  TrainerWord,
} from "./trainer-data";
import {
  itemId,
  sentenceId,
  shortcutId,
  wordId,
} from "./trainer-data";

/** 单字段码长。 */
export type CodeLength = 2 | 3 | 4;

/** 词全码键数(逐字双拼,2~4 字词)。 */
export type WordCodeLength = 4 | 6 | 8;

/**
 * 统一训练项抽象:引擎/调度/弱点分析共用的最小域模型。
 *
 * - `primaryCode`:主练码(简码层的简码;词/句为全码);
 * - `alternateCode`:备用合法码(简码条目的全码;引擎接受两条路线,
 *   但主练码才算完美——见 B5 简码评分契约);
 * - `frozen`:是否为用户肌肉记忆兼容接口(生产简码层/单字/词全部为
 *   canonical 冻结数据;组句 fixtures 是练习数据,不是输入方案契约)。
 */
export type TrainingItem = {
  id: string;
  kind: "char" | "level1" | "word" | "shortcut" | "sentence";
  /** 展示目标(汉字 / 词语 / 句子)。 */
  target: string;
  primaryCode: string;
  alternateCode: string | null;
  /** 汉字数(CPM 分母)。 */
  charCount: number;
  /** 主练码键数。 */
  codeLength: number;
  /** 频率证据(0..,越高越常用)。 */
  frequencyScore: number;
  frozen: boolean;
  /** 句子分段(仅句子条目)。 */
  components: readonly string[] | null;
  /** 读音(仅单字条目)。 */
  readings: readonly string[];
};

/** 单字条目 → 统一训练项。 */
export function charItem(entry: TrainerEntry): TrainingItem {
  return {
    id: itemId(entry),
    kind: "char",
    target: entry.char,
    primaryCode: entry.code,
    alternateCode: null,
    charCount: 1,
    codeLength: entry.length,
    frequencyScore: entry.frequencyScore,
    frozen: true,
    components: null,
    readings: entry.readings,
  };
}

/** 一级简码条目 → 统一训练项(备用合法码 = 该字全码)。 */
export function level1Item(
  shortcut: { key: string; char: string },
  fullCode: string,
  frequencyScore: number,
): TrainingItem {
  return {
    id: `level1:${shortcut.key}:${shortcut.char}`,
    kind: "level1",
    target: shortcut.char,
    primaryCode: shortcut.key,
    alternateCode: fullCode,
    charCount: 1,
    codeLength: 1,
    frequencyScore,
    frozen: true,
    components: null,
    readings: [],
  };
}

/** 固定词条目 → 统一训练项。 */
export function wordItem(word: TrainerWord): TrainingItem {
  return {
    id: wordId(word),
    kind: "word",
    target: word.word,
    primaryCode: word.code,
    alternateCode: null,
    charCount: word.charCount,
    codeLength: word.length,
    frequencyScore: word.rimeWeight,
    frozen: true,
    components: null,
    readings: [],
  };
}

/** 简码条目 → 统一训练项(主练码 = 简码,备用 = 全码)。 */
export function shortcutItem(shortcut: TrainerShortcut): TrainingItem {
  return {
    id: shortcutId(shortcut),
    kind: "shortcut",
    target: shortcut.word,
    primaryCode: shortcut.shortcutCode,
    alternateCode: shortcut.fullCode,
    charCount: [...shortcut.word].length,
    codeLength: shortcut.shortcutCode.length,
    frequencyScore: 0,
    frozen: true,
    components: null,
    readings: [],
  };
}

/** 组句条目 → 统一训练项(连续输入整个拼接码)。 */
export function sentenceItem(sentence: {
  text: string;
  code: string;
  components: string[];
}): TrainingItem {
  return {
    id: sentenceId(sentence),
    kind: "sentence",
    target: sentence.text,
    primaryCode: sentence.code,
    alternateCode: null,
    charCount: [...sentence.text].length,
    codeLength: sentence.code.length,
    frequencyScore: 0,
    frozen: false,
    components: sentence.components,
    readings: [],
  };
}

/** 频率证据排序:不使用 locale 相关比较,决胜键为 (target, primaryCode)。 */
export function compareByFrequency(a: TrainingItem, b: TrainingItem): number {
  if (a.frequencyScore !== b.frequencyScore) {
    return b.frequencyScore - a.frequencyScore;
  }
  if (a.codeLength !== b.codeLength) {
    return a.codeLength - b.codeLength;
  }
  if (a.target !== b.target) return a.target < b.target ? -1 : 1;
  return a.primaryCode < b.primaryCode
    ? -1
    : a.primaryCode > b.primaryCode
      ? 1
      : 0;
}

/** 池 ID:单字按码长,词按码长,简码按层,组句单池。 */
export type PoolId =
  | `char-${CodeLength}`
  | `word-${WordCodeLength}`
  | `shortcut-${TrainerShortcutLayer}`
  | "level1"
  | "sentence";

export const ALL_POOL_IDS: PoolId[] = [
  "char-2",
  "char-3",
  "char-4",
  "word-4",
  "word-6",
  "word-8",
  "shortcut-zero-regression",
  "shortcut-fixed-first",
  "shortcut-two-key-zero-regression",
  "level1",
  "sentence",
];

/** 校验后的 V2 数据集索引(不可变;加载后构建一次)。 */
export type TrainerIndex = {
  dataset: TrainerDataset;
  /** 统一训练项:`${id}` → 条目(覆盖全部条目种类)。 */
  byId: Map<string, TrainingItem>;
  /** 单字条目(生成顺序)。 */
  chars: TrainerEntry[];
  byCharLength: Record<CodeLength, TrainerEntry[]>;
  /** 按池 ID 分组的统一训练项(生成序)。 */
  pools: Record<PoolId, TrainingItem[]>;
  /** 按频率证据排序的池(供难度截断)。 */
  frequencySorted: Record<PoolId, TrainingItem[]>;
};

function sortedPool(
  pools: Record<PoolId, TrainingItem[]>,
  id: PoolId,
): TrainingItem[] {
  return [...pools[id]].sort(compareByFrequency);
}

/** 单字在规范数据中的最长全码(一级简码的备用合法码)。 */
function longestCharCode(dataset: TrainerDataset, char: string): string {
  let longest = "";
  for (const entry of dataset.entries) {
    if (entry.char === char && entry.code.length > longest.length) {
      longest = entry.code;
    }
  }
  return longest;
}

/** 加载校验后构建一次;之后视为不可变。 */
export function buildTrainerIndex(dataset: TrainerDataset): TrainerIndex {
  const byId = new Map<string, TrainingItem>();
  const byCharLength: Record<CodeLength, TrainerEntry[]> = { 2: [], 3: [], 4: [] };

  const charFrequency = new Map<string, number>();
  const pools: Record<PoolId, TrainingItem[]> = {
    "char-2": [],
    "char-3": [],
    "char-4": [],
    "word-4": [],
    "word-6": [],
    "word-8": [],
    "shortcut-zero-regression": [],
    "shortcut-fixed-first": [],
    "shortcut-two-key-zero-regression": [],
    level1: [],
    sentence: [],
  };

  for (const entry of dataset.entries) {
    const item = charItem(entry);
    byId.set(item.id, item);
    byCharLength[entry.length].push(entry);
    pools[`char-${entry.length}`].push(item);
    // 同字多条码时,保留频率证据最大者供一级简码引用。
    const known = charFrequency.get(entry.char) ?? 0;
    if (entry.frequencyScore > known) {
      charFrequency.set(entry.char, entry.frequencyScore);
    }
  }
  for (const word of dataset.words) {
    const item = wordItem(word);
    byId.set(item.id, item);
    pools[`word-${word.length as WordCodeLength}`].push(item);
  }
  for (const shortcut of dataset.level1Shortcuts) {
    const item = level1Item(
      shortcut,
      longestCharCode(dataset, shortcut.char),
      charFrequency.get(shortcut.char) ?? 0,
    );
    byId.set(item.id, item);
    pools.level1.push(item);
  }
  const shortcutLayers = [
    ["wordShortcuts", "shortcut-zero-regression"],
    ["fixedFirstShortcuts", "shortcut-fixed-first"],
    ["twoKeyShortcuts", "shortcut-two-key-zero-regression"],
  ] as const;
  for (const [datasetKey, poolId] of shortcutLayers) {
    for (const shortcut of dataset[datasetKey]) {
      const item = shortcutItem(shortcut);
      byId.set(item.id, item);
      pools[poolId].push(item);
    }
  }
  for (const sentence of dataset.sentences) {
    const item = sentenceItem(sentence);
    byId.set(item.id, item);
    pools.sentence.push(item);
  }

  const frequencySorted = Object.fromEntries(
    ALL_POOL_IDS.map((id) => [id, sortedPool(pools, id)]),
  ) as Record<PoolId, TrainingItem[]>;

  return {
    dataset,
    byId,
    chars: dataset.entries,
    byCharLength,
    pools,
    frequencySorted,
  };
}

export type Difficulty = "beginner" | "daily" | "full";

/** 各难度的候选池上限(按频率证据取前 N 条;池不足时取整池)。 */
export const DIFFICULTY_POOL_LIMIT: Record<Exclude<Difficulty, "full">, number> = {
  beginner: 800,
  daily: 3000,
};

/** 按池与难度取候选池;返回新数组,不修改索引。 */
export function selectPool(
  index: TrainerIndex,
  poolId: PoolId,
  difficulty: Difficulty,
): TrainingItem[] {
  const sorted = index.frequencySorted[poolId];
  if (difficulty === "full") return [...sorted];
  return sorted.slice(0, DIFFICULTY_POOL_LIMIT[difficulty]);
}
