/**
 * 测试共享 fixture:构造规范形状的训练条目/索引/进度。
 * 仅测试使用;生产数据一律来自 Rust 生成的 V2 数据集。
 */

import type {
  TrainerDataset,
  TrainerEntry,
  TrainerSentence,
  TrainerShortcut,
  TrainerWord,
} from "@/lib/trainer-data";
import { buildTrainerIndex, type TrainerIndex } from "@/lib/trainer-index";
import { emptyProgress, type ItemProgress } from "@/lib/progress";

export function makeEntry(char: string, code: string, score = 0): TrainerEntry {
  return {
    char,
    code,
    length: code.length as 2 | 3 | 4,
    readings: ["x"],
    frequencyScore: score,
    rimeWeight: 1,
  };
}

export function makeWord(word: string, code: string, weight = 1): TrainerWord {
  return {
    word,
    code,
    length: code.length,
    charCount: [...word].length,
    rimeWeight: weight,
  };
}

export function makeShortcut(
  word: string,
  fullCode: string,
  shortcutCode: string,
  mode = "FI",
): TrainerShortcut {
  return { word, fullCode, shortcutCode, mode };
}

export function makeSentence(text: string, code: string): TrainerSentence {
  const components: string[] = [];
  for (let index = 0; index < [...text].length; index += 2) {
    components.push([...text].slice(index, index + 2).join(""));
  }
  return { text, code, components };
}

/** 最小合法 V2 数据集(全部 11 个池至少可构建)。 */
export function makeDataset(overrides: Partial<TrainerDataset> = {}): TrainerDataset {
  return {
    schemaVersion: 2,
    packageVersion: "0.0.0-test",
    entries: [
      makeEntry("行", "xk", 90),
      makeEntry("行", "xkg", 80),
      makeEntry("好", "hk", 70),
      makeEntry("好", "hkn", 60),
      makeEntry("好", "hknc", 50),
      makeEntry("我", "wo", 40),
      makeEntry("我", "wop", 30),
      makeEntry("我", "wopd", 20),
    ],
    words: [
      makeWord("我们", "womf", 100),
      makeWord("时间", "uijm", 90),
      makeWord("发展", "favj", 80),
      makeWord("社会保", "uehvbc", 70),
      makeWord("完完整整", "wjwjvgvg", 60),
    ],
    level1Shortcuts: [
      { key: "q", char: "去" },
      { key: "w", char: "我" },
    ],
    wordShortcuts: [makeShortcut("时间", "uijm", "uij")],
    fixedFirstShortcuts: [makeShortcut("发展", "favj", "faj")],
    twoKeyShortcuts: [makeShortcut("记得", "jide", "jd", "II")],
    sentences: [
      makeSentence("我们时间", "womfuijm"),
      makeSentence("时间发展", "uijmfavj"),
    ],
    doublePinyin: {
      initials: [{ initial: "sh", key: "u" }],
      finals: [{ final: "ong", key: "s" }],
      zeroInitials: [{ syllable: "a", code: "aa" }],
    },
    ...overrides,
  };
}

export function makeIndex(overrides: Partial<TrainerDataset> = {}): TrainerIndex {
  return buildTrainerIndex(makeDataset(overrides));
}

export function progressWith(
  overrides: Partial<ItemProgress> = {},
): ItemProgress {
  return { ...emptyProgress(), ...overrides };
}

/** 确定性 RNG(线性同余;种子相同 → 序列相同)。 */
export function seededRng(seed: number): () => number {
  let state = seed >>> 0;
  return () => {
    state = (state * 1664525 + 1013904223) >>> 0;
    return state / 0x100000000;
  };
}
