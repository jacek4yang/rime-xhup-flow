import { describe, expect, it } from "vitest";
import { applyQuestionProgress, emptyProgress } from "./progress";
import { explainReviewStatus, bucketDueIn } from "./review-status";
import type { RecallEvidence } from "./spaced-review";

const DAY = 86_400_000;
const MINUTE = 60_000;
const answer: RecallEvidence = {
  outcome: "perfect", routeUsed: "primary", independentRecall: true,
  usedAnswerHint: false, wrongKeyEvents: 0, questionMs: 1000, chars: 1, now: 1000,
};

function level6Progress() {
  let progress = applyQuestionProgress(emptyProgress(), answer);
  for (let step = 0; step < 5; step += 1) {
    progress = applyQuestionProgress(progress, { ...answer, now: progress.review!.dueAt });
  }
  expect(progress.review!.level).toBe(6);
  return progress;
}

describe("复习等级状态解释", () => {
  it("首次完美回忆:等级 1,未到期,给出下次到期时间", () => {
    const progress = applyQuestionProgress(emptyProgress(), answer);
    const dueAt = progress.review!.dueAt;
    expect(dueAt).toBe(answer.now + DAY);
    const status = explainReviewStatus(progress, answer.now + MINUTE);
    expect(status).toMatchObject({
      level: 1, due: false, capped: false, reason: "not-due",
      nextDueAt: dueAt, nextDueInMs: dueAt - (answer.now + MINUTE),
      lastResult: "recalled",
    });
  });

  it("未验证条目:历史记录按到期处理,新条目不当作到期", () => {
    const legacy = { ...emptyProgress(), attempts: 20, mastery: 100 };
    expect(explainReviewStatus(legacy, 0)).toMatchObject({
      level: null, due: true, reason: "unverified", nextDueAt: null, nextDueInMs: null,
    });
    expect(explainReviewStatus(emptyProgress(), 0)).toMatchObject({
      level: null, due: false, reason: "unverified",
    });
  });

  it("到期边界精确:dueAt 当刻即到期,倒计时归零", () => {
    const progress = applyQuestionProgress(emptyProgress(), answer);
    const dueAt = progress.review!.dueAt;
    expect(explainReviewStatus(progress, dueAt - 1).due).toBe(false);
    const due = explainReviewStatus(progress, dueAt);
    expect(due).toMatchObject({ due: true, reason: "due-now", nextDueInMs: 0 });
  });

  it.each([
    { usedAnswerHint: true, lastResult: "hinted" },
    { routeUsed: "alternate" as const, lastResult: "alternate" },
    { wrongKeyEvents: 1, lastResult: "error" },
  ])("失败归零后按重试等待解释:$lastResult", ({ lastResult, ...patch }) => {
    const known = applyQuestionProgress(emptyProgress(), answer);
    const failedAt = known.review!.dueAt - 1;
    const failed = applyQuestionProgress(known, { ...answer, ...patch, now: failedAt });
    const status = explainReviewStatus(failed, failedAt + 1000);
    expect(status).toMatchObject({
      level: 0, due: false, reason: "lapsed", lastResult,
      nextDueAt: failedAt + 10 * MINUTE,
      nextDueInMs: failedAt + 10 * MINUTE - (failedAt + 1000),
    });
  });

  it("迟疑保持等级:未到期时沿用原到期时间", () => {
    const known = applyQuestionProgress(emptyProgress(), answer);
    const dueAt = known.review!.dueAt;
    const slow = applyQuestionProgress(known, { ...answer, now: dueAt - DAY / 2, questionMs: 8001 });
    expect(slow.review).toMatchObject({ level: 1, dueAt, lastResult: "hesitant" });
    expect(explainReviewStatus(slow, dueAt - DAY / 2 + MINUTE)).toMatchObject({
      level: 1, due: false, reason: "hesitant", nextDueAt: dueAt,
    });
  });

  it("6 级封顶:等级不再变化并明确说明", () => {
    const progress = level6Progress();
    const status = explainReviewStatus(progress, progress.review!.dueAt - DAY / 2);
    expect(status).toMatchObject({ level: 6, capped: true, reason: "capped", due: false });
    const advanced = applyQuestionProgress(progress, { ...answer, now: progress.review!.dueAt });
    expect(advanced.review!.level).toBe(6);
  });

  it("到期倒计时粗粒度化", () => {
    expect(bucketDueIn(0)).toEqual({ unit: "minute", value: 0 });
    expect(bucketDueIn(5 * MINUTE)).toEqual({ unit: "minute", value: 5 });
    expect(bucketDueIn(90 * MINUTE)).toEqual({ unit: "hour", value: 2 });
    expect(bucketDueIn(47 * 60 * MINUTE)).toEqual({ unit: "hour", value: 47 });
    expect(bucketDueIn(49 * 60 * MINUTE)).toEqual({ unit: "day", value: 3 });
    expect(bucketDueIn(3 * DAY)).toEqual({ unit: "day", value: 3 });
  });
});
