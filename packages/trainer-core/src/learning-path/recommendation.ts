/**
 * 每日推荐(纯函数,确定性;里程碑 44)。
 *
 * 无 ML、无云端、无随机源:推荐完全由输入结构决定,相同输入 → 相同
 * 输出(排序的全部决胜键都是显式的,不依赖 locale 比较)。
 *
 * 优先级(从高到低,同桶内再排序):
 * 1. review-item   ——「曾经掌握后回落」的条目(确定性代理:streak === 0
 *    说明最近一次完成有失误,mastery >= 60 说明此前已积累较扎实的掌握);
 * 2. weak-item     —— 复用 listWeakItems(掌握度低 → 错误多 → 最近错过);
 * 3. recent-mistake—— 最近练错过的条目(按 lastSeenAt 降序);
 * 4. lesson        —— 按 LEARN_CHAPTERS 顺序,第一个未达 mastered 的章节;
 * 5. shape-confusion —— 形位出现重复混淆(同一混淆对 count ≥
 *    {@link SHAPE_CONFUSION_MIN_COUNT})时推荐全码模式针对性强化;
 * 6. practice-mode —— 高频未见过的规范条目所在池对应的练习模式
 *    (selectPool 按频率排序,取第一个未见条目;recentIds 中的条目视为
 *    「刚练过」,不再作为新内容推荐)。
 *
 * 已知限制:keyErrors 作为保留字段接收(条目级排序已携带错误信号);
 * 键位级推荐经 confusions 的形位混淆桶(桶 5)接入。
 */

import type { ItemProgress } from "../learning/progress";
import { listWeakItems } from "../learning/review";
import type { ConfusionMap } from "../learning/confusion";
import { sortedConfusions } from "../learning/confusion";
import type { Difficulty, PoolId, TrainerIndex } from "../data/trainer-index";
import { selectPool } from "../data/trainer-index";
import { LEARN_CHAPTERS } from "../lessons/content";
import { MODE_POOL_ROTATION, type PracticeMode } from "../practice/types";
import type { I18nKey } from "../i18n";
import {
  chapterRelatedProgress,
  deriveLessonState,
  type LessonEvidence,
  type LessonState,
} from "./lesson-state";

/** 推荐条目(错题/薄弱/最近错过的统一形状)。 */
type ItemRecommendation = {
  kind: "review-item" | "weak-item" | "recent-mistake";
  itemId: string;
  /** 展示目标(汉字/词语)。 */
  target: string;
  /** 主练码。 */
  code: string;
  reason: I18nKey;
};

/** 推荐章节(学习路径的下一站 / 建议复习的章节)。 */
export type LessonRecommendation = {
  kind: "lesson";
  chapterId: string;
  /** 章节标题(规范中文)。 */
  title: string;
  /** 推导出的章节建议状态。 */
  state: LessonState;
  reason: I18nKey;
};

/** 推荐练习模式(高频新内容入口)。 */
export type PracticeModeRecommendation = {
  kind: "practice-mode";
  mode: PracticeMode;
  reason: I18nKey;
};

/** 形位混淆推荐(重复按错同一对形键 → 全码模式针对性强化)。 */
export type ShapeConfusionRecommendation = {
  kind: "shape-confusion";
  mode: PracticeMode;
  /** 当前最严重的形位混淆对(展示用)。 */
  expected: string;
  actual: string;
  reason: I18nKey;
};

export type Recommendation =
  | ItemRecommendation
  | LessonRecommendation
  | PracticeModeRecommendation
  | ShapeConfusionRecommendation;

export type RecommendationInput = {
  index: TrainerIndex;
  /** 稀疏条目进度(只含见过的条目)。 */
  progressById: Record<string, ItemProgress>;
  /** 键位累积错误(保留字段;见模块注释)。 */
  keyErrors: Record<string, number>;
  /** 键位混淆聚合(可选;形位混淆桶的数据源)。 */
  confusions?: ConfusionMap;
  /** 章节学习证据。 */
  lessonEvidence: Record<string, LessonEvidence>;
  /** 最近练过的条目 id(重复惩罚:不作为「新内容」推荐)。 */
  recentIds?: readonly string[];
  /** 高频新内容选题难度(默认 daily)。 */
  difficulty?: Difficulty;
  now: number;
  limit?: number;
};

/** 「曾经掌握后回落」代理的掌握度下限。 */
export const REVIEW_MASTERY_FLOOR = 60;

/**
 * 形位混淆桶的触发阈值:同一混淆对(期望形键 → 实际键)累计达到该
 * 次数才推荐(避免偶发失误触发);取最严重的一对,决胜键显式。
 */
export const SHAPE_CONFUSION_MIN_COUNT = 3;

/**
 * 薄弱条目的掌握度上限:weak-item 只收掌握度低于该值的条目;
 * 掌握度更高但有错史的条目归入 recent-mistake(按最近见过排序),
 * 两个桶互不重叠,保证低优先级桶不被高优先级桶完全遮蔽。
 */
export const WEAK_MASTERY_CAP = 50;

/** 池 ID → 单一练习模式(按 MODE_POOL_ROTATION 的插入序取首个匹配模式)。 */
export function modeForPool(poolId: PoolId): PracticeMode {
  for (const [mode, pools] of Object.entries(MODE_POOL_ROTATION)) {
    if (pools.includes(poolId)) return mode as PracticeMode;
  }
  return "mixed-all";
}

function itemRec(
  kind: ItemRecommendation["kind"],
  id: string,
  item: { target: string; primaryCode: string } | undefined,
  reason: I18nKey,
): ItemRecommendation {
  return {
    kind,
    itemId: id,
    target: item?.target ?? "",
    code: item?.primaryCode ?? "",
    reason,
  };
}

/**
 * 生成有序推荐(至多 limit 条;桶间按优先级,桶内排序确定性)。
 */
export function dailyRecommendation(input: RecommendationInput): Recommendation[] {
  const limit = input.limit ?? 5;
  const progress = input.progressById;
  const picked: Recommendation[] = [];
  const usedItems = new Set<string>();

  // 1. review-item:曾经扎实、最近一次完成有失误的条目。
  const reviewEntries: { id: string; lastSeenAt: number }[] = [];
  for (const [id, entry] of Object.entries(progress)) {
    if (
      entry.attempts > 0 &&
      entry.wrong > 0 &&
      entry.streak === 0 &&
      entry.mastery >= REVIEW_MASTERY_FLOOR
    ) {
      reviewEntries.push({ id, lastSeenAt: entry.lastSeenAt ?? 0 });
    }
  }
  reviewEntries.sort(
    (a, b) => b.lastSeenAt - a.lastSeenAt || a.id.localeCompare(b.id),
  );
  for (const entry of reviewEntries) {
    if (picked.length >= limit) break;
    usedItems.add(entry.id);
    picked.push(
      itemRec(
        "review-item",
        entry.id,
        input.index.byId.get(entry.id),
        "recommend.reasonReview",
      ),
    );
  }

  // 2. weak-item:复用 listWeakItems(掌握度低 → 错误多 → 最近错过),
  //    只保留掌握度低于 WEAK_MASTERY_CAP 的真薄弱条目。
  for (const { item, progress: entryProgress } of listWeakItems(input.index, progress, limit)) {
    if (picked.length >= limit) break;
    if (usedItems.has(item.id)) continue;
    if (entryProgress.mastery >= WEAK_MASTERY_CAP) continue;
    usedItems.add(item.id);
    picked.push(itemRec("weak-item", item.id, item, "recommend.reasonWeak"));
  }

  // 3. recent-mistake:最近练错过的条目(lastSeenAt 降序;弱桶之外)。
  const recent: { id: string; lastSeenAt: number }[] = [];
  for (const [id, entry] of Object.entries(progress)) {
    if (entry.attempts === 0 || entry.wrong === 0) continue;
    if (usedItems.has(id)) continue;
    if (entry.mastery < WEAK_MASTERY_CAP) continue;
    recent.push({ id, lastSeenAt: entry.lastSeenAt ?? 0 });
  }
  recent.sort((a, b) => b.lastSeenAt - a.lastSeenAt || a.id.localeCompare(b.id));
  for (const entry of recent) {
    if (picked.length >= limit) break;
    usedItems.add(entry.id);
    picked.push(
      itemRec(
        "recent-mistake",
        entry.id,
        input.index.byId.get(entry.id),
        "recommend.reasonRecentMistake",
      ),
    );
  }

  // 4. lesson:LEARN_CHAPTERS 顺序中第一个未达 mastered 的章节。
  for (const chapter of LEARN_CHAPTERS) {
    const state = deriveLessonState(
      input.lessonEvidence[chapter.id],
      chapterRelatedProgress(chapter, input.index, progress),
      input.now,
    );
    if (state === "mastered") continue;
    picked.push({
      kind: "lesson",
      chapterId: chapter.id,
      title: chapter.title,
      state,
      reason:
        state === "needs-review"
          ? "recommend.reasonLessonReview"
          : "recommend.reasonLesson",
    });
    break;
  }

  // 5. shape-confusion:形位重复混淆 → 全码模式针对性强化(确定性取
  //    最严重的形位对;音位与 {other:N} 条目不参与形码判定)。
  if (picked.length < limit) {
    const topShapeConfusion = sortedConfusions(input.confusions ?? {}).find(
      (entry) => entry.position === "shape1" || entry.position === "shape2",
    );
    if (topShapeConfusion && topShapeConfusion.count >= SHAPE_CONFUSION_MIN_COUNT) {
      picked.push({
        kind: "shape-confusion",
        mode: "full",
        expected: topShapeConfusion.expected,
        actual: topShapeConfusion.actual,
        reason: "recommend.reasonShapeConfusion",
      });
    }
  }

  // 6. practice-mode:高频未见过的规范条目所在池(跳过 recentIds)。
  if (picked.length < limit) {
    const recentSet = new Set(input.recentIds ?? []);
    for (const poolId of MODE_POOL_ROTATION["mixed-all"]) {
      const unseen = selectPool(input.index, poolId, input.difficulty ?? "daily").find(
        (item) => !progress[item.id] && !recentSet.has(item.id),
      );
      if (unseen) {
        picked.push({
          kind: "practice-mode",
          mode: modeForPool(poolId),
          reason: "recommend.reasonNewPool",
        });
        break;
      }
    }
  }

  return picked.slice(0, limit);
}
