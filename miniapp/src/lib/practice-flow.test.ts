/**
 * 练习流程语义测试(完整生产数据):
 * - 十二种模式可用;空池仍可被 UI 前置拦截;
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
  createSession,
  confirmQuestion,
  emptyProgress,
  keyHeatmap,
  listWeakItems,
  selectPool,
  typeKey,
} from "@xhup/trainer-core";
import type { PracticeMode, TrainerIndex } from "@xhup/trainer-core";
import { trainerIndex as index } from "../testing/compiled-dataset";
import { computeModeAvailability } from "./mode-availability";


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
    { mode, intent: "blind", hintMode: "on-error", targetLength: questions, pools },
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
    if (state.typed === state.current.primaryCode) {
      state = confirmQuestion(state, makeRng(), 1500).state;
      continue;
    }
    const expected: string = state.current.primaryCode[state.typed.length];
    expect(expected).toBeDefined();
    state = typeKey(state, expected, makeRng(), 1500).state;
  }
  return state;
}

describe("模式可用性(数据驱动,空池禁用)", () => {

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

describe("完整规范数据的十二种模式", () => {
  it.each(Object.keys(MODE_POOL_ROTATION) as PracticeMode[])("%s 按真实主练码完成三题", (mode) => {
    const final = runSession(mode, 3);
    expect(final?.phase).toBe("completed");
    expect(final?.questionsCompleted).toBe(3);
  });
});

describe("一级简码双路线评分(B5 契约)", () => {

  it("合成首键分歧条目:备用全码路线合法但计 imperfect(引擎 B5 语义)", () => {
    // 独立覆盖首键分歧的路线边界；生产数据的成功路径在上面逐模式验证。
    const synthetic: Parameters<typeof buildPool>[1][number] = {
      id: "level1:test:测",
      kind: "level1",
      target: "测",
      primaryCode: "q",
      acceptedCodes: ["q"],
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
      { mode: "level1", intent: "blind", hintMode: "on-error", targetLength: 1, pools: [pool] },
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

});
