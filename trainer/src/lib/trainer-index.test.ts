import { describe, expect, it } from "vitest";
import type { TrainerEntry } from "./trainer-data";
import { buildTrainerIndex, selectPool } from "./trainer-index";

function makeEntry(
  char: string,
  code: string,
  frequencyScore: number,
  rimeWeight = 1,
): TrainerEntry {
  return {
    char,
    code,
    length: code.length as 2 | 3 | 4,
    readings: ["x"],
    frequencyScore,
    rimeWeight,
  };
}

function makeIndex() {
  const entries = [
    makeEntry("的", "de", 1000),
    makeEntry("一", "yi", 900),
    makeEntry("行", "xki", 800),
    makeEntry("长", "ilu", 700),
    makeEntry("字", "zipd", 600),
    makeEntry("码", "mamb", 500),
  ];
  return buildTrainerIndex({
    schemaVersion: 1,
    packageVersion: "0.1.0",
    entries,
    doublePinyin: { initials: [], finals: [], zeroInitials: [] },
  });
}

describe("buildTrainerIndex", () => {
  it("按码长分组且只含对应长度", () => {
    const index = makeIndex();
    expect(index.byLength[2].every((e) => e.length === 2)).toBe(true);
    expect(index.byLength[3].every((e) => e.length === 3)).toBe(true);
    expect(index.byLength[4].every((e) => e.length === 4)).toBe(true);
    expect(index.byLength[2]).toHaveLength(2);
    expect(index.byLength[3]).toHaveLength(2);
    expect(index.byLength[4]).toHaveLength(2);
  });

  it("byId 覆盖全部条目", () => {
    const index = makeIndex();
    expect(index.byId.get("的:de")?.code).toBe("de");
    expect(index.byId.size).toBe(6);
  });
});

describe("selectPool", () => {
  it("beginner 取词频前 800,池不足时取全部", () => {
    const index = makeIndex();
    expect(selectPool(index, 2, "beginner")).toHaveLength(2);
    expect(selectPool(index, 3, "beginner")).toHaveLength(2);
  });

  it("daily 取前 3000,full 取全部", () => {
    const index = makeIndex();
    expect(selectPool(index, 4, "daily")).toHaveLength(2);
    expect(selectPool(index, 4, "full")).toHaveLength(2);
  });

  it("词频排序确定性:分高在前,同分按 rimeWeight/字/码", () => {
    const a = makeEntry("甲", "jaa", 100, 2);
    const b = makeEntry("乙", "yib", 100, 1);
    const c = makeEntry("丙", "bbb", 50, 9);
    const index = buildTrainerIndex({
      schemaVersion: 1,
      packageVersion: "0.1.0",
      entries: [b, c, a],
      doublePinyin: { initials: [], finals: [], zeroInitials: [] },
    });
    const pool = selectPool(index, 3, "full");
    expect(pool.map((e) => e.char)).toEqual(["甲", "乙", "丙"]);
  });
});
