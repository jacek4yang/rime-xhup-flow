import { describe, expect, it } from "vitest";
import {
  buildTrainerIndex,
  charItem,
  selectPool,
} from "./trainer-index";
import { makeDataset, makeEntry } from "../testing/fixtures";

describe("buildTrainerIndex(V4)", () => {
  const index = buildTrainerIndex(makeDataset());


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
    const item = index.pools["shortcut-primary"][0];
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

  it("按需查询多音字不会把另一主练路线的首码混入当前路线", () => {
    const dataset = makeDataset();
    dataset.entries = [makeEntry("行", "xk", 10), makeEntry("行", "hh", 10)];
    const cold = buildTrainerIndex(dataset);
    expect(cold.byId.get("行:xk")?.acceptedCodes).toEqual(["xk", "hh"]);
    expect(cold.byId.get("行:hh")?.acceptedCodes).toEqual(["hh", "xk"]);
    expect(selectPool(cold, "char-2", "full").map((item) => item.primaryCode)).toEqual(["hh", "xk"]);
    expect(cold.byId.get("行:xk")?.acceptedCodes).toEqual(["xk", "hh"]);
  });

  it("跨层同ID不会覆盖题池自己的备用路线;进度查询保留末层优先规则", () => {
    const dataset = makeDataset();
    dataset.primaryShortcuts = [{
      word: "行长", fullCode: "hhvh", shortcutCode: "hv", rank: 1, mergedRank: 1,
    }];
    dataset.fixedFirstShortcuts = [{
      word: "行长", fullCode: "xkvh", shortcutCode: "hv", mode: "FI",
    }];
    const cold = buildTrainerIndex(dataset);
    expect(cold.byId.get("shortcut:行长:hv")?.alternateCode).toBe("xkvh");
    expect(selectPool(cold, "shortcut-primary-two-key", "full")[0].alternateCode).toBe("hhvh");
    expect(selectPool(cold, "shortcut-fixed-first", "full")[0].alternateCode).toBe("xkvh");
    expect([...cold.byId].find(([id]) => id === "shortcut:行长:hv")?.[1].alternateCode).toBe("xkvh");
  });

  it("charItem 独立可用(错题重练路径)", () => {
    const item = charItem(makeEntry("字", "zipd", 5));
    expect(item.id).toBe("字:zipd");
    expect(item.kind).toBe("char");
  });
});
