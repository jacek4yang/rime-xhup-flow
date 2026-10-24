import type { QuestionOutcome, QuestionRoute } from "../practice/types";

const DAY_MS = 24 * 60 * 60 * 1000;
/** 复习等级升级间隔(天);公开给宿主展示升级条件。 */
export const INTERVAL_DAYS = [1, 3, 7, 14, 30, 90] as const;
/** 失败后的重试等待(ms);公开给宿主解释「何时可重试」。 */
export const RETRY_MS = 10 * 60 * 1000;
export const RECALL_HESITATION_MS_PER_CHAR = 8000;

type RecallResult = "recalled" | "hesitant" | "error" | "hinted" | "alternate";

/** V1 records only observed recall. Historical mastery never implies recall. */
export type SpacedReview = {
  version: 1;
  level: number;
  dueAt: number;
  lastAttemptAt: number;
  lastIndependentAt: number | null;
  lastResult: RecallResult;
  questionMs: number;
  wrongKeyEvents: number;
  usedAnswerHint: boolean;
  routeUsed: QuestionRoute;
  lapses: number;
};

export type RecallEvidence = {
  outcome: QuestionOutcome;
  routeUsed: QuestionRoute;
  independentRecall: boolean;
  usedAnswerHint: boolean;
  wrongKeyEvents: number;
  questionMs: number;
  chars: number;
  now: number;
};

export function independentRecall(evidence: RecallEvidence): boolean {
  return evidence.independentRecall && evidence.outcome === "perfect" &&
    evidence.routeUsed === "primary" && !evidence.usedAnswerHint && evidence.wrongKeyEvents === 0;
}

/** Early repeats cannot advance a level or postpone the existing due date. */
export function updateSpacedReview(previous: SpacedReview | null, evidence: RecallEvidence): SpacedReview {
  const recalled = independentRecall(evidence);
  const hesitant = evidence.questionMs > RECALL_HESITATION_MS_PER_CHAR * Math.max(1, evidence.chars);
  const lastResult: RecallResult = evidence.usedAnswerHint ? "hinted"
    : evidence.routeUsed === "alternate" ? "alternate"
    : !recalled ? "error" : hesitant ? "hesitant" : "recalled";
  const due = previous === null || evidence.now >= previous.dueAt;
  const level = !recalled ? 0 : hesitant ? previous?.level ?? 0
    : due ? Math.min(INTERVAL_DAYS.length, (previous?.level ?? 0) + 1) : previous.level;
  const dueAt = !recalled ? evidence.now + RETRY_MS
    : hesitant ? Math.min(!due && previous ? previous.dueAt : Infinity, evidence.now + DAY_MS)
    : due ? evidence.now + INTERVAL_DAYS[level - 1]! * DAY_MS : previous.dueAt;
  return {
    version: 1, level, dueAt, lastAttemptAt: evidence.now,
    lastIndependentAt: recalled ? evidence.now : previous?.lastIndependentAt ?? null,
    lastResult, questionMs: evidence.questionMs, wrongKeyEvents: evidence.wrongKeyEvents,
    usedAnswerHint: evidence.usedAnswerHint, routeUsed: evidence.routeUsed,
    lapses: (previous?.lapses ?? 0) + (recalled ? 0 : 1),
  };
}

export function isReviewDue(progress: { attempts: number; review: SpacedReview | null }, now: number): boolean {
  return progress.attempts > 0 && (progress.review === null || progress.review.dueAt <= now);
}

/** Shared persistence boundary; unknown/corrupt review loses no legacy counters. */
export function parseSpacedReview(value: unknown): SpacedReview | null {
  if (typeof value !== "object" || value === null || Array.isArray(value)) return null;
  const raw = value as Record<string, unknown>;
  const finite = (n: unknown): n is number => typeof n === "number" && Number.isFinite(n) && n >= 0;
  if (raw.version !== 1 || !finite(raw.level) || !Number.isInteger(raw.level) || raw.level > INTERVAL_DAYS.length ||
      !finite(raw.dueAt) || !finite(raw.lastAttemptAt) ||
      !(raw.lastIndependentAt === null || finite(raw.lastIndependentAt)) ||
      !finite(raw.questionMs) || !finite(raw.wrongKeyEvents) || !Number.isInteger(raw.wrongKeyEvents) ||
      !finite(raw.lapses) || !Number.isInteger(raw.lapses) || typeof raw.usedAnswerHint !== "boolean" ||
      (raw.routeUsed !== "primary" && raw.routeUsed !== "alternate") ||
      typeof raw.lastResult !== "string" ||
      !["recalled", "hesitant", "error", "hinted", "alternate"].includes(raw.lastResult)) return null;
  return {
    version: 1, level: raw.level, dueAt: raw.dueAt, lastAttemptAt: raw.lastAttemptAt,
    lastIndependentAt: raw.lastIndependentAt, lastResult: raw.lastResult as RecallResult,
    questionMs: raw.questionMs, wrongKeyEvents: raw.wrongKeyEvents,
    usedAnswerHint: raw.usedAnswerHint, routeUsed: raw.routeUsed, lapses: raw.lapses,
  };
}
