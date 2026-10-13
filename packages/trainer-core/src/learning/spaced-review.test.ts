import { describe, expect, it } from "vitest";
import { applyQuestionProgress, emptyProgress } from "./progress";
import { isReviewDue, parseSpacedReview, type RecallEvidence } from "./spaced-review";
import { buildPool, pickWeighted } from "../practice/scheduler";
import { charItem } from "../data/trainer-index";
import { makeEntry } from "../testing/fixtures";

const DAY = 86_400_000;
const answer: RecallEvidence = {
  outcome: "perfect", routeUsed: "primary", independentRecall: true,
  usedAnswerHint: false, wrongKeyEvents: 0, questionMs: 1000, chars: 1, now: 1000,
};

describe("跨天独立回忆", () => {
  it("同日重复不刷等级或推迟到期,到期边界才晋级", () => {
    const first = applyQuestionProgress(emptyProgress(), answer);
    const due = first.review!.dueAt;
    expect(due).toBe(answer.now + DAY);
    const early = applyQuestionProgress(first, { ...answer, now: due - 1 });
    expect(early.review).toMatchObject({ level: 1, dueAt: due });
    expect(isReviewDue(early, due - 1)).toBe(false);
    expect(isReviewDue(early, due)).toBe(true);
    const next = applyQuestionProgress(early, { ...answer, now: due });
    expect(next.review).toMatchObject({ level: 2, dueAt: due + 3 * DAY });
  });

  it.each([
    { usedAnswerHint: true, lastResult: "hinted" },
    { routeUsed: "alternate" as const, lastResult: "alternate" },
    { wrongKeyEvents: 1, lastResult: "error" },
  ])("提示、备用码和错键不能冒充独立回忆: $lastResult", ({ lastResult, ...patch }) => {
    const known = applyQuestionProgress(emptyProgress(), answer);
    const failed = applyQuestionProgress(known, { ...answer, ...patch, now: 2000 });
    expect(failed.review).toMatchObject({ level: 0, dueAt: 602000, lastResult,
      lastIndependentAt: answer.now, lapses: 1 });
    expect(failed.correct).toBe(1);
    expect(failed.wrong).toBe(1);
  });

  it("迟疑保留独立回忆证据但不晋级,已过期的题安排次日", () => {
    const known = applyQuestionProgress(emptyProgress(), answer);
    const now = known.review!.dueAt + DAY;
    const slow = applyQuestionProgress(known, { ...answer, now, questionMs: 8001 });
    expect(slow.review).toMatchObject({ level: 1, dueAt: now + DAY,
      lastResult: "hesitant", lastIndependentAt: now, lapses: 0 });
    const sentence = applyQuestionProgress(known, { ...answer, now, chars: 2, questionMs: 8001 });
    expect(sentence.review!.level).toBe(2);
  });

  it("历史满掌握记录不捏造独立回忆,新题不当作到期旧题", () => {
    const legacy = { ...emptyProgress(), attempts: 20, mastery: 100 };
    expect(isReviewDue(legacy, 0)).toBe(true);
    expect(isReviewDue(emptyProgress(), 0)).toBe(false);
    expect(parseSpacedReview(undefined)).toBeNull();
    const valid = applyQuestionProgress(emptyProgress(), answer).review!;
    expect(parseSpacedReview({ ...valid, version: 2 })).toBeNull();
    expect(parseSpacedReview({ ...valid, dueAt: -1 })).toBeNull();
  });

  it("到期题胜过高频未到期题,仍遵守最近题排除", () => {
    const a = charItem(makeEntry("甲", "ja", 100000));
    const b = charItem(makeEntry("乙", "yi", 1));
    const pool = buildPool("char-2", [a, b]);
    const first = applyQuestionProgress(emptyProgress(), answer);
    const due = first.review!.dueAt;
    const later = applyQuestionProgress(emptyProgress(), { ...answer, now: due });
    const progress = new Map([[a.id, later], [b.id, first]]);
    expect(pickWeighted(pool, progress, new Set(), () => 0, due)?.id).toBe(b.id);
    expect(pickWeighted(pool, progress, new Set([b.id]), () => 0, due)?.id).toBe(a.id);
  });
});
