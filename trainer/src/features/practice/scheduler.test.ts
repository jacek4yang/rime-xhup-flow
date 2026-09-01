import { describe, expect, it } from "vitest";
import type { TrainerEntry } from "@/lib/trainer-data";
import { emptyProgress, type ItemProgress } from "@/lib/progress";
import {
  buildPool,
  computePriority,
  pickNext,
  pickWeighted,
  scheduleReview,
  type QuestionPool,
} from "./scheduler";

function makeEntry(char: string, code: string, score = 0): TrainerEntry {
  return {
    char,
    code,
    length: code.length as 2 | 3 | 4,
    readings: ["x"],
    frequencyScore: score,
    rimeWeight: 1,
  };
}

function progressOf(overrides: Partial<ItemProgress>): ItemProgress {
  return { ...emptyProgress(), ...overrides };
}

const zeroRng = () => 0;

describe("computePriority", () => {
  it("词频更高优先级更高", () => {
    const low = makeEntry("甲", "ja", 10);
    const high = makeEntry("乙", "yb", 10_000);
    const pool = buildPool(2, [low, high]);
    const p = emptyProgress();
    expect(computePriority(high, p, pool.maxLogFrequency)).toBeGreaterThan(
      computePriority(low, p, pool.maxLogFrequency),
    );
  });

  it("全零词频池 maxLog 为 0 时频率增益取 0.75", () => {
    const entry = makeEntry("甲", "ja", 0);
    expect(computePriority(entry, emptyProgress(), 0)).toBeCloseTo(
      0.75 * (1 + 100 / 20) * 1.4,
    );
  });

  it("薄弱度随错误与低掌握度上升", () => {
    const entry = makeEntry("甲", "ja", 0);
    const weak = progressOf({ attempts: 2, wrong: 2, mastery: 20 });
    const strong = progressOf({ attempts: 2, correct: 2, mastery: 90 });
    expect(computePriority(entry, weak, 0)).toBeGreaterThan(
      computePriority(entry, strong, 0),
    );
  });

  it("未见条目有 1.4 倍增益", () => {
    const entry = makeEntry("甲", "ja", 0);
    const unseen = computePriority(entry, emptyProgress(), 0);
    const seen = computePriority(
      entry,
      progressOf({ attempts: 1, correct: 1, mastery: 4 }),
      0,
    );
    expect(unseen / seen).toBeGreaterThan(1.4);
  });
});

describe("pickWeighted", () => {
  const pool = (): QuestionPool =>
    buildPool(2, [makeEntry("甲", "ja", 10), makeEntry("乙", "yb", 10_000)]);

  it("从池中选题", () => {
    const result = pickWeighted(pool(), new Map(), new Set(), zeroRng);
    expect(result).not.toBeNull();
    expect(pool().entries).toContainEqual(result);
  });

  it("有备选时不立即重复最近条目", () => {
    const selected = pickWeighted(
      pool(),
      new Map(),
      new Set(["甲:ja"]),
      zeroRng,
    );
    expect(selected?.char).toBe("乙");
  });

  it("单条目池可以工作", () => {
    const single = buildPool(2, [makeEntry("甲", "ja")]);
    const selected = pickWeighted(single, new Map(), new Set(["甲:ja"]), zeroRng);
    expect(selected?.char).toBe("甲");
  });

  it("权重高的条目覆盖更大的随机区间", () => {
    const p = pool();
    const progress = new Map<string, ItemProgress>();
    const pLow = computePriority(p.entries[0]!, emptyProgress(), p.maxLogFrequency);
    const pHigh = computePriority(p.entries[1]!, emptyProgress(), p.maxLogFrequency);
    // rng 落在低频条目区间内 → 选第一个;超过则选第二个。
    const inLow = pickWeighted(p, progress, new Set(), () => (pLow * 0.5) / (pLow + pHigh));
    const pastLow = pickWeighted(p, progress, new Set(), () => (pLow + pHigh * 0.5) / (pLow + pHigh));
    expect(inLow?.char).toBe("甲");
    expect(pastLow?.char).toBe("乙");
  });
});

describe("scheduleReview", () => {
  it("回炉间隔在 3-8 题之间", () => {
    expect(scheduleReview([], "甲:ja", () => 0)[0]).toEqual({
      id: "甲:ja",
      dueIn: 3,
    });
    expect(scheduleReview([], "甲:ja", () => 0.999)[0]?.dueIn).toBe(8);
  });

  it("同一条目重复答错只保留最新计划", () => {
    const queue = scheduleReview([{ id: "甲:ja", dueIn: 5 }], "甲:ja", () => 0);
    expect(queue).toHaveLength(1);
    expect(queue[0]?.dueIn).toBe(3);
  });
});

describe("pickNext", () => {
  const pools = () => [
    buildPool(2, [makeEntry("甲", "ja")]),
    buildPool(3, [makeEntry("乙", "yba")]),
    buildPool(4, [makeEntry("丙", "bbcc")]),
  ];

  const baseInput = () => ({
    mode: "mixed" as const,
    pools: pools(),
    progressById: new Map<string, ItemProgress>(),
    recentIds: [] as string[],
    reviewQueue: [] as { id: string; dueIn: number }[],
    mixedCursor: 0,
    rng: zeroRng,
  });

  it("mixed 模式按 2 → 3 → 4 均衡轮换", () => {
    const lengths: number[] = [];
    let input = baseInput();
    for (let i = 0; i < 6; i += 1) {
      const result = pickNext(input);
      expect(result).not.toBeNull();
      lengths.push(result!.entry.length);
      input = { ...input, mixedCursor: result!.mixedCursor };
    }
    expect(lengths).toEqual([2, 3, 4, 2, 3, 4]);
  });

  it("单长度模式只在对应池中选题", () => {
    const result = pickNext({ ...baseInput(), mode: "double" });
    expect(result?.entry.length).toBe(2);
  });

  it("到期回炉题优先,且会跳过与上一题相同的项", () => {
    const input = {
      ...baseInput(),
      recentIds: ["乙:yba"],
      reviewQueue: [
        { id: "乙:yba", dueIn: 1 }, // 推进后到期,但就是上一题 → 跳过
        { id: "丙:bbcc", dueIn: 1 }, // 到期 → 应当被选
      ],
    };
    const result = pickNext(input);
    expect(result?.entry.char).toBe("丙");
    expect(result?.reviewQueue.some((item) => item.id === "丙:bbcc")).toBe(false);
  });

  it("回炉题 dueIn 每出一题减一,到期后才出现", () => {
    // 丁 在 recentIds 中,轮换不会选它;dueIn=2 时第一题也不该是它。
    const input = {
      ...baseInput(),
      recentIds: ["丁:db"],
      reviewQueue: [{ id: "丁:db", dueIn: 2 }],
      pools: [buildPool(2, [makeEntry("甲", "ja"), makeEntry("丁", "db")])],
      mode: "double" as const,
    };
    const first = pickNext(input);
    expect(first?.entry.char).not.toBe("丁");
    expect(first?.reviewQueue).toEqual([{ id: "丁:db", dueIn: 1 }]);
    const second = pickNext({
      ...input,
      recentIds: [],
      mixedCursor: first!.mixedCursor,
      reviewQueue: first!.reviewQueue,
    });
    expect(second?.entry.char).toBe("丁");
  });

  it("空池返回 null", () => {
    const result = pickNext({
      ...baseInput(),
      mode: "double",
      pools: [buildPool(2, [])],
    });
    expect(result).toBeNull();
  });
});
