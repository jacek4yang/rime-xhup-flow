/**
 * 复习等级状态解释(纯函数,桌面与小程序共享)。
 *
 * 只从已持久化的证据推导当前状态,不引入新语义;调度合同见
 * spaced-review.ts(0-6 级,间隔 [1,3,7,14,30,90] 天,提前重复不升级
 * 也不推迟到期,失败归零后 10 分钟可重试)。宿主据此回答:
 * 「当前等级是什么」「是否到期」「下次到期还有多久」「为什么等级没变」。
 */

import type { ItemProgress } from "./progress";
import { INTERVAL_DAYS, isReviewDue, type SpacedReview } from "./spaced-review";
import { translate, type Language } from "../i18n";

/**
 * 当前等级没有变化的可解释原因(确定性,由持久化状态推导)。
 * 宿主把它映射为 i18n 文案;这里不携带任何语言字符串。
 */
export type ReviewHoldReason =
  /** 尚无 V1 独立回忆证据(历史迁移或从未完成练习)。 */
  | "unverified"
  /** 已到期:本次完美独立回忆即可升级(封顶时保持 6 级)。 */
  | "due-now"
  /** 未到期:提前重复既不升级也不推迟既定到期时间。 */
  | "not-due"
  /** 上次回忆成功但超时(>8s/字),等级不变,原到期时间不变。 */
  | "hesitant"
  /** 上次未完成独立回忆(错键/提示/备用码),等级已重置。 */
  | "lapsed"
  /** 已达 6 级封顶,不存在更高等级。 */
  | "capped";

export type ReviewStatus = {
  /** 当前复习等级;null = 尚无独立回忆证据。 */
  level: number | null;
  /** 是否已到期(含「从未验证但已有练习记录」的条目)。 */
  due: boolean;
  /** 是否已达 6 级封顶。 */
  capped: boolean;
  /** 下次到期时间(epoch ms);未验证为 null。 */
  nextDueAt: number | null;
  /** 距下次到期还剩的 ms;已到期为 0;未验证为 null。 */
  nextDueInMs: number | null;
  /** 最近一次尝试结果(原样透传,供宿主展示「上次发生了什么」)。 */
  lastResult: SpacedReview["lastResult"] | null;
  /** 当前为什么维持这个等级(确定性的原因码)。 */
  reason: ReviewHoldReason;
};

export function explainReviewStatus(
  progress: Pick<ItemProgress, "attempts" | "review">,
  now: number,
): ReviewStatus {
  const review = progress.review;
  if (review === null) {
    return {
      level: null,
      due: progress.attempts > 0,
      capped: false,
      nextDueAt: null,
      nextDueInMs: null,
      lastResult: null,
      reason: "unverified",
    };
  }
  const due = isReviewDue(progress, now);
  const capped = review.level >= INTERVAL_DAYS.length;
  let reason: ReviewHoldReason;
  if (capped) {
    reason = "capped";
  } else if (due) {
    reason = "due-now";
  } else if (review.lastResult === "hesitant") {
    reason = "hesitant";
  } else if (
    review.lastResult === "error" ||
    review.lastResult === "hinted" ||
    review.lastResult === "alternate"
  ) {
    reason = "lapsed";
  } else {
    reason = "not-due";
  }
  return {
    level: review.level,
    due,
    capped,
    nextDueAt: review.dueAt,
    nextDueInMs: Math.max(0, review.dueAt - now),
    lastResult: review.lastResult,
    reason,
  };
}

/** 把到期倒计时粗粒度化,供宿主拼「{duration}后到期」文案(纯函数)。 */
export type DueInBucket = { unit: "day" | "hour" | "minute"; value: number };

export function bucketDueIn(ms: number): DueInBucket {
  if (ms <= 0) return { unit: "minute", value: 0 };
  const minutes = Math.ceil(ms / (60 * 1000));
  if (minutes < 60) return { unit: "minute", value: minutes };
  const hours = Math.ceil(ms / (60 * 60 * 1000));
  if (hours < 48) return { unit: "hour", value: hours };
  return { unit: "day", value: Math.ceil(ms / (24 * 60 * 60 * 1000)) };
}

/** 共享的「{duration}后到期」时长文案(双端同词;语言由宿主选择)。 */
export function formatDueIn(ms: number, language: Language): string {
  const bucket = bucketDueIn(ms);
  const key = bucket.unit === "day"
    ? "duration.dayCount"
    : bucket.unit === "hour"
      ? "duration.hourCount"
      : "duration.minuteCount";
  return translate(language, key, { n: bucket.value });
}
