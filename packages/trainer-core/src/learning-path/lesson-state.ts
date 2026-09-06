/**
 * 章节学习状态推导(纯函数,确定性;里程碑 44)。
 *
 * 状态只从真实学习证据推导:章节页的打开/练习记录({@link LessonEvidence})
 * 加上该章节练习模式关联条目的稀疏进度。打开过章节页面 ≠ 掌握;
 * 没有任何证据就绝不声称进度。
 *
 * 状态是「建议标签」而不是访问闸门(ADVISORY,永不锁课):任何状态下,
 * 章节内容与练习入口都保持可用,宿主 UI 只做展示,不做门控。
 *
 * 已知限制(诚实记录,不虚构数据):
 * - 引擎未持久化逐题「首次尝试是否完美」,这里以 bestAccuracy(练习中
 *   单题键级准确率的最佳值,由宿主在每题完成时上报)作为最近练习表现
 *   的代理;
 * - 「曾经掌握」没有独立标记,由 bestAccuracy 达标而当前平均掌握度回落
 *   来推断「掌握衰减 → 建议复习」。
 */

import type { ItemProgress } from "../learning/progress";
import type { TrainerIndex, TrainingItem } from "../data/trainer-index";
import {
  LEARN_CHAPTERS,
  type LearnChapter,
} from "../lessons/content";
import { MODE_POOL_ROTATION } from "../practice/types";

/** 章节的建议学习状态。 */
export type LessonState =
  | "not-started"
  | "learning"
  | "practicing"
  | "mastered"
  | "needs-review";

/** 单个章节的学习证据(稀疏:没有交互的章节没有记录)。 */
export type LessonEvidence = {
  /** 最近一次打开章节页的时间(epoch ms);从未打开为 null。 */
  openedAt: number | null;
  /** 从该章节发起的练习会话次数。 */
  practiceSessions: number;
  /** 通过该章节练习模式完成的题数(跨会话累计)。 */
  practiceAttempts: number;
  /** 最近一次练习完成时间(epoch ms);从未练习为 null。 */
  lastPracticeAt: number | null;
  /** 练习中单题键级准确率的最佳值(0..1);无有效样本为 null。 */
  bestAccuracy: number | null;
};

export function emptyLessonEvidence(): LessonEvidence {
  return {
    openedAt: null,
    practiceSessions: 0,
    practiceAttempts: 0,
    lastPracticeAt: null,
    bestAccuracy: null,
  };
}

/** 判定阈值(默认值;可整体覆盖以便测试与宿主调参)。 */
export type LessonStateThresholds = {
  /** 关联条目平均掌握度达到该值才算「掌握候选」。 */
  mastery: number;
  /** 练习准确率代理(bestAccuracy)达到该值才算「表现达标」。 */
  accuracy: number;
  /** 最近活动(打开/练习)在该时间窗内才算「保持」。 */
  recencyMs: number;
};

export const DEFAULT_LESSON_THRESHOLDS: LessonStateThresholds = {
  mastery: 80,
  accuracy: 0.8,
  recencyMs: 7 * 24 * 60 * 60 * 1000,
};

/**
 * 「掌握衰减」判定的最低练习量:bestAccuracy 是单题代理,样本太少时
 * (如只完美打对一题)不足以声称「曾经掌握后回落」,按 practicing 处理。
 */
export const MIN_DECAY_ATTEMPTS = 10;

/**
 * 推导章节的建议学习状态。语义(精确):
 *
 * 1. 无任何证据 → `not-started`;
 * 2. 有证据(打开过页面/发起过练习)但从未完成任何题 → `learning`;
 * 3. 有练习但关联条目尚无进度记录 → `practicing`;
 * 4. bestAccuracy 达标且平均掌握度回落到阈值以下(练习量足够)→
 *    `needs-review`(掌握衰减);
 * 5. 平均掌握度与 bestAccuracy 均达标:最近活动在 recency 窗口内 →
 *    `mastered`;超出窗口 → `needs-review`(久未巩固,建议回炉);
 * 6. 其余(平均掌握度或表现未达标)→ `practicing`。
 *
 * 平均掌握度只统计见过的关联条目(attempts > 0);关联条目全部未见过
 * 时按第 3 条处理。
 */
export function deriveLessonState(
  evidence: LessonEvidence | undefined,
  relatedItemsProgress: readonly ItemProgress[],
  now: number,
  thresholds: LessonStateThresholds = DEFAULT_LESSON_THRESHOLDS,
): LessonState {
  if (!evidence) return "not-started";
  const hasEvidence =
    evidence.openedAt !== null ||
    evidence.practiceSessions > 0 ||
    evidence.practiceAttempts > 0;
  if (!hasEvidence) return "not-started";

  const practiced = relatedItemsProgress.filter(
    (progress) => progress.attempts > 0,
  );
  const hasPractice =
    evidence.practiceSessions > 0 ||
    evidence.practiceAttempts > 0 ||
    practiced.length > 0;
  if (!hasPractice) return "learning";
  if (practiced.length === 0) return "practicing";

  const totalMastery = practiced.reduce((sum, p) => sum + p.mastery, 0);
  const avgMastery = totalMastery / practiced.length;
  const bestAccuracy = evidence.bestAccuracy ?? 0;

  const enoughVolume =
    evidence.practiceAttempts >= MIN_DECAY_ATTEMPTS ||
    practiced.reduce((sum, p) => sum + p.attempts, 0) >= MIN_DECAY_ATTEMPTS;

  if (
    enoughVolume &&
    bestAccuracy >= thresholds.accuracy &&
    avgMastery < thresholds.mastery
  ) {
    return "needs-review";
  }

  if (
    avgMastery >= thresholds.mastery &&
    bestAccuracy >= thresholds.accuracy
  ) {
    const lastActivity = Math.max(evidence.openedAt ?? 0, evidence.lastPracticeAt ?? 0);
    return lastActivity >= now - thresholds.recencyMs ? "mastered" : "needs-review";
  }

  return "practicing";
}

/** 章节关联条目 = 该章节所有练习小节的模式对应池中的全部条目。 */
export function chapterRelatedItems(
  chapter: LearnChapter,
  index: TrainerIndex,
): TrainingItem[] {
  const seen = new Set<string>();
  const items: TrainingItem[] = [];
  for (const section of chapter.sections) {
    if (section.kind !== "practice") continue;
    for (const poolId of MODE_POOL_ROTATION[section.mode]) {
      for (const item of index.pools[poolId]) {
        if (seen.has(item.id)) continue;
        seen.add(item.id);
        items.push(item);
      }
    }
  }
  return items;
}

/**
 * 章节关联条目的稀疏进度(只含有练习记录的条目,保持条目顺序)。
 * LEARN_CHAPTERS 中无练习小节的章节返回空数组。
 */
export function chapterRelatedProgress(
  chapter: LearnChapter,
  index: TrainerIndex,
  progressById: Record<string, ItemProgress>,
): ItemProgress[] {
  const progress: ItemProgress[] = [];
  for (const item of chapterRelatedItems(chapter, index)) {
    const entry = progressById[item.id];
    if (entry && entry.attempts > 0) progress.push(entry);
  }
  return progress;
}

/** 全部章节的建议状态(按 LEARN_CHAPTERS 顺序;供列表徽标使用)。 */
export function deriveAllLessonStates(
  index: TrainerIndex,
  progressById: Record<string, ItemProgress>,
  lessonEvidence: Record<string, LessonEvidence>,
  now: number,
): { chapter: LearnChapter; state: LessonState }[] {
  return LEARN_CHAPTERS.map((chapter) => ({
    chapter,
    state: deriveLessonState(
      lessonEvidence[chapter.id],
      chapterRelatedProgress(chapter, index, progressById),
      now,
    ),
  }));
}
