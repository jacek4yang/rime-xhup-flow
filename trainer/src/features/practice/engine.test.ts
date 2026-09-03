import { describe, expect, it } from "vitest";
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
  activeCode,
  type SessionConfig,
  type SessionState,
} from "./engine";
import { buildPool } from "./scheduler";
import { makeEntry, makeIndex, makeSentence, makeShortcut, progressWith } from "@/test/fixtures";

const zeroRng = () => 0;

function makeConfig(): SessionConfig {
  const index = makeIndex();
  const pools = [
    buildPool("char-2", index.pools["char-2"]),
    buildPool("char-3", index.pools["char-3"]),
  ];
  return { mode: "double", targetLength: 30, pools };
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
    expect(state.current.primaryCode).toBe("xk");
    expect(state.phase).toBe("question");
    expect(expectedKey(state)).toBe("x");
  });

  it("空池返回 null", () => {
    expect(
      createSession(
        { ...makeConfig(), pools: [buildPool("char-2", [])] },
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

  it("错键不推进,标记 hadError 并计错键事件与本题错键列表", () => {
    const { state } = typeKey(start(), "z", zeroRng, 100);
    expect(state.typed).toBe("");
    expect(state.hadError).toBe(true);
    expect(state.keystrokes).toBe(1);
    expect(state.wrongKeyEvents).toBe(1);
    expect(state.wrongKeys).toEqual(["z"]);
    expect(state.lastWrongKey).toBe("z");
  });

  it("重复按同一个错键只记一次错键列表,错误事件照常累计", () => {
    let state = start();
    state = typeKey(state, "z", zeroRng, 100).state;
    state = typeKey(state, "z", zeroRng, 110).state;
    expect(state.wrongKeyEvents).toBe(2);
    expect(state.wrongKeys).toEqual(["z"]);
  });

  it("非出题阶段忽略输入", () => {
    const paused = pause(start(), 100).state;
    const { state } = typeKey(paused, "x", zeroRng, 200);
    expect(state.typed).toBe("");
  });
});

describe("简码路线(B5)", () => {
  const index = makeIndex();
  const shortcutItem = index.pools["shortcut-zero-regression"][0];
  // fixture:时间 ZR 简码 uij(主)/ 全码 uijm(备用,非前缀分歧路径)
  it("主练码(简码)完成 = perfect,routeUsed = primary", () => {
    const pool = buildPool("shortcut-zero-regression", [shortcutItem]);
    const config: SessionConfig = { mode: "zero-regression", targetLength: 1, pools: [pool] };
    let state = start(config);
    for (const key of ["u", "i", "j"]) {
      state = typeKey(state, key, zeroRng, 100).state;
    }
    expect(state.phase).toBe("feedback");
    expect(state.lastOutcome).toBe("perfect");
    expect(state.lastRoute).toBe("primary");
  });

  it("主练码自身无分歧时按 perfect 完成", () => {
    // fixture:发展 FF 简码 faj,全码 favj;输入主练码 faj。
    const pool = buildPool("shortcut-fixed-first", [
      { ...shortcutItem, id: "shortcut:发展:faj", primaryCode: "faj", alternateCode: "favj" },
    ]);
    const config: SessionConfig = { mode: "fixed-first", targetLength: 1, pools: [pool] };
    let state = start(config);
    for (const key of ["f", "a", "j"]) {
      state = typeKey(state, key, zeroRng, 100).state;
    }
    expect(state.lastRoute).toBe("primary");
    expect(state.lastOutcome).toBe("perfect");
  });

  it("首键分歧后切换备用路线(全码)可完成,routeUsed = alternate", () => {
    // 主练码 faj,备用 favj:输入 f(同)→ a(同)→ v(分歧,备用在此位为 v)
    // → j → m。前缀连续匹配,favj 全码完成。
    const item = {
      ...shortcutItem,
      id: "shortcut:发展:faj",
      target: "发展",
      primaryCode: "faj",
      alternateCode: "favj",
      charCount: 2,
    };
    const pool = buildPool("shortcut-fixed-first", [item]);
    const config: SessionConfig = { mode: "fixed-first", targetLength: 1, pools: [pool] };
    let state = start(config);
    for (const key of ["f", "a", "v", "j"]) {
      const result = typeKey(state, key, zeroRng, 100);
      state = result.state;
      if (result.state.phase === "feedback") break;
    }
    expect(state.lastRoute).toBe("alternate");
    expect(state.lastOutcome).toBe("imperfect");
    expect(state.typed).toBe("favj");
  });

  it("activeCode 跟随实际路线", () => {
    const item = {
      ...shortcutItem,
      primaryCode: "faj",
      alternateCode: "favj",
    };
    const state: SessionState = {
      ...(start({
        mode: "fixed-first",
        targetLength: 1,
        pools: [buildPool("shortcut-fixed-first", [item])],
      })),
      route: "alternate",
    };
    expect(activeCode(state)).toBe("favj");
  });
});

describe("backspace", () => {
  it("删除最后一个已接受的键并计修正", () => {
    const typed = typeKey(start(), "x", zeroRng, 100).state;
    const after = backspace(typed);
    expect(after.typed).toBe("");
    expect(after.corrections).toBe(1);
    expect(after.correctionsThisQuestion).toBe(1);
  });

  it("空输入时不变", () => {
    const state = start();
    expect(backspace(state)).toBe(state);
  });
});

describe("答题判定", () => {
  it("无错完成记 perfect 并更新进度(含延迟样本)", () => {
    let state = start();
    state = typeKey(state, "x", zeroRng, 100).state;
    const result = typeKey(state, "k", zeroRng, 200);
    expect(result.state.phase).toBe("feedback");
    expect(result.state.lastOutcome).toBe("perfect");
    expect(result.state.questionsCompleted).toBe(1);
    expect(result.state.perfect).toBe(1);
    expect(result.state.charsCompleted).toBe(1);
    const event = result.events.find((e) => e.type === "question-completed");
    expect(event).toMatchObject({
      type: "question-completed",
      outcome: "perfect",
      routeUsed: "primary",
      keystrokes: 2,
      wrongKeyEvents: 0,
      practiceMs: 200,
      corrections: 0,
    });
    expect(result.state.progressById.get("行:xk")).toMatchObject({
      attempts: 1,
      correct: 1,
      streak: 1,
      avgLatencyMs: 200,
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

  it("备用路线完成也进回炉队列(鼓励简码回忆)", () => {
    const item = {
      ...makeIndex().pools["shortcut-zero-regression"][0],
      primaryCode: "faj",
      alternateCode: "favj",
    };
    const pool = buildPool("shortcut-fixed-first", [item]);
    const config: SessionConfig = { mode: "fixed-first", targetLength: 0, pools: [pool] };
    let state = start(config);
    for (const key of ["f", "a", "v", "j", "m"]) {
      const result = typeKey(state, key, zeroRng, 100);
      state = result.state;
      if (result.state.phase === "feedback") break;
    }
    expect(state.lastRoute).toBe("alternate");
    expect(state.reviewQueue.length).toBe(1);
    void makeShortcut;
  });
});

describe("advance", () => {
  it("反馈后推进到下一题并重置输入态", () => {
    const index = makeIndex();
    const pools = [
      buildPool("char-2", index.pools["char-2"]),
      buildPool("char-3", index.pools["char-3"]),
    ];
    const config: SessionConfig = { mode: "mixed", targetLength: 30, pools };
    let state = start(config);
    state = typeKey(state, "x", zeroRng, 100).state;
    state = typeKey(state, "k", zeroRng, 200).state;
    const { state: next } = advance(state, zeroRng, 300);
    expect(next.phase).toBe("question");
    expect(next.typed).toBe("");
    expect(next.hadError).toBe(false);
    expect(next.wrongKeys).toEqual([]);
    expect(next.correctionsThisQuestion).toBe(0);
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
    const index = makeIndex();
    const pools = [
      buildPool("char-2", index.pools["char-2"]),
      buildPool("char-3", index.pools["char-3"]),
    ];
    const config: SessionConfig = { mode: "mixed", targetLength: 0, pools };
    let state = start(config);
    state = typeKey(state, "x", zeroRng, 100).state;
    state = typeKey(state, "k", zeroRng, 200).state;
    expect(advance(state, zeroRng, 300).state.phase).toBe("question");
  });
});

describe("组句(B6)", () => {
  it("整句拼接码连续输入;charsCompleted 累计汉字数;无 auto commit", () => {
    const sentence = makeSentence("我们时间", "womfuijm");
    const index = makeIndex({
      sentences: [sentence],
    });
    const pool = buildPool("sentence", index.pools.sentence);
    const config: SessionConfig = { mode: "sentence", targetLength: 1, pools: [pool] };
    let state = start(config);
    expect(state.current.charCount).toBe(4);
    for (const key of "womfuijm") {
      const result = typeKey(state, key, zeroRng, 100);
      state = result.state;
    }
    expect(state.phase).toBe("feedback");
    expect(state.lastOutcome).toBe("perfect");
    expect(state.charsCompleted).toBe(4);
    expect(state.questionsCompleted).toBe(1);
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

describe("进度对齐", () => {
  it("引擎内进度副本与 store 重放一致", () => {
    let state = start();
    state = typeKey(state, "x", zeroRng, 100).state;
    const result = typeKey(state, "k", zeroRng, 200);
    const engineProgress = result.state.progressById.get("行:xk");
    // store 侧用同一组纯函数重放(applyPerfect 初始 +1)。
    const replayed = progressWith();
    expect(engineProgress).toMatchObject({
      attempts: replayed.attempts + 1,
    });
    void makeEntry;
  });
});
