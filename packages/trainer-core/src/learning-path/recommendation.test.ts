import { describe, expect, it } from "vitest";
import { dailyRecommendation, modeForPool, REVIEW_MASTERY_FLOOR } from "./recommendation";
import { emptyProgress, type ItemProgress } from "../learning/progress";
import { recordConfusion } from "../learning/confusion";
import { makeIndex } from "../testing/fixtures";

const NOW = 1_000_000_000_000;

function progress(overrides: Partial<ItemProgress> = {}): ItemProgress {
  return { ...emptyProgress(), ...overrides };
}

function input(
  overrides: Partial<Parameters<typeof dailyRecommendation>[0]> = {},
): Parameters<typeof dailyRecommendation>[0] {
  return {
    index: makeIndex(),
    progressById: {},
    keyErrors: {},
    lessonEvidence: {},
    now: NOW,
    limit: 5,
    ...overrides,
  };
}

describe("dailyRecommendation", () => {
  it("空进度时推荐学习路径首章 + 高频新内容模式", () => {
    const picks = dailyRecommendation(input());
    const kinds = picks.map((pick) => pick.kind);
    expect(kinds).toContain("lesson");
    expect(kinds).toContain("practice-mode");
    const lesson = picks.find((pick) => pick.kind === "lesson")!;
    expect(lesson.chapterId).toBe("overview");
    // 单字池未见条目存在 → 推荐双拼模式。
    const mode = picks.find((pick) => pick.kind === "practice-mode")!;
    expect(mode.mode).toBe("double");
  });

  it("review-item 优先于 weak-item(曾经扎实、最近一次有失误的条目)", () => {
    const index = input().index;
    // 行:xk —— 掌握度高于底线但最近一次完成有失误(review)。
    // 好:hk —— 掌握度低、错误多(weak)。
    const progressById: Record<string, ItemProgress> = {
      "行:xk": progress({
        attempts: 20,
        correct: 19,
        wrong: 1,
        streak: 0,
        mastery: REVIEW_MASTERY_FLOOR + 5,
        lastSeenAt: NOW - 1000,
      }),
      "好:hk": progress({
        attempts: 10,
        correct: 2,
        wrong: 8,
        streak: 0,
        mastery: 10,
        lastSeenAt: NOW,
      }),
    };
    const picks = dailyRecommendation(input({ index, progressById, limit: 2 }));
    expect(picks[0]).toMatchObject({ kind: "review-item", itemId: "行:xk" });
    expect(picks[1]).toMatchObject({ kind: "weak-item", itemId: "好:hk" });
  });

  it("review 桶内按最近见过排序", () => {
    const index = input().index;
    const progressById: Record<string, ItemProgress> = {
      "行:xk": progress({
        attempts: 20, correct: 19, wrong: 1, streak: 0, mastery: 70, lastSeenAt: 100,
      }),
      "好:hk": progress({
        attempts: 20, correct: 18, wrong: 2, streak: 0, mastery: 65, lastSeenAt: 200,
      }),
    };
    const picks = dailyRecommendation(input({ index, progressById, limit: 2 }));
    expect(
      picks.map((pick) =>
        pick.kind === "lesson"
          ? "lesson"
          : pick.kind === "practice-mode" || pick.kind === "shape-confusion"
            ? "mode"
            : pick.itemId,
      ),
    ).toEqual([
      "好:hk",
      "行:xk",
    ]);
  });

  it("weak-item 之后是 recent-mistake(最近错过的条目)", () => {
    const index = input().index;
    const progressById: Record<string, ItemProgress> = {
      // weak 桶(掌握度低于 WEAK_MASTERY_CAP,占满 limit = 3)。
      "我:wo": progress({ attempts: 5, correct: 1, wrong: 4, mastery: 20, lastSeenAt: 300 }),
      "好:hknc": progress({ attempts: 6, correct: 2, wrong: 4, mastery: 25, lastSeenAt: 350 }),
      "好:hkn": progress({ attempts: 6, correct: 2, wrong: 4, mastery: 30, lastSeenAt: 400 }),
      // recent-mistake:有错史但掌握度不低、最近见过(不满足 review 底线)。
      "行:xkg": progress({ attempts: 6, correct: 5, wrong: 1, mastery: 50, lastSeenAt: 900 }),
    };
    const picks = dailyRecommendation(input({ index, progressById, limit: 3 }));
    expect(picks[0]).toMatchObject({ kind: "weak-item", itemId: "我:wo" });
    expect(picks[1]).toMatchObject({ kind: "weak-item", itemId: "好:hknc" });
    expect(picks[2]).toMatchObject({ kind: "weak-item", itemId: "好:hkn" });
    const withRoom = dailyRecommendation(input({ index, progressById, limit: 4 }));
    expect(withRoom[3]).toMatchObject({ kind: "recent-mistake", itemId: "行:xkg" });
  });

  it("首个未掌握章节作为 lesson 推荐;needs-review 章节给出复习理由", () => {
    const index = input().index;
    // 空证据:overview(无练习小节)永远 not-started,总是被选中。
    const base = dailyRecommendation(input({ index }));
    const lesson = base.find((pick) => pick.kind === "lesson")!;
    expect(lesson).toMatchObject({ chapterId: "overview", state: "not-started" });
  });

  it("recentIds 惩罚:刚练过的高频条目不再驱动 practice-mode 推荐", () => {
    const index = input().index;
    const topChar2 = index.frequencySorted["char-2"][0]!;
    const withRecent = dailyRecommendation(
      input({ index, recentIds: [topChar2.id] }),
    );
    const mode = withRecent.find((pick) => pick.kind === "practice-mode")!;
    // char-2 首条被惩罚后,若 char-2 仍有其余未见条目,推荐不变;
    // 这里池中还有未见条目,仍推荐 double。
    expect(mode.mode).toBe("double");

    // 把 char-2 全部条目标记为 recent → 推荐推进到 char-3 对应模式。
    const allChar2 = index.pools["char-2"].map((item) => item.id);
    const advanced = dailyRecommendation(input({ index, recentIds: allChar2 }));
    const advancedMode = advanced.find((pick) => pick.kind === "practice-mode")!;
    expect(advancedMode.mode).toBe("sound-shape");
  });

  it("limit 截断输出", () => {
    const picks = dailyRecommendation(input({ limit: 1 }));
    expect(picks).toHaveLength(1);
  });

  it("shape-confusion:形位重复混淆(count ≥ 阈值)→ 推荐全码强化", () => {
    const confusions = {
      ...recordConfusion(
        recordConfusion(recordConfusion({}, "n", "m", "shape1"), "n", "m", "shape1"),
        "n",
        "m",
        "shape1",
      ),
      ...recordConfusion({}, "x", "z", "sound1"), // 音位高频混淆不触发形码桶
    };
    const picks = dailyRecommendation(input({ confusions }));
    const shape = picks.find((pick) => pick.kind === "shape-confusion");
    expect(shape).toMatchObject({
      kind: "shape-confusion",
      mode: "full",
      expected: "n",
      actual: "m",
    });
    // 低于阈值(偶发失误)不触发。
    const sparse = dailyRecommendation(input({ confusions: recordConfusion({}, "n", "m", "shape1") }));
    expect(sparse.find((pick) => pick.kind === "shape-confusion")).toBeUndefined();
  });

  it("确定性:相同输入两次调用输出完全一致", () => {
    const index = input().index;
    const progressById: Record<string, ItemProgress> = {
      "行:xk": progress({ attempts: 4, wrong: 4, mastery: 0, lastSeenAt: 1 }),
      "好:hk": progress({ attempts: 5, wrong: 2, mastery: 30, lastSeenAt: 2 }),
    };
    const a = dailyRecommendation(input({ index, progressById }));
    const b = dailyRecommendation(input({ index, progressById }));
    expect(a).toEqual(b);
  });
});

describe("modeForPool", () => {
  it("池 → 最具体的练习模式", () => {
    expect(modeForPool("char-2")).toBe("double");
    expect(modeForPool("char-3")).toBe("sound-shape");
    expect(modeForPool("char-4")).toBe("full");
    expect(modeForPool("level1")).toBe("level1");
    expect(modeForPool("shortcut-two-key-zero-regression")).toBe("two-key-word");
    expect(modeForPool("word-6")).toBe("fixed-word");
    expect(modeForPool("sentence")).toBe("sentence");
  });
});
