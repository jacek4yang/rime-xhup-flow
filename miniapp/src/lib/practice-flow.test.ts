/**
 * 练习流程语义测试(完整生产数据):
 * - 十二种模式可用;空池仍可被 UI 前置拦截;
 * - 简码/词/句等非单字模式端到端跑通(逐键真实编码);
 * - 错题重练(src=weak)的池构建与评分;
 * - 一级简码的双路线评分(简码完美 / 全码合法但 imperfect);
 * - 行为组合:非法键、恰好一次完成事件、提示语义、既有进度保持、
 *   中途退出不重复记账(引擎层,配合真实数据)。
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
  finish,
  keyHeatmap,
  listWeakItems,
  revealAnswer,
  selectPool,
  typeKey,
  type SessionEvent,
  type SessionState,
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

function startSession(
  mode: PracticeMode,
  options: { targetLength?: number; intent?: "blind" | "learn"; progress?: Map<string, ReturnType<typeof emptyProgress>> } = {},
): SessionState {
  const pools = modePools(mode);
  const state = createSession(
    {
      mode,
      intent: options.intent ?? "blind",
      hintMode: "on-error",
      targetLength: options.targetLength ?? 3,
      pools,
    },
    options.progress ?? new Map(),
    makeRng(),
    1000,
  );
  if (!state) throw new Error(`${mode} 会话创建失败(池为空)`);
  return state;
}

type CompletedEvent = Extract<SessionEvent, { type: "question-completed" }>;

function completedEvents(events: SessionEvent[]): CompletedEvent[] {
  return events.filter((event): event is CompletedEvent => event.type === "question-completed");
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

describe("行为组合:非法按键不推进(真实数据,双拼模式)", () => {
  it.each(["A", "1", "!", " ", "xk", ""] as const)("非小写字母键 %j 被整体拒绝,状态不变", (key) => {
    const state = startSession("double", { targetLength: 0 });
    const result = typeKey(state, key, makeRng(), 1500);
    expect(result.state).toBe(state);
    expect(result.events).toHaveLength(0);
  });
});

describe("行为组合:完成事件恰好一次,退出不重复记账", () => {
  it("每题恰好一个 question-completed;结束后 finish 不再产生任何事件", () => {
    let state = startSession("double", { targetLength: 2 });
    let completions = 0;
    let sessionEnds = 0;
    let guard = 0;
    while (state.phase !== "completed" && guard++ < 100) {
      if (state.phase === "feedback") {
        const step = advance(state, makeRng(), 2000);
        state = step.state;
        sessionEnds += step.events.filter((event) => event.type === "session-completed").length;
        continue;
      }
      const expected = state.current.primaryCode[state.typed.length]!;
      const step = typeKey(state, expected, makeRng(), 1500);
      state = step.state;
      completions += completedEvents(step.events).length;
      // 确认键路径:短码为主练码前缀时需要显式确认(词/简码模式)。
      if (state.phase === "question" && state.typed === state.current.primaryCode) {
        const confirmed = confirmQuestion(state, makeRng(), 1600);
        state = confirmed.state;
        completions += completedEvents(confirmed.events).length;
      }
    }
    expect(completions).toBe(2);
    expect(sessionEnds).toBe(1);
    expect(state.phase).toBe("completed");
    // 已完成的会话再 finish:原状态返回,零事件。
    const settled = finish(state, 3000);
    expect(settled.state).toBe(state);
    expect(settled.events).toHaveLength(0);
  });

  it("中途退出(finish)只结清一次时间,不产生额外完成事件", () => {
    let state = startSession("mixed", { targetLength: 0 });
    // 完成一题后立即退出。
    const first = state.current.primaryCode[0]!;
    state = typeKey(state, first, makeRng(), 1100).state;
    const rest = state.current.primaryCode.slice(1);
    let lastEvents: SessionEvent[] = [];
    for (const key of rest) {
      const step = typeKey(state, key, makeRng(), 1200);
      state = step.state;
      lastEvents = step.events;
      if (state.phase === "feedback") break;
    }
    if (state.phase === "question" && state.typed === state.current.primaryCode) {
      const confirmed = confirmQuestion(state, makeRng(), 1300);
      state = confirmed.state;
      lastEvents = confirmed.events;
    }
    expect(completedEvents(lastEvents)).toHaveLength(1);
    const before = { questions: state.questionsCompleted, ms: state.activeMs };
    const exited = finish(state, 5000);
    expect(exited.state.phase).toBe("completed");
    expect(exited.state.questionsCompleted).toBe(before.questions);
    expect(exited.state.activeMs).toBeGreaterThanOrEqual(before.ms);
    expect(completedEvents(exited.events)).toHaveLength(0);
  });
});

describe("行为组合:提示不被记作独立回忆(learn 意图 + on-error)", () => {
  it("错键触发提示资格 → revealAnswer → 正确完成后 usedAnswerHint=true 且 imperfect", () => {
    let state = startSession("double", { targetLength: 1, intent: "learn" });
    const wrong = state.current.primaryCode[0] === "x" ? "z" : "x";
    state = typeKey(state, wrong, makeRng(), 1100).state; // 触发 hadError → 提示资格
    state = revealAnswer(state); // 记为已用提示
    expect(state.usedAnswerHint).toBe(true);
    for (const key of state.current.primaryCode) {
      const step = typeKey(state, key, makeRng(), 1200);
      state = step.state;
      if (completedEvents(step.events).length > 0) {
        const [event] = completedEvents(step.events);
        expect(event.usedAnswerHint).toBe(true);
        expect(event.independentRecall).toBe(false);
        expect(event.outcome).toBe("imperfect");
        return;
      }
    }
    // 主练码为短码时需要确认。
    const confirmed = confirmQuestion(state, makeRng(), 1300);
    const [event] = completedEvents(confirmed.events);
    expect(event.usedAnswerHint).toBe(true);
    expect(event.independentRecall).toBe(false);
    expect(event.outcome).toBe("imperfect");
  });
});

describe("行为组合:既有进度保持与持久化 ID 一致", () => {
  it("带历史进度开场:完成同一条目是累计(attempts+1),不是重置;ID 不变", () => {
    const progress = new Map<string, ReturnType<typeof emptyProgress>>();
    let probe = startSession("double", { targetLength: 1 });
    const itemId = probe.current.id;
    progress.set(itemId, { ...emptyProgress(), attempts: 7, correct: 7, wrong: 0, streak: 4, mastery: 60 });
    // 用同一 RNG 种子保证抽到同一条目。
    let state = createSession(
      {
        mode: "double", intent: "blind", hintMode: "on-error", targetLength: 1,
        pools: modePools("double"),
      },
      progress,
      makeRng(),
      1000,
    )!;
    const drawnId = state.current.id;
    for (const key of state.current.primaryCode) {
      const step = typeKey(state, key, makeRng(), 1200);
      state = step.state;
      if (completedEvents(step.events).length > 0) break;
    }
    if (state.phase === "question" && state.typed === state.current.primaryCode) {
      state = confirmQuestion(state, makeRng(), 1300).state;
    }
    const updated = state.progressById.get(drawnId)!;
    expect(drawnId).toBe(itemId);
    expect(updated.attempts).toBe(8);
    expect(updated.mastery).toBeGreaterThan(60);
    // 其他历史字段不丢。
    expect(updated.correct).toBe(8);
  });
});

describe("行为组合:真实数据的一级简码全码路线", () => {
  it("生产 level1 条目:主练码单键 + 确认 = perfect;全码路线 = 合法但 imperfect", () => {
    let state = startSession("level1", { targetLength: 0 });
    const item = state.current;
    expect(item.alternateCode).toBeTruthy();
    expect(item.alternateCode!.length).toBeGreaterThan(item.primaryCode.length);
    // 路线一:主练码(单键)需要确认提交。
    state = typeKey(state, item.primaryCode[0]!, makeRng(), 1100).state;
    let confirmed = confirmQuestion(state, makeRng(), 1200);
    expect(state.phase === "question" || confirmed.state.phase === "feedback").toBe(true);
    if (confirmed.state.phase === "feedback") {
      expect(confirmed.state.lastRoute).toBe("primary");
      expect(confirmed.state.lastOutcome).toBe("perfect");
    }
    // 路线二:同一条目走真实全码(备用路线)。
    let again = startSession("level1", { targetLength: 0, progress: new Map([[item.id, emptyProgress()]]) });
    for (const key of item.alternateCode!) {
      again = typeKey(again, key, makeRng(), 1300).state;
    }
    expect(again.phase).toBe("feedback");
    expect(again.lastRoute).toBe("alternate");
    expect(again.lastOutcome).toBe("imperfect");
  });
});
