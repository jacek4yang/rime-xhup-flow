import { describe, expect, it } from "vitest";
import {
  CONFUSION_MAP_CAP,
  confusionsFor,
  confusionId,
  isCodePosition,
  mergeConfusionMaps,
  positionForIndex,
  positionId,
  positionLabel,
  recordConfusion,
  sanitizeConfusionMap,
  sortedConfusions,
  topConfusions,
  type ConfusionMap,
} from "./confusion";
import { makeIndex } from "../testing/fixtures";

describe("confusionId / positionId", () => {
  it("键格式为 expected>actual@positionId(如 k>m@shape1)", () => {
    expect(confusionId("k", "m", "shape1")).toBe("k>m@shape1");
    expect(confusionId("x", "z", "sound1")).toBe("x>z@sound1");
    expect(confusionId("k", "w", "sound2")).toBe("k>w@sound2");
    expect(confusionId("n", "c", "shape2")).toBe("n>c@shape2");
  });

  it("{other:N} 序列化为 oN;第 5 位 → o5", () => {
    expect(positionId({ other: 5 })).toBe("o5");
    expect(confusionId("a", "b", { other: 5 })).toBe("a>b@o5");
  });

  it("isCodePosition 接受具名四值与 {other:非负整数},拒绝其余", () => {
    expect(isCodePosition("sound1")).toBe(true);
    expect(isCodePosition("shape2")).toBe(true);
    expect(isCodePosition({ other: 0 })).toBe(true);
    expect(isCodePosition({ other: -1 })).toBe(false);
    expect(isCodePosition({ other: 1.5 })).toBe(false);
    expect(isCodePosition("sound3")).toBe(false);
    expect(isCodePosition(null)).toBe(false);
  });
});

describe("recordConfusion", () => {
  it("同三元组累加计数;不同码位各自成条", () => {
    let map: ConfusionMap = {};
    map = recordConfusion(map, "k", "m", "shape1");
    map = recordConfusion(map, "k", "m", "shape1");
    map = recordConfusion(map, "k", "m", "shape2");
    expect(map["k>m@shape1"]).toEqual({
      expected: "k",
      actual: "m",
      position: "shape1",
      count: 2,
    });
    expect(Object.keys(map)).toEqual(["k>m@shape1", "k>m@shape2"]);
  });

  it("纯函数:不修改输入表", () => {
    const map: ConfusionMap = {};
    const next = recordConfusion(map, "x", "z", "sound1");
    expect(map).toEqual({});
    expect(Object.keys(next)).toEqual(["x>z@sound1"]);
  });

  it("非法键(非单个小写 a-z)忽略", () => {
    let map: ConfusionMap = {};
    map = recordConfusion(map, "K", "m", "sound1");
    map = recordConfusion(map, "k", "mm", "sound1");
    map = recordConfusion(map, "", "m", "sound1");
    expect(map).toEqual({});
  });

  it("条目数达到上限后忽略新的三元组,已有条目继续累加", () => {
    let map: ConfusionMap = {};
    for (let i = 0; i < CONFUSION_MAP_CAP; i += 1) {
      // 用 other:N 生成互不相同的三元组。
      map = recordConfusion(map, "a", "b", { other: i });
    }
    expect(Object.keys(map).length).toBe(CONFUSION_MAP_CAP);
    const before = map["a>b@o0"]!.count;
    map = recordConfusion(map, "a", "b", { other: 0 });
    expect(map["a>b@o0"]!.count).toBe(before + 1);
    map = recordConfusion(map, "c", "d", "sound1");
    expect(map["c>d@sound1"]).toBeUndefined();
    expect(Object.keys(map).length).toBe(CONFUSION_MAP_CAP);
  });
});

describe("mergeConfusionMaps", () => {
  it("同 id 计数相加,不同 id 并集", () => {
    const a = recordConfusion(recordConfusion({}, "k", "m", "shape1"), "x", "z", "sound1");
    const b = recordConfusion(recordConfusion({}, "k", "m", "shape1"), "k", "w", "sound2");
    const merged = mergeConfusionMaps(a, b);
    expect(merged["k>m@shape1"]!.count).toBe(2);
    expect(merged["x>z@sound1"]!.count).toBe(1);
    expect(merged["k>w@sound2"]!.count).toBe(1);
  });

  it("合并结果同样受上限约束(b 的新条目按插入序丢弃)", () => {
    let a: ConfusionMap = {};
    for (let i = 0; i < CONFUSION_MAP_CAP; i += 1) {
      a = recordConfusion(a, "a", "b", { other: i });
    }
    const b = recordConfusion(recordConfusion({}, "a", "b", { other: 0 }), "c", "d", "sound1");
    const merged = mergeConfusionMaps(a, b);
    expect(merged["a>b@o0"]!.count).toBe(2);
    expect(merged["c>d@sound1"]).toBeUndefined();
  });
});

describe("positionForIndex", () => {
  it("单字条目:0→声1、1→声2、2→形1、3→形2、≥4→other", () => {
    const index = makeIndex();
    const char2 = index.pools["char-2"][0]; // 行:xk
    const char4 = index.pools["char-4"][0]; // 我:wopd
    expect(positionForIndex(char2, 0)).toBe("sound1");
    expect(positionForIndex(char2, 1)).toBe("sound2");
    expect(positionForIndex(char4, 2)).toBe("shape1");
    expect(positionForIndex(char4, 3)).toBe("shape2");
    expect(positionForIndex(char4, 4)).toEqual({ other: 4 });
  });

  it("词/句条目一律降级为 {other:N}(已知限制)", () => {
    const index = makeIndex();
    const word = index.pools["word-4"][0];
    const sentence = index.pools.sentence[0];
    expect(positionForIndex(word, 0)).toEqual({ other: 0 });
    expect(positionForIndex(word, 3)).toEqual({ other: 3 });
    expect(positionForIndex(sentence, 1)).toEqual({ other: 1 });
  });
});

describe("排序与查询", () => {
  const map: ConfusionMap = {
    ...recordConfusion(recordConfusion({}, "k", "m", "shape1"), "k", "m", "shape1"),
    ...recordConfusion({}, "x", "z", "sound1"),
    ...recordConfusion({}, "k", "w", "sound2"),
  };

  it("sortedConfusions:count 降序 → expected → actual 决胜", () => {
    expect(sortedConfusions(map).map((entry) => confusionId(entry.expected, entry.actual, entry.position))).toEqual([
      "k>m@shape1",
      "k>w@sound2",
      "x>z@sound1",
    ]);
  });

  it("topConfusions 截断", () => {
    expect(topConfusions(map, 1)).toEqual([map["k>m@shape1"]]);
  });

  it("confusionsFor 按期望键或实际键命中", () => {
    const forK = confusionsFor(map, "k");
    expect(forK.map((entry) => entry.actual)).toEqual(["m", "w"]);
    const forZ = confusionsFor(map, "z");
    expect(forZ).toEqual([map["x>z@sound1"]]);
    expect(confusionsFor(map, "q")).toEqual([]);
  });
});

describe("positionLabel", () => {
  it("具名码位 → 对应 i18n 键;other → 第 N 位插值", () => {
    expect(positionLabel("sound1")).toEqual({ key: "confusion.position.sound1" });
    expect(positionLabel("shape2")).toEqual({ key: "confusion.position.shape2" });
    expect(positionLabel({ other: 5 })).toEqual({
      key: "confusion.position.other",
      params: { n: 5 },
    });
  });
});

describe("sanitizeConfusionMap", () => {
  it("合法条目保留,损坏条目单独丢弃,绝不整体清空", () => {
    const sanitized = sanitizeConfusionMap({
      "k>m@shape1": { expected: "k", actual: "m", position: "shape1", count: 3 },
      broken: { expected: "k", actual: "MM", position: "shape1", count: 1 },
      "bad-position": { expected: "k", actual: "w", position: "middle", count: 1 },
      "negative": { expected: "k", actual: "w", position: "sound1", count: -1 },
      garbage: "nope",
    });
    expect(Object.keys(sanitized)).toEqual(["k>m@shape1"]);
    expect(sanitizeConfusionMap(null)).toEqual({});
    expect(sanitizeConfusionMap("garbage")).toEqual({});
  });

  it("count 为 0 的条目保留(调用方决定是否展示)", () => {
    const sanitized = sanitizeConfusionMap({
      "k>m@shape1": { expected: "k", actual: "m", position: "shape1", count: 0 },
    });
    expect(sanitized["k>m@shape1"]!.count).toBe(0);
  });

  it("超出上限的条目按插入序丢弃", () => {
    const raw: Record<string, unknown> = {};
    for (let i = 0; i < CONFUSION_MAP_CAP + 10; i += 1) {
      raw[`e${i}`] = { expected: "a", actual: "b", position: { other: i }, count: 1 };
    }
    expect(Object.keys(sanitizeConfusionMap(raw)).length).toBe(CONFUSION_MAP_CAP);
  });
});
