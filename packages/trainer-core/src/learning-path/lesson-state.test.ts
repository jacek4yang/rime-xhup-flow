import { describe, expect, it } from "vitest";
import {
  DEFAULT_LESSON_THRESHOLDS,
  MIN_DECAY_ATTEMPTS,
  chapterRelatedItems,
  chapterRelatedProgress,
  deriveAllLessonStates,
  deriveLessonState,
  emptyLessonEvidence,
  type LessonEvidence,
} from "./lesson-state";
import { emptyProgress, type ItemProgress } from "../learning/progress";
import { makeIndex } from "../testing/fixtures";
import { LEARN_CHAPTERS } from "../lessons/content";

const NOW = 1_000_000_000_000;

function evidence(overrides: Partial<LessonEvidence> = {}): LessonEvidence {
  return { ...emptyLessonEvidence(), ...overrides };
}

function progress(overrides: Partial<ItemProgress> = {}): ItemProgress {
  return { ...emptyProgress(), ...overrides };
}

describe("deriveLessonState", () => {
  it("无任何证据 → not-started", () => {
    expect(deriveLessonState(undefined, [], NOW)).toBe("not-started");
    expect(deriveLessonState(emptyLessonEvidence(), [], NOW)).toBe("not-started");
  });

  it("打开过章节页但没有练习证据 → learning(打开页面 ≠ 掌握)", () => {
    expect(deriveLessonState(evidence({ openedAt: NOW - 100 }), [], NOW)).toBe(
      "learning",
    );
  });

  it("有练习会话但关联条目尚无进度 → practicing", () => {
    expect(
      deriveLessonState(
        evidence({ practiceSessions: 2, practiceAttempts: 5, lastPracticeAt: NOW }),
        [],
        NOW,
      ),
    ).toBe("practicing");
  });

  it("掌握度与表现均达标且活动新鲜 → mastered", () => {
    const related = [progress({ attempts: 10, correct: 10, mastery: 90, lastSeenAt: NOW })];
    expect(
      deriveLessonState(
        evidence({
          openedAt: NOW,
          practiceSessions: 3,
          practiceAttempts: 30,
          lastPracticeAt: NOW,
          bestAccuracy: 1,
        }),
        related,
        NOW,
      ),
    ).toBe("mastered");
  });

  it("达标但最近活动超出 recency 窗口 → needs-review(久未巩固)", () => {
    const related = [progress({ attempts: 10, correct: 10, mastery: 90, lastSeenAt: NOW })];
    const stale = NOW - DEFAULT_LESSON_THRESHOLDS.recencyMs - 1;
    expect(
      deriveLessonState(
        evidence({
          openedAt: stale,
          practiceSessions: 3,
          practiceAttempts: 30,
          lastPracticeAt: stale,
          bestAccuracy: 1,
        }),
        related,
        NOW,
      ),
    ).toBe("needs-review");
  });

  it("表现曾达标但平均掌握度回落(练习量足够)→ needs-review(掌握衰减)", () => {
    const related = [
      progress({ attempts: 12, correct: 6, wrong: 6, mastery: 40, lastSeenAt: NOW }),
    ];
    expect(
      deriveLessonState(
        evidence({
          practiceSessions: 5,
          practiceAttempts: 40,
          lastPracticeAt: NOW,
          bestAccuracy: 1,
        }),
        related,
        NOW,
      ),
    ).toBe("needs-review");
  });

  it("练习量太少时不判定掌握衰减(单题完美不算「曾掌握后回落」)", () => {
    const related = [progress({ attempts: 1, correct: 1, mastery: 10, lastSeenAt: NOW })];
    expect(
      deriveLessonState(
        evidence({
          practiceSessions: 1,
          practiceAttempts: 1,
          lastPracticeAt: NOW,
          bestAccuracy: 1,
        }),
        related,
        NOW,
      ),
    ).toBe("practicing");
    expect(MIN_DECAY_ATTEMPTS).toBeGreaterThan(1);
  });

  it("平均掌握度达标但表现未达标 → practicing", () => {
    const related = [progress({ attempts: 10, correct: 10, mastery: 95, lastSeenAt: NOW })];
    expect(
      deriveLessonState(
        evidence({
          practiceSessions: 2,
          practiceAttempts: 10,
          lastPracticeAt: NOW,
          bestAccuracy: 0.5,
        }),
        related,
        NOW,
      ),
    ).toBe("practicing");
  });

  it("忽略 attempts === 0 的关联进度条目", () => {
    expect(
      deriveLessonState(
        evidence({ practiceSessions: 1, practiceAttempts: 1, lastPracticeAt: NOW }),
        [progress(), progress({ attempts: 0, mastery: 100 })],
        NOW,
      ),
    ).toBe("practicing");
  });
});

describe("chapterRelatedItems / chapterRelatedProgress", () => {
  it("双拼章节关联 char-2 池;无练习小节的章节关联为空", () => {
    const index = makeIndex();
    const doubleChapter = LEARN_CHAPTERS.find((c) => c.id === "double")!;
    const overview = LEARN_CHAPTERS.find((c) => c.id === "overview")!;
    expect(chapterRelatedItems(doubleChapter, index).length).toBe(
      index.pools["char-2"].length,
    );
    expect(chapterRelatedItems(overview, index)).toEqual([]);
  });

  it("chapterRelatedProgress 只返回有练习记录的条目进度", () => {
    const index = makeIndex();
    const doubleChapter = LEARN_CHAPTERS.find((c) => c.id === "double")!;
    const first = index.pools["char-2"][0]!;
    const progressed = chapterRelatedProgress(doubleChapter, index, {
      [first.id]: progress({ attempts: 3, mastery: 50 }),
      "word:我们:womf": progress({ attempts: 9, mastery: 90 }),
    });
    expect(progressed).toHaveLength(1);
    expect(progressed[0]).toMatchObject({ attempts: 3, mastery: 50 });
  });
});

describe("deriveAllLessonStates", () => {
  it("按 LEARN_CHAPTERS 顺序输出全部章节状态", () => {
    const index = makeIndex();
    const states = deriveAllLessonStates(index, {}, {}, NOW);
    expect(states.map((entry) => entry.chapter.id)).toEqual(
      LEARN_CHAPTERS.map((chapter) => chapter.id),
    );
    expect(states.every((entry) => entry.state === "not-started")).toBe(true);
  });
});
