/**
 * 学习中心覆盖数推导(小程序)。
 *
 * 诚实边界:小程序没有桌面端的 lessonEvidence 章节记账,章节「学习
 * 状态」(learning/practicing/mastered...)在本机不可推导 → 调用方不得
 * 展示状态徽标;覆盖数(已练/关联条目)只依赖本机真实练习进度,可靠。
 */

import {
  deriveAllLessonStates,
  type ItemProgress,
  type TrainerIndex,
} from "@xhup/trainer-core";

export type ChapterCoverage = { practiced: number; total: number };

/** 各章节覆盖数(按 LEARN_CHAPTERS 顺序;不含状态判定)。 */
export function chapterCoverageById(
  index: TrainerIndex,
  progress: Record<string, ItemProgress>,
  now: number,
): Map<string, ChapterCoverage> {
  const map = new Map<string, ChapterCoverage>();
  // 证据表刻意传 {}:deriveAllLessonStates 的状态字段因此全部为
  // not-started,调用方只读覆盖数,不消费状态。
  for (const summary of deriveAllLessonStates(index, progress, {}, now)) {
    map.set(summary.chapter.id, {
      practiced: summary.practicedCount,
      total: summary.relatedTotal,
    });
  }
  return map;
}
