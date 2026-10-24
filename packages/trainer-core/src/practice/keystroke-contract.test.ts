import { describe, expect, it } from "vitest";
import type { ItemProgress } from "../learning/progress";
import { applyQuestionProgress, emptyProgress } from "../learning/progress";
import {
  advance,
  backspace,
  confirmQuestion,
  createSession,
  retryCurrent,
  typeKey,
  type SessionConfig,
  type SessionState,
  type SessionEvent,
} from "./engine";
import { buildPool } from "./scheduler";
import { makeIndex } from "../testing/fixtures";

const zeroRng = () => 0;

function makeConfig(): SessionConfig {
  const index = makeIndex();
  return {
    mode: "double", intent: "blind", hintMode: "on-error", targetLength: 0,
    pools: [buildPool("char-2", index.pools["char-2"])],
  };
}

function start(config = makeConfig(), progress = new Map<string, ItemProgress>()): SessionState {
  const state = createSession(config, progress, zeroRng, 0);
  if (!state) throw new Error("会话创建失败");
  return state;
}

type CompletedEvent = Extract<SessionEvent, { type: "question-completed" }>;

function completions(events: SessionEvent[]): CompletedEvent[] {
  return events.filter((event): event is CompletedEvent =>
    event.type === "question-completed");
}

/** 一级简码 w(主练码)/ wopd(备用全码)的会话配置。 */
function level1Config(): { config: SessionConfig; item: ReturnType<typeof firstLevel1> } {
  const index = makeIndex();
  const item = firstLevel1(index);
  const config: SessionConfig = {
    ...makeConfig(), mode: "level1", targetLength: 0,
    pools: [buildPool("level1", [item])],
  };
  return { config, item };
}

function firstLevel1(index: ReturnType<typeof makeIndex>) {
  const wo = index.pools["level1"].find((entry) => entry.primaryCode === "w");
  if (!wo) throw new Error("fixture 缺少一级简码 w");
  return wo;
}

describe("击键计数合同:各题事件击键数之和 = 会话接收的字母键事件数", () => {
  it("场景1 正常输入 xk:事件 2 = 会话 2", () => {
    let state = start();
    state = typeKey(state, "x", zeroRng, 10).state;
    const result = typeKey(state, "k", zeroRng, 20);
    const [event] = completions(result.events);
    expect(event.keystrokes).toBe(2);
    expect(result.state.keystrokes).toBe(2);
  });

  it("场景2 退格后重输:字母键 x,x,k = 3,不得按推导码长记 2", () => {
    let state = start();
    state = typeKey(state, "x", zeroRng, 10).state;
    state = backspace(state);
    state = typeKey(state, "x", zeroRng, 20).state;
    const result = typeKey(state, "k", zeroRng, 30);
    const [event] = completions(result.events);
    expect(event.keystrokes).toBe(3);
    expect(result.state.keystrokes).toBe(3);
  });

  it("场景3 错键后改正:字母键 z,x,k = 3,错键事件 1", () => {
    let state = start();
    state = typeKey(state, "z", zeroRng, 10).state;
    state = typeKey(state, "x", zeroRng, 20).state;
    const result = typeKey(state, "k", zeroRng, 30);
    const [event] = completions(result.events);
    expect(event.keystrokes).toBe(3);
    expect(event.wrongKeyEvents).toBe(1);
    expect(result.state.keystrokes).toBe(3);
  });

  it("场景4 重复错键:z,z,x,k = 4,错键事件 2", () => {
    let state = start();
    state = typeKey(state, "z", zeroRng, 10).state;
    state = typeKey(state, "z", zeroRng, 20).state;
    state = typeKey(state, "x", zeroRng, 30).state;
    const result = typeKey(state, "k", zeroRng, 40);
    const [event] = completions(result.events);
    expect(event.keystrokes).toBe(4);
    expect(event.wrongKeyEvents).toBe(2);
  });

  it("场景5 错键后 retryCurrent 重输:z,x,k = 3", () => {
    let state = start();
    state = typeKey(state, "z", zeroRng, 10).state;
    state = retryCurrent(state);
    state = typeKey(state, "x", zeroRng, 20).state;
    const result = typeKey(state, "k", zeroRng, 30);
    const [event] = completions(result.events);
    expect(event.keystrokes).toBe(3);
  });

  it("场景6 主练码退格后切备用全码完成:5 个字母键,事件记真实计数", () => {
    const { config } = level1Config();
    let state = start(config);
    state = typeKey(state, "w", zeroRng, 10).state;
    state = backspace(state);
    let last: SessionEvent[] = [];
    for (const key of ["w", "o", "p", "d"]) {
      const step = typeKey(state, key, zeroRng, 20);
      state = step.state;
      last = step.events;
    }
    expect(state.phase).toBe("feedback");
    expect(state.lastRoute).toBe("alternate");
    expect(state.keystrokes).toBe(5);
    const [event] = completions(last);
    expect(event.keystrokes).toBe(5);
    expect(event.routeUsed).toBe("alternate");
  });

  it("场景7 备用码半途退格回主练码并确认:w,o,退格,确认 = 2 字母键", () => {
    const { config } = level1Config();
    let state = start(config);
    state = typeKey(state, "w", zeroRng, 10).state;
    state = typeKey(state, "o", zeroRng, 20).state;
    state = backspace(state);
    const result = confirmQuestion(state, zeroRng, 30);
    const [event] = completions(result.events);
    expect(event.routeUsed).toBe("primary");
    expect(event.keystrokes).toBe(2);
  });

  it("不变量:多题连续完成时,各题事件击键数之和恒等于会话累计 keystrokes", () => {
    let state = start();
    let eventSum = 0;
    // 第 1 题:正常 xk
    state = typeKey(state, "x", zeroRng, 10).state;
    let step = typeKey(state, "k", zeroRng, 20);
    state = step.state;
    eventSum += completions(step.events).reduce((sum, e) => sum + e.keystrokes, 0);
    expect(state.keystrokes).toBe(eventSum);
    // 推进并完成后续两题(每题混入退格/错键),逐题校验不变量。
    state = advance(state, zeroRng, 30).state;
    let question = 0;
    while (question < 2) {
      expect(state.phase).toBe("question");
      // 每题注入一次错键与一次退格,其余键按主练码推进。
      state = typeKey(state, "z", zeroRng, 40).state;
      const code = state.current.primaryCode;
      state = typeKey(state, code[0]!, zeroRng, 50).state;
      state = backspace(state);
      let inner: { state: SessionState; events: SessionEvent[] } = { state, events: [] };
      for (const key of code) {
        inner = typeKey(state, key, zeroRng, 60);
        state = inner.state;
      }
      const done = completions(inner.events);
      eventSum += done.reduce((sum, e) => sum + e.keystrokes, 0);
      expect(done).toHaveLength(1);
      // 当前题完成:会话累计 = 事件之和(反馈阶段不接收字母键)。
      expect(state.keystrokes).toBe(eventSum);
      state = advance(state, zeroRng, 70).state;
      question += 1;
    }
  });
});

describe("统计四维度语义锚定(§5 六场景)", () => {
  /** 四维度:错键事件数 / 键级准确率 / 题目完成结果(outcome)/ 复习与掌握变化。 */
  type Dimension = {
    wrongKeyEvents: number;
    keyAccuracy: number | null;
    outcome: "perfect" | "imperfect";
    review: { level: number; lastResult: string };
    masteryDelta: number;
  };

  function dimensionsFor(
    evidence: {
      keystrokes: number; wrongKeyEvents: number; outcome: "perfect" | "imperfect";
      routeUsed: "primary" | "alternate"; usedAnswerHint: boolean;
      questionMs: number; chars: number; now: number;
    },
    previousMastery = 50,
  ): Dimension {
    const progress = applyQuestionProgress(
      { ...emptyProgress(), mastery: previousMastery, attempts: 1 },
      {
        outcome: evidence.outcome, routeUsed: evidence.routeUsed,
        independentRecall: evidence.outcome === "perfect",
        usedAnswerHint: evidence.usedAnswerHint,
        wrongKeyEvents: evidence.wrongKeyEvents,
        questionMs: evidence.questionMs, chars: evidence.chars,
        now: evidence.now, keystrokes: evidence.keystrokes,
      },
    );
    const keyAccuracy = evidence.keystrokes > 0
      ? (evidence.keystrokes - evidence.wrongKeyEvents) / evidence.keystrokes
      : null;
    return {
      wrongKeyEvents: evidence.wrongKeyEvents,
      keyAccuracy,
      outcome: evidence.outcome,
      review: { level: progress.review!.level, lastResult: progress.review!.lastResult },
      masteryDelta: progress.mastery - previousMastery,
    };
  }

  it("场景A 备用码完成,零错键:键级准确率 100%,但完成结果 imperfect、复习归零(B5 合同)", () => {
    const d = dimensionsFor({
      keystrokes: 4, wrongKeyEvents: 0, outcome: "imperfect",
      routeUsed: "alternate", usedAnswerHint: false,
      questionMs: 1000, chars: 1, now: 1000,
    });
    expect(d).toMatchObject({
      wrongKeyEvents: 0, keyAccuracy: 1, outcome: "imperfect",
      review: { level: 0, lastResult: "alternate" },
    });
    expect(d.masteryDelta).toBe(-15);
  });

  it("场景B 看提示后正确输入,零错键:键级准确率 100%,完成结果 imperfect、复习归零", () => {
    const d = dimensionsFor({
      keystrokes: 2, wrongKeyEvents: 0, outcome: "imperfect",
      routeUsed: "primary", usedAnswerHint: true,
      questionMs: 1000, chars: 1, now: 1000,
    });
    expect(d).toMatchObject({
      wrongKeyEvents: 0, keyAccuracy: 1, outcome: "imperfect",
      review: { level: 0, lastResult: "hinted" },
    });
  });

  it("场景C 按错一次后完成:键级准确率 <100%,imperfect,复习归零", () => {
    const d = dimensionsFor({
      keystrokes: 3, wrongKeyEvents: 1, outcome: "imperfect",
      routeUsed: "primary", usedAnswerHint: false,
      questionMs: 1000, chars: 1, now: 1000,
    });
    expect(d.keyAccuracy).toBeCloseTo(2 / 3);
    expect(d.review).toEqual({ level: 0, lastResult: "error" });
  });

  it("场景D 慢但正确(8s/字以内):perfect,复习按到期规则升级", () => {
    const d = dimensionsFor({
      keystrokes: 2, wrongKeyEvents: 0, outcome: "perfect",
      routeUsed: "primary", usedAnswerHint: false,
      questionMs: 7000, chars: 1, now: 1000,
    });
    expect(d).toMatchObject({
      keyAccuracy: 1, outcome: "perfect",
      review: { level: 1, lastResult: "recalled" },
    });
    expect(d.masteryDelta).toBeGreaterThan(0);
  });

  it("场景E 提前复习正确:到期前重复不升级、不推迟(等级与到期时间不变)", () => {
    const first = applyQuestionProgress(emptyProgress(), {
      outcome: "perfect", routeUsed: "primary", independentRecall: true,
      usedAnswerHint: false, wrongKeyEvents: 0,
      questionMs: 1000, chars: 1, now: 1000, keystrokes: 2,
    });
    const dueAt = first.review!.dueAt;
    const early = applyQuestionProgress(first, {
      outcome: "perfect", routeUsed: "primary", independentRecall: true,
      usedAnswerHint: false, wrongKeyEvents: 0,
      questionMs: 1000, chars: 1, now: dueAt - 1, keystrokes: 2,
    });
    expect(early.review).toMatchObject({ level: 1, dueAt });
  });

  it("场景F 完全独立回忆且到期:perfect,等级 +1,新间隔取下一档", () => {
    const first = applyQuestionProgress(emptyProgress(), {
      outcome: "perfect", routeUsed: "primary", independentRecall: true,
      usedAnswerHint: false, wrongKeyEvents: 0,
      questionMs: 1000, chars: 1, now: 1000, keystrokes: 2,
    });
    const dueAt = first.review!.dueAt;
    const second = applyQuestionProgress(first, {
      outcome: "perfect", routeUsed: "primary", independentRecall: true,
      usedAnswerHint: false, wrongKeyEvents: 0,
      questionMs: 1000, chars: 1, now: dueAt, keystrokes: 2,
    });
    expect(second.review).toMatchObject({ level: 2, lastResult: "recalled" });
    expect(second.review!.dueAt).toBe(dueAt + 3 * 86_400_000);
  });
});

describe("会话进度与 Store 重放 parity(P0-2)", () => {
  it("同一事件序列下,引擎 progressById 与宿主重放严格一致(含 accuracyEwma)", () => {
    const { config, item: level1Item } = level1Config();
    let state = start();
    const replay: Record<string, ItemProgress> = {};
    let now = 100;

    /** 宿主路径:完成事件到达时立即记账(与引擎完成共用同一时间戳)。 */
    const applyEvent = (event: CompletedEvent, completedAt: number) => {
      const previous = replay[event.item.id] ?? emptyProgress();
      replay[event.item.id] = applyQuestionProgress(previous, {
        outcome: event.outcome,
        routeUsed: event.routeUsed,
        independentRecall: event.independentRecall,
        usedAnswerHint: event.usedAnswerHint,
        wrongKeyEvents: event.wrongKeyEvents,
        questionMs: event.questionMs,
        chars: event.item.charCount,
        now: completedAt,
        keystrokes: event.keystrokes,
      });
    };

    // 第 1 题:完美 primary
    state = typeKey(state, "x", zeroRng, now++).state;
    let step = typeKey(state, "k", zeroRng, now);
    const t1 = now++;
    state = step.state;
    completions(step.events).forEach((event) => applyEvent(event, t1));
    state = advance(state, zeroRng, now++).state;

    // 第 2 题:错键后退格改正
    state = typeKey(state, "z", zeroRng, now++).state;
    state = typeKey(state, state.current.primaryCode[0]!, zeroRng, now++).state;
    state = backspace(state);
    const code2 = state.current.primaryCode;
    step = { state, events: [] as SessionEvent[] } as { state: SessionState; events: SessionEvent[] };
    for (let i = 0; i < code2.length; i += 1) {
      step = typeKey(state, code2[i]!, zeroRng, now++);
      state = step.state;
    }
    const t2 = now - 1;
    completions(step.events).forEach((event) => applyEvent(event, t2));
    state = advance(state, zeroRng, now++).state;

    // 第 3 题:切到一级简码会话,备用全码完成(alternate 路线)
    let alt = start(config);
    alt = typeKey(alt, "w", zeroRng, now++).state;
    alt = backspace(alt);
    let altStep: { state: SessionState; events: SessionEvent[] } = { state: alt, events: [] };
    for (const key of ["w", "o", "p", "d"]) {
      altStep = typeKey(alt, key, zeroRng, now++);
      alt = altStep.state;
    }
    const t3 = now - 1;
    completions(altStep.events).forEach((event) => applyEvent(event, t3));

    // 宿主重放表中的每个条目必须与引擎 progressById 完全相等。
    for (const [id, progress] of Object.entries(replay)) {
      const engineCopy = id === level1Item.id ? alt.progressById.get(id) : state.progressById.get(id);
      expect(engineCopy).toBeDefined();
      expect(engineCopy).toEqual(progress);
    }
  });
});
