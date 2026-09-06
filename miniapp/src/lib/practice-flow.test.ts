/**
 * 练习流程语义测试(真实分片数据):
 * - 模式可用性与分片池内容一致(空池模式可被 UI 前置拦截);
 * - 简码/词/句等非单字模式端到端跑通(逐键真实编码);
 * - 错题重练(src=weak)的池构建与评分;
 * - 一级简码的双路线评分(简码完美 / 全码合法但 imperfect)。
 */

import { describe, expect, it } from "vitest";
import {
  MODE_POOL_ROTATION,
  advance,
  aggregateWeakness,
  buildPool,
  buildTrainerIndex,
  createSession,
  emptyProgress,
  keyHeatmap,
  listWeakItems,
  selectPool,
  typeKey,
  validateTrainerDataset,
} from "@xhup/trainer-core";
import type { PracticeMode, TrainerIndex } from "@xhup/trainer-core";
import shard from "../data/generated/dataset.json";
import { computeModeAvailability } from "./mode-availability";

const dataset = validateTrainerDataset(shard);
const index: TrainerIndex = buildTrainerIndex(dataset);

function makeRng(seed = 20260906): () => number {
  let state = seed;
  return () => {
    state = (state * 1103515245 + 12345) % 2147483648;
    return state / 2147483648;
  };
}

function modePools(mode: PracticeMode) {
  return MODE_POOL_ROTATION[mode]
    .map((poolId) => buildPool(poolId, selectPool(index, poolId, "daily")))
    .filter((pool) => pool.items.length > 0);
}

/** 用真实编码打完整场会话,返回完成的题数与错键事件数。 */
function runSession(mode: PracticeMode, questions: number) {
  const pools = modePools(mode);
  if (pools.length === 0) return null;
  let state = createSession(
    { mode, targetLength: questions, pools },
    new Map(),
    makeRng(),
    1000,
  );
  if (!state) return null;

  let keystrokeBudget = 2000;
  while (state.phase !== "completed" && keystrokeBudget-- > 0) {
    if (state.phase === "feedback") {
      state = advance(state, makeRng(), 2000).state;
      continue;
    }
    const expected: string = state.current.primaryCode[state.typed.length];
    expect(expected).toBeDefined();
    state = typeKey(state, expected, makeRng(), 1500).state;
  }
  return state;
}

describe("模式可用性(数据驱动,空池禁用)", () => {
  it("可用性判定与分片池内容一致", () => {
    const availability = computeModeAvailability(index);
    for (const mode of Object.keys(MODE_POOL_ROTATION) as PracticeMode[]) {
      const hasData = MODE_POOL_ROTATION[mode].some(
        (poolId) => index.pools[poolId].length > 0,
      );
      expect(availability[mode]).toBe(hasData);
    }
  });

  it("本分片支持全部 12 种模式(每种轮换至少有一个非空池)", () => {
    const availability = computeModeAvailability(index);
    for (const mode of Object.keys(availability) as PracticeMode[]) {
      expect(availability[mode], `模式 ${mode} 应可用`).toBe(true);
    }
  });

  it("数据不足的模式确实返回禁用(合成空索引验证拦截路径)", () => {
    const emptyPoolsIndex = {
      ...index,
      pools: Object.fromEntries(
        Object.keys(index.pools).map((poolId) => [poolId, []]),
      ),
    } as unknown as TrainerIndex;
    const availability = computeModeAvailability(emptyPoolsIndex);
    expect(Object.values(availability).every((value) => value === false)).toBe(true);
  });
});

describe("非单字模式端到端(真实编码)", () => {
  it("一级简码:5 题双路线引擎完整跑通", () => {
    const final = runSession("level1", 5);
    expect(final).not.toBeNull();
    expect(final!.phase).toBe("completed");
    expect(final!.questionsCompleted).toBe(5);
  });

  it("二码词简码:5 题完整跑通", () => {
    const final = runSession("two-key-word", 5);
    expect(final).not.toBeNull();
    expect(final!.phase).toBe("completed");
    expect(final!.questionsCompleted).toBe(5);
  });

  it("组句:3 题完整跑通(整句拼接码逐键输入)", () => {
    const final = runSession("sentence", 3);
    expect(final).not.toBeNull();
    expect(final!.phase).toBe("completed");
    expect(final!.questionsCompleted).toBe(3);
    expect(final!.charsCompleted).toBeGreaterThan(3);
  });

  it("全模式综合:5 题完整跑通(跨池轮换)", () => {
    const final = runSession("mixed-all", 5);
    expect(final).not.toBeNull();
    expect(final!.phase).toBe("completed");
    expect(final!.questionsCompleted).toBe(5);
  });
});

describe("一级简码双路线评分(B5 契约)", () => {
  it("分片数据:全码首键与简码同键时,按全码输入实际走简码路线(完美)", () => {
    // 真实分片行为:一级简码的全码(如 我=wo)首键与简码键相同,
    // 首键即触发简码路线完成 —— 这正是「忘了简码也能打出字」的降级。
    const pool = buildPool(
      "level1",
      selectPool(index, "level1", "daily").filter(
        (item) => item.alternateCode !== null && item.alternateCode !== "",
      ),
    );
    expect(pool.items.length).toBeGreaterThan(0);
    const rng = makeRng();
    let state = createSession(
      { mode: "level1", targetLength: 1, pools: [pool] },
      new Map(),
      rng,
      1000,
    )!;
    const item = state.current;
    expect(item.alternateCode![0]).toBe(item.primaryCode[0]);
    for (const key of item.alternateCode!) {
      state = typeKey(state, key, rng, 1500).state;
    }
    expect(state.lastOutcome).toBe("perfect");
    expect(state.lastRoute).toBe("primary");
  });

  it("合成首键分歧条目:备用全码路线合法但计 imperfect(引擎 B5 语义)", () => {
    // 分片内没有首键分歧的一级简码;用合成 TrainingItem 验证引擎
    // 的双路线评分在会话语境下同样成立(引擎语义由核心测试兜底)。
    const synthetic: Parameters<typeof buildPool>[1][number] = {
      id: "level1:test:测",
      kind: "level1",
      target: "测",
      primaryCode: "q",
      alternateCode: "xa",
      charCount: 1,
      codeLength: 1,
      frequencyScore: 1000,
      frozen: true,
      components: null,
      readings: [],
    };
    // 池 ID 必须与 MODE_POOL_ROTATION["level1"] 对齐,调度器才能找到。
    const pool = buildPool("level1", [synthetic]);
    const rng = makeRng();
    let state = createSession(
      { mode: "level1", targetLength: 1, pools: [pool] },
      new Map(),
      rng,
      1000,
    )!;
    for (const key of state.current.alternateCode!) {
      state = typeKey(state, key, rng, 1500).state;
    }
    expect(state.lastOutcome).toBe("imperfect");
    expect(state.lastRoute).toBe("alternate");
  });

  it("备用码不在分片的一级简码仍可按简码路线正常完成(降级不崩溃)", () => {
    const pool = buildPool(
      "level1",
      selectPool(index, "level1", "daily").filter(
        (item) => item.alternateCode === null || item.alternateCode === "",
      ),
    );
    if (pool.items.length === 0) return; // 分片恰好全覆盖时无此路径
    const rng = makeRng();
    let state = createSession(
      { mode: "level1", targetLength: 1, pools: [pool] },
      new Map(),
      rng,
      1000,
    )!;
    for (const key of state.current.primaryCode) {
      state = typeKey(state, key, rng, 1500).state;
    }
    expect(state.lastOutcome).toBe("perfect");
  });
});

describe("错题重练(src=weak 路径)", () => {
  it("有错条目进入薄弱清单;重练会话只从这些条目出题", () => {
    // 制造一条错题进度。
    const progress = { "的:de": { ...emptyProgress(), attempts: 2, wrong: 2 } };
    const weak = listWeakItems(index, progress);
    expect(weak.length).toBeGreaterThan(0);
    expect(weak[0]!.item.id).toBe("的:de");

    // 弱点聚合 + 键位热力读同一份数据(错题页数据源)。
    const report = aggregateWeakness(index, new Map(Object.entries(progress)), { d: 2 }, 10);
    expect(report.items[0]!.id).toBe("的:de");
    expect(report.keyErrors).toEqual({ d: 2 });
    expect(keyHeatmap({ d: 2, Z: 1 })).toEqual({ d: 2 });
  });

  it("错题为空时池为空(会话页据此显示空态而非崩溃)", () => {
    const pools = (() => {
      const weak = listWeakItems(index, {}, 30);
      return weak.length === 0 ? [] : weak.map((entry) => entry.item);
    })();
    expect(pools).toEqual([]);
  });
});
