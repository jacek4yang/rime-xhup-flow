import { describe, expect, it } from "vitest";
import { emptyProgress, type ItemProgress } from "@/lib/progress";
import { charItem, wordItem } from "@/lib/trainer-index";
import { makeEntry, makeWord, makeShortcut, progressWith, seededRng } from "@/test/fixtures";
import {
  buildPool,
  computePriority,
  latencyBoost,
  LATENCY_REF_MS_PER_CHAR,
  pickNext,
  pickWeighted,
  scheduleReview,
  type QuestionPool,
} from "./scheduler";

function progressOf(overrides: Partial<ItemProgress>): ItemProgress {
  return { ...emptyProgress(), ...overrides };
}

const zeroRng = () => 0;

describe("computePriority", () => {
  it("词频更高优先级更高", () => {
    const low = charItem(makeEntry("甲", "ja", 10));
    const high = charItem(makeEntry("乙", "yb", 10_000));
    const pool = buildPool("char-2", [low, high]);
    const p = emptyProgress();
    expect(computePriority(high, p, pool.maxLogFrequency)).toBeGreaterThan(
      computePriority(low, p, pool.maxLogFrequency),
    );
  });

  it("全零词频池 maxLog 为 0 时频率增益取 0.75", () => {
    const item = charItem(makeEntry("甲", "ja", 0));
    expect(computePriority(item, emptyProgress(), 0)).toBeCloseTo(
      0.75 * (1 + 100 / 20) * 1.4,
    );
  });

  it("薄弱度随错误与低掌握度上升", () => {
    const item = charItem(makeEntry("甲", "ja", 0));
    const weak = progressOf({ attempts: 2, wrong: 2, mastery: 20 });
    const strong = progressOf({ attempts: 2, correct: 2, mastery: 90 });
    expect(computePriority(item, weak, 0)).toBeGreaterThan(
      computePriority(item, strong, 0),
    );
  });

  it("未见条目有 1.4 倍增益", () => {
    const item = charItem(makeEntry("甲", "ja", 0));
    const unseen = computePriority(item, emptyProgress(), 0);
    const seen = computePriority(
      item,
      progressOf({ attempts: 1, correct: 1, mastery: 4 }),
      0,
    );
    expect(unseen / seen).toBeGreaterThan(1.4);
  });

  it("迟疑增益:平均耗时越慢(按汉字归一)优先级越高,封顶 1.5", () => {
    const item = wordItem(makeWord("我们时间", "womfuijm", 0));
    const fast = progressOf({ attempts: 1, correct: 1, avgLatencyMs: 100 });
    const slow = progressOf({
      attempts: 1,
      correct: 1,
      avgLatencyMs: LATENCY_REF_MS_PER_CHAR * item.charCount * 10,
    });
    expect(latencyBoost(slow, item.charCount)).toBe(1.5);
    expect(computePriority(item, slow, 0)).toBeGreaterThan(
      computePriority(item, fast, 0),
    );
  });

  it("无延迟样本时迟疑增益为 1", () => {
    expect(latencyBoost(progressWith({ avgLatencyMs: null }), 1)).toBe(1);
    expect(latencyBoost(emptyProgress(), 1)).toBe(1);
  });

  it("池级层权重进入优先级(基础层可略优先)", () => {
    const item = charItem(makeEntry("甲", "ja", 0));
    const base = computePriority(item, emptyProgress(), 0, 1);
    const boosted = computePriority(item, emptyProgress(), 0, 1.2);
    expect(boosted).toBeCloseTo(base * 1.2);
  });
});

describe("pickWeighted", () => {
  const pool = (): QuestionPool =>
    buildPool("char-2", [
      charItem(makeEntry("甲", "ja", 10)),
      charItem(makeEntry("乙", "yb", 10_000)),
    ]);

  it("从池中选题", () => {
    const result = pickWeighted(pool(), new Map(), new Set(), zeroRng);
    expect(result).not.toBeNull();
    expect(pool().items).toContainEqual(result);
  });

  it("有备选时不立即重复最近条目", () => {
    const selected = pickWeighted(
      pool(),
      new Map(),
      new Set(["甲:ja"]),
      zeroRng,
    );
    expect(selected?.target).toBe("乙");
  });

  it("单条目池可以工作", () => {
    const single = buildPool("char-2", [charItem(makeEntry("甲", "ja"))]);
    const selected = pickWeighted(single, new Map(), new Set(["甲:ja"]), zeroRng);
    expect(selected?.target).toBe("甲");
  });

  it("权重高的条目覆盖更大的随机区间", () => {
    const p = pool();
    const progress = new Map<string, ItemProgress>();
    const pLow = computePriority(p.items[0]!, emptyProgress(), p.maxLogFrequency);
    const pHigh = computePriority(p.items[1]!, emptyProgress(), p.maxLogFrequency);
    // rng 落在低频条目区间内 → 选第一个;超过则选第二个。
    const inLow = pickWeighted(p, progress, new Set(), () => (pLow * 0.5) / (pLow + pHigh));
    const pastLow = pickWeighted(p, progress, new Set(), () => (pLow + pHigh * 0.5) / (pLow + pHigh));
    expect(inLow?.target).toBe("甲");
    expect(pastLow?.target).toBe("乙");
  });

  it("简码池携带层权重;同 RNG 序列可复现", () => {
    const items = [
      { ...charItem(makeEntry("甲", "ja")), primaryCode: "j", alternateCode: "ja" },
    ];
    const a = buildPool("level1", items, 1.2);
    const b = buildPool("level1", items, 1.2);
    expect(pickWeighted(a, new Map(), new Set(), seededRng(7))).toEqual(
      pickWeighted(b, new Map(), new Set(), seededRng(7)),
    );
    expect(a.layerWeight).toBe(1.2);
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
    buildPool("char-2", [charItem(makeEntry("甲", "ja"))]),
    buildPool("char-3", [charItem(makeEntry("乙", "yba"))]),
    buildPool("char-4", [charItem(makeEntry("丙", "bbcc"))]),
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
      lengths.push(result!.item.codeLength);
      input = { ...input, mixedCursor: result!.mixedCursor };
    }
    expect(lengths).toEqual([2, 3, 4, 2, 3, 4]);
  });

  it("简码层模式在对应池中选题", () => {
    const shortcutPools = [
      buildPool("shortcut-zero-regression", [
        { ...charItem(makeEntry("时间", "uijm")), id: "shortcut:时间:uij", primaryCode: "uij" },
      ]),
    ];
    const result = pickNext({ ...baseInput(), mode: "zero-regression", pools: shortcutPools });
    expect(result?.item.kind).toBe("char");
    expect(result?.item.primaryCode).toBe("uij");
    void makeShortcut;
  });

  it("组句模式在 sentence 池中选题", () => {
    const sentencePools = [
      buildPool("sentence", [
        { ...charItem(makeEntry("甲", "ja")), kind: "sentence" as const, charCount: 2 },
      ]),
    ];
    const result = pickNext({ ...baseInput(), mode: "sentence", pools: sentencePools });
    expect(result).not.toBeNull();
  });

  it("单池模式只在对应池中选题", () => {
    const result = pickNext({ ...baseInput(), mode: "double" });
    expect(result?.item.codeLength).toBe(2);
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
    expect(result?.item.target).toBe("丙");
    expect(result?.reviewQueue.some((item) => item.id === "丙:bbcc")).toBe(false);
  });

  it("回炉题 dueIn 每出一题减一,到期后才出现", () => {
    // 丁 在 recentIds 中,轮换不会选它;dueIn=2 时第一题也不该是它。
    const input = {
      ...baseInput(),
      recentIds: ["丁:db"],
      reviewQueue: [{ id: "丁:db", dueIn: 2 }],
      pools: [buildPool("char-2", [charItem(makeEntry("甲", "ja")), charItem(makeEntry("丁", "db"))])],
      mode: "double" as const,
    };
    const first = pickNext(input);
    expect(first?.item.target).not.toBe("丁");
    expect(first?.reviewQueue).toEqual([{ id: "丁:db", dueIn: 1 }]);
    const second = pickNext({
      ...input,
      recentIds: [],
      mixedCursor: first!.mixedCursor,
      reviewQueue: first!.reviewQueue,
    });
    expect(second?.item.target).toBe("丁");
  });

  it("空池返回 null", () => {
    const result = pickNext({
      ...baseInput(),
      mode: "double",
      pools: [buildPool("char-2", [])],
    });
    expect(result).toBeNull();
  });

  it("同模式同进度下选题序列可复现(注入 RNG)", () => {
    const run = (): string[] => {
      const input = {
        ...baseInput(),
        pools: [
          buildPool("char-2", [
            charItem(makeEntry("甲", "ja", 10)),
            charItem(makeEntry("乙", "yb", 100)),
            charItem(makeEntry("丙", "bb", 1000)),
          ]),
        ],
        mode: "double" as const,
        rng: seededRng(42),
      };
      const picked: string[] = [];
      let cursor = 0;
      let recent: string[] = [];
      for (let i = 0; i < 5; i += 1) {
        const result = pickNext({ ...input, mixedCursor: cursor, recentIds: recent });
        if (!result) break;
        picked.push(result.item.id);
        cursor = result.mixedCursor;
        recent = [...recent, result.item.id].slice(-5);
      }
      return picked;
    };
    expect(run()).toEqual(run());
  });
});
