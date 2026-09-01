import { describe, expect, it } from "vitest";
import type { TrainerEntry } from "@/lib/trainer-data";
import type { ItemProgress } from "@/lib/progress";
import {
  advance,
  backspace,
  createSession,
  expectedKey,
  finish,
  pause,
  resume,
  typeKey,
  type SessionConfig,
  type SessionState,
} from "./engine";
import { buildPool } from "./scheduler";

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

const zeroRng = () => 0;

function makeConfig(entries: TrainerEntry[] = [makeEntry("行", "xk")]): SessionConfig {
  const byLength = new Map<number, TrainerEntry[]>();
  for (const entry of entries) {
    const list = byLength.get(entry.length) ?? [];
    list.push(entry);
    byLength.set(entry.length, list);
  }
  const pools = [...byLength.entries()].map(([length, list]) =>
    buildPool(length as 2 | 3 | 4, list),
  );
  return { mode: "double", difficulty: "daily", targetLength: 30, pools };
}

function start(
  config = makeConfig(),
  progress = new Map<string, ItemProgress>(),
  now = 0,
): SessionState {
  const state = createSession(config, progress, zeroRng, now);
  if (!state) throw new Error("会话创建失败");
  return state;
}

describe("createSession", () => {
  it("立即抽出第一题", () => {
    const state = start();
    expect(state.current.code).toBe("xk");
    expect(state.phase).toBe("question");
    expect(expectedKey(state)).toBe("x");
  });

  it("空池返回 null", () => {
    expect(
      createSession(
        { ...makeConfig(), pools: [buildPool(2, [])] },
        new Map(),
        zeroRng,
        0,
      ),
    ).toBeNull();
  });
});

describe("typeKey", () => {
  it("正确的键推进码位", () => {
    const { state } = typeKey(start(), "x", zeroRng, 100);
    expect(state.typed).toBe("x");
    expect(expectedKey(state)).toBe("k");
  });

  it("错键不推进,标记 hadError 并计错键事件", () => {
    const { state } = typeKey(start(), "z", zeroRng, 100);
    expect(state.typed).toBe("");
    expect(state.hadError).toBe(true);
    expect(state.keystrokes).toBe(1);
    expect(state.wrongKeyEvents).toBe(1);
    expect(state.lastWrongKey).toBe("z");
  });

  it("非出题阶段忽略输入", () => {
    const paused = pause(start(), 100).state;
    const { state } = typeKey(paused, "x", zeroRng, 200);
    expect(state.typed).toBe("");
  });
});

describe("backspace", () => {
  it("删除最后一个已接受的键", () => {
    const typed = typeKey(start(), "x", zeroRng, 100).state;
    expect(backspace(typed).typed).toBe("");
  });

  it("空输入时不变", () => {
    const state = start();
    expect(backspace(state)).toBe(state);
  });
});

describe("答题判定", () => {
  it("无错完成记 perfect 并更新进度", () => {
    let state = start();
    state = typeKey(state, "x", zeroRng, 100).state;
    const result = typeKey(state, "k", zeroRng, 200);
    expect(result.state.phase).toBe("feedback");
    expect(result.state.lastOutcome).toBe("perfect");
    expect(result.state.questionsCompleted).toBe(1);
    expect(result.state.perfect).toBe(1);
    const event = result.events.find((e) => e.type === "question-completed");
    expect(event).toMatchObject({
      type: "question-completed",
      outcome: "perfect",
      keystrokes: 2,
      wrongKeyEvents: 0,
      practiceMs: 200,
    });
    expect(result.state.progressById.get("行:xk")).toMatchObject({
      attempts: 1,
      correct: 1,
      streak: 1,
    });
  });

  it("有错完成记 imperfect,计入回炉队列,一条错键只算一道错题", () => {
    let state = start();
    state = typeKey(state, "z", zeroRng, 100).state;
    state = typeKey(state, "z", zeroRng, 110).state;
    state = typeKey(state, "x", zeroRng, 120).state;
    const result = typeKey(state, "k", zeroRng, 200);
    expect(result.state.lastOutcome).toBe("imperfect");
    expect(result.state.imperfect).toBe(1);
    expect(result.state.wrongKeyEvents).toBe(2);
    expect(result.state.reviewQueue).toEqual([{ id: "行:xk", dueIn: 3 }]);
    expect(result.state.progressById.get("行:xk")).toMatchObject({
      attempts: 1,
      wrong: 1,
      streak: 0,
    });
  });
});

describe("advance", () => {
  it("反馈后推进到下一题并重置输入态", () => {
    const config = makeConfig([makeEntry("行", "xk"), makeEntry("好", "hk")]);
    let state = start(config);
    state = typeKey(state, "x", zeroRng, 100).state;
    state = typeKey(state, "k", zeroRng, 200).state;
    const { state: next } = advance(state, zeroRng, 300);
    expect(next.phase).toBe("question");
    expect(next.typed).toBe("");
    expect(next.hadError).toBe(false);
    expect(next.questionsCompleted).toBe(1);
  });

  it("达到目标题数后结束会话", () => {
    const config = { ...makeConfig(), targetLength: 1 as const };
    let state = start(config);
    state = typeKey(state, "x", zeroRng, 100).state;
    state = typeKey(state, "k", zeroRng, 200).state;
    const result = advance(state, zeroRng, 300);
    expect(result.state.phase).toBe("completed");
    expect(result.events.some((e) => e.type === "session-completed")).toBe(true);
  });

  it("无限模式不自动结束", () => {
    const config = {
      ...makeConfig([makeEntry("行", "xk"), makeEntry("好", "hk")]),
      targetLength: 0 as const,
    };
    let state = start(config);
    state = typeKey(state, "x", zeroRng, 100).state;
    state = typeKey(state, "k", zeroRng, 200).state;
    expect(advance(state, zeroRng, 300).state.phase).toBe("question");
  });
});

describe("计时", () => {
  it("暂停时间不计入活跃时长", () => {
    let state = start();
    state = typeKey(state, "x", zeroRng, 100).state;
    const paused = pause(state, 200);
    expect(paused.state.activeMs).toBe(200);
    const resumed = resume(paused.state, 1000);
    const finished = finish(resumed, 1300);
    expect(finished.state.activeMs).toBe(500); // 200 + (1300 - 1000)
    const flushed = finished.events
      .filter((e) => e.type === "time-flushed")
      .reduce((sum, e) => sum + (e.type === "time-flushed" ? e.practiceMs : 0), 0);
    expect(flushed).toBe(300);
  });

  it("反馈阶段不可暂停(避免出现已完成但无法推进的状态)", () => {
    let state = start();
    state = typeKey(state, "x", zeroRng, 100).state;
    state = typeKey(state, "k", zeroRng, 200).state;
    expect(state.phase).toBe("feedback");
    const result = pause(state, 300);
    expect(result.state.phase).toBe("feedback");
    expect(result.events).toEqual([]);
  });

  it("Escape 暂停后 resume 继续", () => {
    const state = start();
    const paused = pause(state, 50).state;
    expect(paused.phase).toBe("paused");
    expect(resume(paused, 60).phase).toBe("question");
  });
});
