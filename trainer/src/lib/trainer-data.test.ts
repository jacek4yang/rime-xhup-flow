import { describe, expect, it } from "vitest";
import {
  TrainerDataError,
  itemId,
  sentenceId,
  shortcutId,
  validateTrainerDataset,
  wordId,
  type TrainerDataset,
  type TrainerEntry,
} from "./trainer-data";

function makeEntry(overrides: Partial<TrainerEntry> = {}): TrainerEntry {
  return {
    char: "行",
    code: "xk",
    length: 2,
    readings: ["xing"],
    frequencyScore: 123,
    rimeWeight: 42,
    ...overrides,
  };
}

function makeDataset(overrides: Partial<TrainerDataset> = {}): TrainerDataset {
  return {
    schemaVersion: 2,
    packageVersion: "0.1.0",
    entries: [makeEntry()],
    words: [{ word: "我们", code: "womf", length: 4, charCount: 2, rimeWeight: 9 }],
    level1Shortcuts: Array.from({ length: 26 }, (_, index) => ({
      key: "abcdefghijklmnopqrstuvwxyz"[index] ?? "a",
      char: `甲乙丙丁戊己庚辛壬癸子丑寅卯辰巳午未申酉戌亥甲乙丙丁戊`[index] ?? "甲",
    })),
    wordShortcuts: [{ word: "时间", fullCode: "uijm", shortcutCode: "uij", mode: "FF" }],
    fixedFirstShortcuts: [
      { word: "发展", fullCode: "favj", shortcutCode: "faj", mode: "FFI" },
    ],
    twoKeyShortcuts: [{ word: "记得", fullCode: "jide", shortcutCode: "jd", mode: "II" }],
    sentences: [{ text: "我们时间", code: "womfuijm", components: ["我们", "时间"] }],
    doublePinyin: {
      initials: [{ initial: "sh", key: "u" }],
      finals: [{ final: "ong", key: "s" }],
      zeroInitials: [{ syllable: "a", code: "aa" }],
    },
    ...overrides,
  };
}

describe("validateTrainerDataset", () => {
  it("接受合法的 V2 数据集", () => {
    const dataset = validateTrainerDataset(makeDataset());
    expect(dataset.schemaVersion).toBe(2);
    expect(dataset.entries).toHaveLength(1);
    expect(dataset.words).toHaveLength(1);
    expect(dataset.level1Shortcuts).toHaveLength(26);
    expect(dataset.sentences).toHaveLength(1);
  });

  it("拒绝 V1 与未知 schemaVersion", () => {
    expect(() =>
      validateTrainerDataset({ ...makeDataset(), schemaVersion: 1 }),
    ).toThrow(/版本应为 2/);
    expect(() =>
      validateTrainerDataset({ ...makeDataset(), schemaVersion: 3 }),
    ).toThrow(TrainerDataError);
  });

  it("拒绝缺少字段的数据", () => {
    const raw = makeDataset() as Record<string, unknown>;
    delete raw.entries;
    expect(() => validateTrainerDataset(raw)).toThrow(/entries/);
    expect(() => validateTrainerDataset(null)).toThrow(TrainerDataError);
    expect(() =>
      validateTrainerDataset({ ...makeDataset(), packageVersion: "" }),
    ).toThrow(/packageVersion/);
  });

  it("拒绝非法 code", () => {
    expect(() =>
      validateTrainerDataset(makeDataset({ entries: [makeEntry({ code: "Xk" })] })),
    ).toThrow(/code/);
    expect(() =>
      validateTrainerDataset(makeDataset({ entries: [makeEntry({ code: "aaaaa" })] })),
    ).toThrow(/code/);
  });

  it("拒绝 length 与 code 不一致", () => {
    expect(() =>
      validateTrainerDataset(makeDataset({ entries: [makeEntry({ length: 3 })] })),
    ).toThrow(/length/);
  });

  it("拒绝重复的 (char, code)", () => {
    expect(() =>
      validateTrainerDataset(
        makeDataset({ entries: [makeEntry(), makeEntry({ readings: ["hang"] })] }),
      ),
    ).toThrow(/重复/);
  });

  it("拒绝负数、非整数与超出安全整数的 frequencyScore", () => {
    for (const score of [-1, 1.5, Number.MAX_SAFE_INTEGER + 1]) {
      expect(() =>
        validateTrainerDataset(
          makeDataset({ entries: [makeEntry({ frequencyScore: score })] }),
        ),
      ).toThrow(/frequencyScore/);
    }
  });

  it("拒绝 rimeWeight <= 0", () => {
    expect(() =>
      validateTrainerDataset(makeDataset({ entries: [makeEntry({ rimeWeight: 0 })] })),
    ).toThrow(/rimeWeight/);
  });

  it("拒绝非法双拼键位(大写)", () => {
    const raw = makeDataset();
    raw.doublePinyin.initials = [{ initial: "b", key: "Z" }];
    expect(() => validateTrainerDataset(raw)).toThrow(TrainerDataError);
  });

  it("拒绝多字符 char 与重复读音", () => {
    expect(() =>
      validateTrainerDataset(makeDataset({ entries: [makeEntry({ char: "汉字" })] })),
    ).toThrow(/char/);
    expect(() =>
      validateTrainerDataset(
        makeDataset({ entries: [makeEntry({ readings: ["xing", "xing"] })] }),
      ),
    ).toThrow(/readings/);
  });

  it("拒绝 word 与 code 形状不符的词条", () => {
    // 三字词给 4 键码 → 应拒绝(逐字双拼 = 字数 × 2)。
    expect(() =>
      validateTrainerDataset(
        makeDataset({
          words: [{ word: "我们爱", code: "womf", length: 4, charCount: 2, rimeWeight: 1 }],
        }),
      ),
    ).toThrow(TrainerDataError);
  });

  it("拒绝 shortcutCode 不短于 fullCode 的简码", () => {
    expect(() =>
      validateTrainerDataset(
        makeDataset({
          wordShortcuts: [
            { word: "时间", fullCode: "uijm", shortcutCode: "uijm", mode: "FF" },
          ],
        }),
      ),
    ).toThrow(/短于/);
  });

  it("拒绝 components 与 text 不一致的组句", () => {
    expect(() =>
      validateTrainerDataset(
        makeDataset({
          sentences: [{ text: "我们时间", code: "womfuijm", components: ["我们"] }],
        }),
      ),
    ).toThrow(/components/);
  });

  it("拒绝重复的一级简码键", () => {
    const level1 = makeDataset().level1Shortcuts.slice();
    level1[1] = { ...level1[0]! };
    expect(() =>
      validateTrainerDataset(makeDataset({ level1Shortcuts: level1 })),
    ).toThrow(/重复/);
  });
});

describe("稳定 ID", () => {
  it("itemId 是稳定的 char:code 形式", () => {
    expect(itemId({ char: "行", code: "xk" })).toBe("行:xk");
  });

  it("wordId / shortcutId / sentenceId 带种类前缀", () => {
    expect(wordId({ word: "我们", code: "womf" })).toBe("word:我们:womf");
    expect(shortcutId({ word: "时间", shortcutCode: "uij" })).toBe("shortcut:时间:uij");
    expect(sentenceId({ text: "我们时间" })).toBe("sentence:我们时间");
  });
});
