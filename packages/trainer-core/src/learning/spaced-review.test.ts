import { describe, expect, it } from "vitest";
import { applyQuestionProgress, emptyProgress } from "./progress";
import { isReviewDue, parseSpacedReview, type RecallEvidence } from "./spaced-review";
import { buildPool, pickWeighted } from "../practice/scheduler";
import { charItem } from "../data/trainer-index";
import { makeEntry, makeIndex } from "../testing/fixtures";
import { listReviewItems } from "./review";

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

  it("间隔按 epoch 计算,不受本地时区/夏令时切换影响", () => {
    // 2026-03-08 美国夏令时切换日;从切换前 2 小时起的第一级间隔
    // 仍是精确 24h epoch,而不是"次日同一本地时刻"。
    const beforeDst = Date.UTC(2026, 2, 8, 6, 0, 0); // UTC 06:00,当地 01:00 EST
    const first = applyQuestionProgress(emptyProgress(), { ...answer, now: beforeDst });
    expect(first.review!.dueAt - beforeDst).toBe(DAY);
    const advanced = applyQuestionProgress(first, { ...answer, now: first.review!.dueAt });
    expect(advanced.review!.dueAt - first.review!.dueAt).toBe(3 * DAY);
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

it("零错误的到期项进入复习,同分决胜不依赖存储对象顺序或本机语言", () => {
  const index = makeIndex({ entries: [makeEntry("爱", "ai"), makeEntry("汉", "hj")] });
  const progress = { ...emptyProgress(), attempts: 3, correct: 3, mastery: 100 };
  const left = listReviewItems(index, { "爱:ai": progress, "汉:hj": progress }, 0);
  const right = listReviewItems(index, { "汉:hj": progress, "爱:ai": progress }, 0);
  expect(left.map(({ item }) => item.id)).toEqual(["汉:hj", "爱:ai"]);
  expect(right.map(({ item }) => item.id)).toEqual(["汉:hj", "爱:ai"]);
});
