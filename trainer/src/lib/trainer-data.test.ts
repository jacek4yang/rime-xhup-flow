import { describe, expect, it } from "vitest";
import {
  TrainerDataError,
  itemId,
  validateTrainerDataset,
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

function makeDoublePinyin() {
  const letters = "abcdefghijklmnopqrstuvwxyz";
  const initials = Array.from({ length: 23 }, (_, i) => ({
    initial: `x${letters[i % 26] ?? "a"}`,
    key: letters[i] ?? "a",
  }));
  const finals = Array.from({ length: 33 }, (_, i) => ({
    final: `y${letters[i % 26] ?? "a"}`,
    key: letters[i % 26] ?? "a",
  }));
  const zeroInitials = Array.from({ length: 12 }, (_, i) => ({
    syllable: `z${letters[i % 26] ?? "a"}`,
    code: `${letters[i % 26] ?? "a"}${letters[(i + 1) % 26] ?? "a"}`,
  }));
  return { initials, finals, zeroInitials };
}

function makeDataset(entries: TrainerEntry[] = [makeEntry()]) {
  return {
    schemaVersion: 1,
    packageVersion: "0.1.0",
    entries,
    doublePinyin: makeDoublePinyin(),
  };
}

describe("validateTrainerDataset", () => {
  it("接受合法的 V1 数据集", () => {
    const dataset = validateTrainerDataset(makeDataset());
    expect(dataset.schemaVersion).toBe(1);
    expect(dataset.entries).toHaveLength(1);
    expect(dataset.doublePinyin.initials).toHaveLength(23);
  });

  it("拒绝不支持的 schemaVersion", () => {
    expect(() => validateTrainerDataset(makeDataset())).not.toThrow();
    expect(() =>
      validateTrainerDataset({ ...makeDataset(), schemaVersion: 2 }),
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
      validateTrainerDataset(makeDataset([makeEntry({ code: "Xk" })])),
    ).toThrow(/code/);
    expect(() =>
      validateTrainerDataset(makeDataset([makeEntry({ code: "aaaaa" })])),
    ).toThrow(/code/);
  });

  it("拒绝 length 与 code 不一致", () => {
    expect(() =>
      validateTrainerDataset(makeDataset([makeEntry({ length: 3 })])),
    ).toThrow(/length/);
  });

  it("拒绝重复的 (char, code)", () => {
    expect(() =>
      validateTrainerDataset(
        makeDataset([makeEntry(), makeEntry({ readings: ["hang"] })]),
      ),
    ).toThrow(/重复/);
  });

  it("拒绝负数 frequencyScore", () => {
    expect(() =>
      validateTrainerDataset(makeDataset([makeEntry({ frequencyScore: -1 })])),
    ).toThrow(/frequencyScore/);
  });

  it("拒绝非整数 frequencyScore", () => {
    expect(() =>
      validateTrainerDataset(makeDataset([makeEntry({ frequencyScore: 1.5 })])),
    ).toThrow(/frequencyScore/);
  });

  it("拒绝超出安全整数的 frequencyScore", () => {
    expect(() =>
      validateTrainerDataset(
        makeDataset([makeEntry({ frequencyScore: Number.MAX_SAFE_INTEGER + 1 })]),
      ),
    ).toThrow(/frequencyScore/);
  });

  it("拒绝 rimeWeight <= 0", () => {
    expect(() =>
      validateTrainerDataset(makeDataset([makeEntry({ rimeWeight: 0 })])),
    ).toThrow(/rimeWeight/);
  });

  it("拒绝非法双拼键位", () => {
    const raw = makeDataset();
    raw.doublePinyin.initials[0] = { initial: "b", key: "Z" };
    expect(() => validateTrainerDataset(raw)).toThrow(/键位/);
  });

  it("拒绝多字符 char 与重复读音", () => {
    expect(() =>
      validateTrainerDataset(makeDataset([makeEntry({ char: "汉字" })])),
    ).toThrow(/char/);
    expect(() =>
      validateTrainerDataset(
        makeDataset([makeEntry({ readings: ["xing", "xing"] })]),
      ),
    ).toThrow(/readings/);
  });
});

describe("itemId", () => {
  it("是稳定的 char:code 形式", () => {
    expect(itemId(makeEntry())).toBe("行:xk");
  });
});
