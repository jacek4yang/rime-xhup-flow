/**
 * 训练数据的不可变索引:加载后构建一次,供练习/错题/键位视图共享。
 *
 * 除单字外,还索引固定词、一级简码、canonical v2 PRIMARY/FIXED_FIRST
 * 练习池与组句 fixtures,
 * 并把全部条目投影为统一的 {@link TrainingItem} 域模型(练习引擎只消费
 * 该抽象,不为任何模式单写一套调度)。
 */

import type {
  TrainerDataset,
  TrainerEntry,
  TrainerLevel1Shortcut,
  TrainerSentence,
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
  /** 同目标、同码长的规范等价主练码；不从拼音推造。 */
  acceptedCodes: readonly string[];
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

const NO_READINGS: readonly string[] = Object.freeze([]);

/** 单字条目 → 统一训练项。 */
export function charItem(entry: TrainerEntry): TrainingItem {
  return {
    id: itemId(entry),
    kind: "char",
    target: entry.char,
    primaryCode: entry.code,
    acceptedCodes: [entry.code],
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
    acceptedCodes: [shortcut.key],
    alternateCode: fullCode,
    charCount: 1,
    codeLength: 1,
    frequencyScore,
    frozen: true,
    components: null,
    readings: NO_READINGS,
  };
}

/** 固定词条目 → 统一训练项。 */
export function wordItem(word: TrainerWord): TrainingItem {
  return {
    id: wordId(word),
    kind: "word",
    target: word.word,
    primaryCode: word.code,
    acceptedCodes: [word.code],
    alternateCode: null,
    charCount: word.charCount,
    codeLength: word.length,
    frequencyScore: word.rimeWeight,
    frozen: true,
    components: null,
    readings: NO_READINGS,
  };
}

/** 简码条目 → 统一训练项(主练码 = 简码,备用 = 全码)。 */
export function shortcutItem(shortcut: TrainerShortcut): TrainingItem {
  let charCount = 0;
  for (const _character of shortcut.word) charCount++;
  return {
    id: shortcutId(shortcut),
    kind: "shortcut",
    target: shortcut.word,
    primaryCode: shortcut.shortcutCode,
    acceptedCodes: [shortcut.shortcutCode],
    alternateCode: shortcut.fullCode,
    charCount,
    codeLength: shortcut.shortcutCode.length,
    frequencyScore: 0,
    frozen: true,
    components: null,
    readings: NO_READINGS,
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
    acceptedCodes: [sentence.code],
    alternateCode: null,
    charCount: [...sentence.text].length,
    codeLength: sentence.code.length,
    frequencyScore: 0,
    frozen: false,
    components: sentence.components,
    readings: NO_READINGS,
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
  "shortcut-primary",
  "shortcut-fixed-first",
  "shortcut-primary-two-key",
  "level1",
  "sentence",
];

/** 校验后的 V4 数据集索引(不可变;加载后构建一次)。 */
export type TrainerIndex = {
  dataset: TrainerDataset;
  /** 统一训练项:`${id}` → 条目(覆盖全部条目种类)。 */
  byId: ReadonlyMap<string, TrainingItem>;
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

type TrainingSource = TrainerEntry | TrainerWord | TrainerLevel1Shortcut | TrainerShortcut | TrainerSentence;

/** Complete ID lookup; materialization is bounded by this dataset, not session history. */
class TrainingItemMap implements ReadonlyMap<string, TrainingItem> {
  constructor(
    private readonly sources: ReadonlyMap<string, TrainingSource>,
    private readonly resolve: (source: TrainingSource) => TrainingItem,
  ) {}

  get size() { return this.sources.size; }
  get(key: string): TrainingItem | undefined {
    const source = this.sources.get(key);
    return source === undefined ? undefined : this.resolve(source);
  }
  has(key: string) { return this.sources.has(key); }
  keys() { return this.sources.keys(); }
  *values(): MapIterator<TrainingItem> {
    for (const source of this.sources.values()) yield this.resolve(source);
  }
  *entries(): MapIterator<[string, TrainingItem]> {
    for (const [key, source] of this.sources) yield [key, this.resolve(source)] as [string, TrainingItem];
  }
  [Symbol.iterator]() { return this.entries(); }
  forEach(
    callback: (value: TrainingItem, key: string, map: ReadonlyMap<string, TrainingItem>) => void,
    thisArg?: unknown,
  ): void {
    for (const [key, value] of this) callback.call(thisArg, value, key, this);
  }
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

/** Index all validated rows once; only materialize/sort the pools a caller uses. */
export function buildTrainerIndex(dataset: TrainerDataset): TrainerIndex {
  const sources = new Map<string, TrainingSource>();
  const byCharLength: Record<CodeLength, TrainerEntry[]> = { 2: [], 3: [], 4: [] };
  const charFrequency = new Map<string, number>();
  const sourcePools = Object.fromEntries(
    ALL_POOL_IDS.map((id) => [id, [] as TrainingSource[]]),
  ) as Record<PoolId, TrainingSource[]>;
  const add = (source: TrainingSource, id: string, pool: PoolId) => {
    sources.set(id, source);
    sourcePools[pool].push(source);
  };
  const equivalentCodes = new Map<string, string[]>();
  for (const entry of dataset.entries) {
    const key = `${entry.char}:${entry.length}`;
    const codes = equivalentCodes.get(key);
    if (codes) {
      if (!codes.includes(entry.code)) codes.push(entry.code);
    } else {
      equivalentCodes.set(key, [entry.code]);
    }
    add(entry, itemId(entry), `char-${entry.length}`);
    byCharLength[entry.length].push(entry);
    const known = charFrequency.get(entry.char) ?? 0;
    if (entry.frequencyScore > known) charFrequency.set(entry.char, entry.frequencyScore);
  }
  for (const codes of equivalentCodes.values()) if (codes.length > 1) codes.sort();
  for (const word of dataset.words) {
    add(word, wordId(word), `word-${word.length as WordCodeLength}`);
  }
  for (const shortcut of dataset.level1Shortcuts) {
    add(shortcut, `level1:${shortcut.key}:${shortcut.char}`, "level1");
  }
  for (const shortcut of dataset.primaryShortcuts) {
    add(shortcut, shortcutId(shortcut),
      shortcut.shortcutCode.length === 2 ? "shortcut-primary-two-key" : "shortcut-primary");
  }
  for (const shortcut of dataset.fixedFirstShortcuts) {
    add(shortcut, shortcutId(shortcut), "shortcut-fixed-first");
  }
  for (const sentence of dataset.sentences) add(sentence, sentenceId(sentence), "sentence");

  // Cache by source row, not ID: two layers can share an ID but have different
  // alternate routes. Preserve each pool's row and the last-source byId rule.
  const materialized = new WeakMap<TrainingSource, TrainingItem>();
  const resolve = (source: TrainingSource): TrainingItem => {
    const cached = materialized.get(source);
    if (cached) return cached;
    let item: TrainingItem;
    if ("key" in source) {
      item = level1Item(source, longestCharCode(dataset, source.char), charFrequency.get(source.char) ?? 0);
    } else if ("shortcutCode" in source) {
      item = shortcutItem(source);
    } else if ("word" in source) {
      item = wordItem(source);
    } else if ("text" in source) {
      item = sentenceItem(source);
    } else {
      item = charItem(source);
      const codes = equivalentCodes.get(`${source.char}:${source.length}`)!;
      if (codes.length > 1) {
        const accepted = [source.code];
        for (const code of codes) if (code !== source.code) accepted.push(code);
        item.acceptedCodes = accepted;
      }
    }
    materialized.set(source, item);
    return item;
  };
  const pools = {} as Record<PoolId, TrainingItem[]>;
  const frequencySorted = {} as Record<PoolId, TrainingItem[]>;
  for (const id of ALL_POOL_IDS) {
    let items: TrainingItem[] | undefined;
    let sorted: TrainingItem[] | undefined;
    Object.defineProperty(pools, id, {
      enumerable: true,
      get: () => items ??= sourcePools[id].map(resolve),
    });
    Object.defineProperty(frequencySorted, id, {
      enumerable: true,
      get: () => sorted ??= sortedPool(pools, id),
    });
  }
  return {
    dataset,
    byId: new TrainingItemMap(sources, resolve),
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
