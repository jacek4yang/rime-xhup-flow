import { describe, expect, it } from "vitest";
import {
  ALL_POOL_IDS,
  buildTrainerIndex,
  charItem,
  selectPool,
} from "./trainer-index";
import { makeDataset, makeEntry } from "@/test/fixtures";

describe("buildTrainerIndex(V2)", () => {
  const index = buildTrainerIndex(makeDataset());

  it("全部池 ID 都被构建且非空", () => {
    for (const poolId of ALL_POOL_IDS) {
      expect(index.pools[poolId].length, poolId).toBeGreaterThan(0);
      expect(index.frequencySorted[poolId].length, poolId).toBeGreaterThan(0);
    }
  });

  it("byId 覆盖全部条目种类且 ID 唯一", () => {
    const expected =
      index.dataset.entries.length +
      index.dataset.words.length +
      index.dataset.level1Shortcuts.length +
      index.dataset.wordShortcuts.length +
      index.dataset.fixedFirstShortcuts.length +
      index.dataset.twoKeyShortcuts.length +
      index.dataset.sentences.length;
    expect(index.byId.size).toBe(expected);
  });

  it("单字条目投影保留读音与码长", () => {
    const item = index.byId.get("行:xk");
    expect(item).toMatchObject({
      kind: "char",
      target: "行",
      primaryCode: "xk",
      charCount: 1,
      codeLength: 2,
      frozen: true,
    });
    expect(item?.readings).toContain("x");
  });

  it("一级简码条目:主练码为单键,备用码为该字最长全码", () => {
    const item = index.pools.level1.find((candidate) => candidate.target === "我");
    expect(item).toMatchObject({
      kind: "level1",
      primaryCode: "w",
      frozen: true,
    });
    expect(item?.alternateCode).toBe("wopd"); // fixture 中 我 的最长全码
  });

  it("简码条目:主练码 = 简码,备用码 = 全码", () => {
    const item = index.pools["shortcut-zero-regression"][0];
    expect(item).toMatchObject({
      kind: "shortcut",
      target: "时间",
      primaryCode: "uij",
      alternateCode: "uijm",
      frozen: true,
    });
  });

  it("词条目:全码为主练码,无备用码,字数正确", () => {
    const item = index.pools["word-4"][0];
    expect(item).toMatchObject({
      kind: "word",
      target: "我们",
      primaryCode: "womf",
      alternateCode: null,
      charCount: 2,
      codeLength: 4,
    });
  });

  it("组句条目:携带分段,非冻结", () => {
    const item = index.pools.sentence[0];
    expect(item.kind).toBe("sentence");
    expect(item.frozen).toBe(false);
    expect(item.components).not.toBeNull();
    expect(item.components?.join("")).toBe(item.target);
  });

  it("频率排序:降序且不受 locale 影响", () => {
    const sorted = index.frequencySorted["char-2"];
    for (let i = 1; i < sorted.length; i += 1) {
      expect(sorted[i - 1]!.frequencyScore).toBeGreaterThanOrEqual(
        sorted[i]!.frequencyScore,
      );
    }
  });

  it("selectPool:难度截断取频率前 N 条;full 返回整池副本", () => {
    const beginner = selectPool(index, "char-2", "beginner");
    const full = selectPool(index, "char-2", "full");
    expect(beginner.length).toBeLessThanOrEqual(800);
    expect(full).toHaveLength(index.pools["char-2"].length);
    expect(full).not.toBe(index.pools["char-2"]);
    expect(full[0]?.frequencyScore).toBeGreaterThanOrEqual(
      full[full.length - 1]?.frequencyScore ?? 0,
    );
  });

  it("charItem 独立可用(错题重练路径)", () => {
    const item = charItem(makeEntry("字", "zipd", 5));
    expect(item.id).toBe("字:zipd");
    expect(item.kind).toBe("char");
  });
});
