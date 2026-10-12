import { describe, expect, it } from "vitest";
import { advance, createSession, pause, resume, typeKey } from "./engine";
import { hintAccess, questionElapsedMs, revealAnswer, tickQuestion } from "./hints";
import { buildPool } from "./scheduler";
import { makeIndex } from "../testing/fixtures";
import type { HintMode, PracticeIntent } from "./types";

const rng = () => 0;
function start(intent: PracticeIntent, hintMode: HintMode) {
  return createSession({ intent, hintMode, mode: "double", targetLength: 0,
    pools: [buildPool("char-2", makeIndex().pools["char-2"])] }, new Map(), rng, 0)!;
}

describe("答案授权与本题活跃时钟", () => {
  it.each<HintMode>(["always", "on-delay", "on-error", "hidden"])(
    "blind/%s 等待和错键均不自动揭示", (mode) => {
      let state = start("blind", mode);
      state = typeKey(state, "z", rng, 3000).state;
      state = tickQuestion(state, 9000);
      expect(hintAccess(state, 9000).revealAnswer).toBe(false);
      expect(state.usedAnswerHint).toBe(false);
      state = pause(state, 9000).state;
      expect(hintAccess(state, 60000).revealAnswer).toBe(false);
      expect(hintAccess(revealAnswer(state), 60000).revealAnswer).toBe(true);
    },
  );

  it("暂停不累计，题间重置，逐键结算不能漏记同帧自动提示", () => {
    let state = start("learn", "on-delay");
    state = pause(state, 1000).state;
    expect(questionElapsedMs(state, 61000)).toBe(1000);
    state = resume(state, 61000);
    expect(hintAccess(state, 61999).revealAnswer).toBe(false);
    state = typeKey(state, "x", rng, 62000).state;
    expect(state.usedAnswerHint).toBe(true);
    const result = typeKey(state, "k", rng, 62100);
    expect(result.events[0]).toMatchObject({ type: "question-completed",
      questionMs: 2100, practiceMs: 1100, independentRecall: false,
      usedAnswerHint: true, outcome: "imperfect" });
    state = advance(result.state, rng, 63000).state;
    expect(questionElapsedMs(state, 64999)).toBe(1999);
    expect(hintAccess(state, 64999).revealAnswer).toBe(false);
    expect(hintAccess(state, 65000).revealAnswer).toBe(true);
  });

  it("主动求助后无错完成也不记独立回忆", () => {
    let state = revealAnswer(start("blind", "hidden"));
    state = typeKey(state, "x", rng, 100).state;
    const result = typeKey(state, "k", rng, 200);
    expect(result.events[0]).toMatchObject({ outcome: "imperfect",
      independentRecall: false, usedAnswerHint: true, questionMs: 200 });
  });
});
