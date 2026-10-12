import { describe, expect, it } from "vitest";
import { advance, backspace, confirmQuestion, createSession, expectedKey, typeKey } from "./engine";
import { buildPool } from "./scheduler";
import { makeEntry, makeIndex } from "../testing/fixtures";

const rng = () => 0;
describe("无上下文多音题", () => {
  it.each(["xk", "hh"])("接受同目标同长度的规范码 %s", (code) => {
    const index = makeIndex({ entries: [makeEntry("行", "xk"), makeEntry("行", "hh"), makeEntry("行", "xkg")] });
    const item = index.byId.get("行:xk")!;
    let state = createSession({ mode: "double", intent: "blind", hintMode: "hidden", targetLength: 0,
      pools: [buildPool("char-2", [item])] }, new Map(), rng, 0)!;
    expect(expectedKey(state)).toBeNull();
    for (const key of code) state = typeKey(state, key, rng, 100).state;
    expect(state.lastOutcome).toBe("perfect");
    expect(state.lastRoute).toBe("primary");
    state = advance(state, rng, 200).state;
    state = typeKey(state, "h", rng, 300).state;
    expect(expectedKey(state)).toBe("h");
    state = backspace(state);
    expect(expectedKey(state)).toBeNull();
    state = typeKey(state, "q", rng, 400).state;
    expect(state.typed).toBe("");
    expect(state.hadError).toBe(true);
  });
});

describe("短码终端与完整路线", () => {
  it.each(["confirm", "continue"])("规范时间=uij/uijm：%s", (action) => {
    const item = makeIndex().pools["shortcut-primary"][0];
    let state = createSession({ mode: "zero-regression", intent: "blind", hintMode: "hidden",
      targetLength: 1, pools: [buildPool("shortcut-primary", [item])] }, new Map(), rng, 0)!;
    expect(confirmQuestion(state, rng, 1).events).toEqual([]);
    for (const key of "uij") state = typeKey(state, key, rng, 100).state;
    expect(state.phase).toBe("question");
    expect(state.questionsCompleted).toBe(0);
    const result = action === "confirm"
      ? confirmQuestion(state, rng, 200)
      : typeKey(state, "m", rng, 200);
    expect(result.events[0]).toMatchObject({
      type: "question-completed",
      routeUsed: action === "confirm" ? "primary" : "alternate",
      independentRecall: action === "confirm",
      outcome: action === "confirm" ? "perfect" : "imperfect",
    });
    expect(confirmQuestion(result.state, rng, 300).events).toEqual([]);
  });
});
