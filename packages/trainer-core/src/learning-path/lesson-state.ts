/**
 * 章节学习状态推导(纯函数,确定性;里程碑 44,任务书 §2.2 修订)。
 *
 * 状态只从真实学习证据推导:章节页的打开/练习记录({@link LessonEvidence})
 * 加上该章节练习模式关联条目的稀疏进度。打开过章节页面 ≠ 掌握;
 * 没有任何证据就绝不声称进度。
 *
 * 状态是「建议标签」而不是访问闸门(ADVISORY,永不锁课):任何状态下,
 * 章节内容与练习入口都保持可用,宿主 UI 只做展示,不做门控。
 *
 * 真实性边界(诚实记录,不虚构数据):
 * - 章节关联条目池通常很大(数千条),「已掌握」只对**已练习样本**成立,
 *   绝不声称整章覆盖;覆盖数(已练/关联总数)由本模块返回,宿主必须展示;
 * - 样本充分性:已练条目少于 `minPracticedItems` 时不可声称「已掌握」,
 *   无论平均掌握度多高;
 * - 练习表现优先用按题键级准确率的 EWMA(ItemProgress.accuracyEwma,
 *   近期真实表现);关联条目全部没有 EWMA(迁移历史)时才回退到宿主上报的
 *   历史 bestAccuracy 代理,该代理是终身最佳单题值,只作兼容、不作为
 *   新数据的首选依据;
 * - 「曾经掌握」没有独立标记,由表现达标而当前平均掌握度回落来推断
 *   「掌握衰减 → 建议复习」。
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
  /** 练习中单题键级准确率的最佳值(0..1);无有效样本为 null。
   *  旧版代理:仅当关联条目全部没有 accuracyEwma 时用于兼容回退。 */
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
  /** 练习表现(准确率 EWMA,回退 bestAccuracy)达到该值才算「表现达标」。 */
  accuracy: number;
  /** 最近练习(仅练习,不含打开页面)在该时间窗内才算「保持」。 */
  recencyMs: number;
  /** 「已掌握」要求的最少已练不同条目数(样本充分性)。 */
  minPracticedItems: number;
  /**
   * 「已掌握」要求的最少新证据条目数(accuracyEwma 非 null)。
   * 只要存在新证据,就必须达到该数量才验证整章表现;全部为迁移历史
   * (零新证据)时维持 bestAccuracy 兼容回退。防止单个新样本沿用
   * 终身最佳代理的达标(混合数据下的复发路径)。
   */
  minNewEvidenceItems: number;
};

export const DEFAULT_LESSON_THRESHOLDS: LessonStateThresholds = {
  mastery: 80,
  accuracy: 0.8,
  recencyMs: 7 * 24 * 60 * 60 * 1000,
  minPracticedItems: 20,
  minNewEvidenceItems: 5,
};

/**
 * 「掌握衰减」判定的最低练习量:表现代理样本太少时
 * (如只完美打对一题)不足以声称「曾经掌握后回落」,按 practicing 处理。
 */
export const MIN_DECAY_ATTEMPTS = 10;

/**
 * 推导章节的建议学习状态。语义(精确):
 *
 * 1. 无任何证据 → `not-started`;
 * 2. 有证据(打开过页面/发起过练习)但从未完成任何题 → `learning`;
 * 3. 有练习但关联条目尚无进度记录 → `practicing`;
 * 4. 表现达标且平均掌握度回落到阈值以下(练习量足够)→
 *    `needs-review`(掌握衰减);
 * 5. 样本充分(已练条目 ≥ minPracticedItems)且平均掌握度与表现均达标
 *    且新证据充分(存在 accuracyEwma 时 ≥ minNewEvidenceItems;全为迁移
 *    历史时沿用 bestAccuracy 回退):最近**练习**在 recency 窗口内 →
 *    `mastered`;超出窗口 → `needs-review`(久未巩固,建议回炉)。
 *    打开页面只算浏览,不延长「已掌握」的新鲜度;
 * 6. 其余(样本不足,或平均掌握度/表现未达标)→ `practicing`。
 *
 * 平均掌握度只统计见过的关联条目(attempts > 0);关联条目全部未见过
 * 时按第 3 条处理。`relatedTotal` 用于宿主展示覆盖数,不参与状态判定
 * (关联池常达数千条,覆盖率门禁不可行)。
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
  const performance = practicedPerformance(practiced, evidence);

  const enoughVolume =
    evidence.practiceAttempts >= MIN_DECAY_ATTEMPTS ||
    practiced.reduce((sum, p) => sum + p.attempts, 0) >= MIN_DECAY_ATTEMPTS;

  if (
    enoughVolume &&
    performance >= thresholds.accuracy &&
    avgMastery < thresholds.mastery
  ) {
    return "needs-review";
  }

  if (
    practiced.length >= thresholds.minPracticedItems &&
    avgMastery >= thresholds.mastery &&
    performance >= thresholds.accuracy &&
    newEvidenceSufficient(practiced, thresholds)
  ) {
    const lastActivity = evidence.lastPracticeAt ?? 0;
    return lastActivity >= now - thresholds.recencyMs ? "mastered" : "needs-review";
  }

  return "practicing";
}

/**
 * 新证据充分性:全部为迁移历史(零新证据)时沿用兼容回退;
 * 一旦存在新证据(accuracyEwma 非 null),必须达到 minNewEvidenceItems
 * 才允许「已掌握」。新证据少于阈值时按 practicing 处理,不把单个新
 * 样本当成整章表现的验证。
 */
function newEvidenceSufficient(
  practiced: readonly ItemProgress[],
  thresholds: LessonStateThresholds,
): boolean {
  let known = 0;
  for (const progress of practiced) {
    if (progress.accuracyEwma !== null) known += 1;
    if (known >= thresholds.minNewEvidenceItems) return true;
  }
  return known === 0;
}

/**
 * 章节练习表现(0..1):优先用关联条目按题键级准确率 EWMA 的均值
 * (近期真实表现);全部条目都没有 EWMA(迁移历史)时回退到宿主上报的
 * 历史 bestAccuracy 代理。
 */
export function practicedPerformance(
  practiced: readonly ItemProgress[],
  evidence: LessonEvidence,
): number {
  const withEwma = practiced
    .map((progress) => progress.accuracyEwma)
    .filter((value): value is number => value !== null);
  if (withEwma.length > 0) {
    return withEwma.reduce((sum, value) => sum + value, 0) / withEwma.length;
  }
  return evidence.bestAccuracy ?? 0;
}

/** 章节关联条目 = 该章节所有练习小节的模式对应池中的全部条目。 */
export function chapterRelatedItems(
  chapter: LearnChapter,
  index: TrainerIndex,
): TrainingItem[] {
  return relatedItemsFor(chapter, index);
}

/**
 * 关联条目列表缓存(WeakMap,按 index 身份)。章节关联池随数据集固定
 * (mastery 章节实测 117k 条目),而状态推导每分钟刷新;重复物化没有
 * 意义。纯记忆化:同输入同输出,不改变任何计算结果。
 */
const relatedItemsCache = new WeakMap<TrainerIndex, Map<string, TrainingItem[]>>();

function relatedItemsFor(
  chapter: LearnChapter,
  index: TrainerIndex,
): TrainingItem[] {
  let byChapter = relatedItemsCache.get(index);
  if (!byChapter) {
    byChapter = new Map();
    relatedItemsCache.set(index, byChapter);
  }
  const cached = byChapter.get(chapter.id);
  if (cached) return cached;
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
  byChapter.set(chapter.id, items);
  return items;
}

/** 关联条目 ID 集合缓存(反向成员判定;deriveAllLessonStates 用)。 */
const relatedIdsCache = new WeakMap<TrainerIndex, Map<string, Set<string>>>();

function chapterRelatedIds(chapter: LearnChapter, index: TrainerIndex): Set<string> {
  let byChapter = relatedIdsCache.get(index);
  if (!byChapter) {
    byChapter = new Map();
    relatedIdsCache.set(index, byChapter);
  }
  const cached = byChapter.get(chapter.id);
  if (cached) return cached;
  const ids = new Set(relatedItemsFor(chapter, index).map((item) => item.id));
  byChapter.set(chapter.id, ids);
  return ids;
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

/** 章节状态与覆盖信息(覆盖数供宿主展示,不让「已掌握」冒充整章掌握)。 */
export type LessonStateSummary = {
  chapter: LearnChapter;
  state: LessonState;
  /** 章节关联条目总数(0 = 无练习小节的章节)。 */
  relatedTotal: number;
  /** 已实际练习过的不同关联条目数。 */
  practicedCount: number;
};

/** 全部章节的建议状态与覆盖(按 LEARN_CHAPTERS 顺序;供列表徽标使用)。
 * 反向遍历:按 progressById(通常数千条)查每章的关联 ID 集合,而不是
 * 按每章关联条目(最大 117k)查进度表;结果与 chapterRelatedProgress
 * 路径完全一致(顺序无关的聚合)。 */
export function deriveAllLessonStates(
  index: TrainerIndex,
  progressById: Record<string, ItemProgress>,
  lessonEvidence: Record<string, LessonEvidence>,
  now: number,
): LessonStateSummary[] {
  const practicedById = Object.entries(progressById);
  const summaries = LEARN_CHAPTERS.map((chapter) => {
    const ids = chapterRelatedIds(chapter, index);
    const practiced: ItemProgress[] = [];
    for (const [id, progress] of practicedById) {
      if (progress.attempts > 0 && ids.has(id)) practiced.push(progress);
    }
    return {
      chapter,
      state: deriveLessonState(
        lessonEvidence[chapter.id],
        practiced,
        now,
      ),
      relatedTotal: ids.size,
      practicedCount: practiced.length,
    };
  });
  return summaries;
}
