/**
 * 练习调度器(纯逻辑,不依赖 React,可注入 RNG 测试)。
 *
 * V2:池以字符串 PoolId 为键(单字/词按码长、简码按层、组句单池),
 * 轮换顺序由 MODE_POOL_ROTATION[mode] 给出;同一套加权调度覆盖全部模式,
 * 不为任何模式单写一套选题逻辑。
 *
 * 选题优先级 = 词频增益 × 薄弱度 × 未见增益 × 迟疑增益 × 层权重:
 *
 *   frequencyNorm  = maxLog === 0 ? 0 : log1p(score) / maxLog
 *   frequencyBoost = 0.75 + frequencyNorm * 0.75      (约 0.75..1.50)
 *   weakness       = 1 + wrong * 3 + (100 - mastery) / 20
 *   unseenBoost    = attempts === 0 ? 1.4 : 1
 *   latencyBoost   = 1 + min(0.5, avgLatencyPerChar / LATENCY_REF_MS_PER_CHAR)
 *   priority       = frequencyBoost * weakness * unseenBoost * latencyBoost * layerWeight
 *
 * 另有两条规则:最近 5 题防重复;答错的题 3-8 题后自然回炉(仅会话内,
 * 不持久化)。轮换按 MODE_POOL_ROTATION 顺序推进,到期回炉题可临时打破。
 */

import { emptyProgress, type ItemProgress } from "@/lib/progress";
import type { TrainingItem } from "@/lib/trainer-index";
import { MODE_POOL_ROTATION, type PracticeMode } from "./types";

export type Rng = () => number;

/** 一个候选池:统一训练项列表 + 预计算的 log 词频上限。 */
export type QuestionPool = {
  id: string;
  items: readonly TrainingItem[];
  maxLogFrequency: number;
  /** 池级权重(如一级简码等基础层略优先;默认 1)。 */
  layerWeight: number;
};

/** 迟疑增益的参照:每汉字平均完成耗时达到该值时增益封顶(+0.5)。 */
export const LATENCY_REF_MS_PER_CHAR = 8000;

export function buildPool(
  id: string,
  items: readonly TrainingItem[],
  layerWeight = 1,
): QuestionPool {
  let maxLogFrequency = 0;
  for (const item of items) {
    maxLogFrequency = Math.max(maxLogFrequency, Math.log1p(item.frequencyScore));
  }
  return { id, items, maxLogFrequency, layerWeight };
}

/** 迟疑增益:按汉字数归一的完成耗时越慢,越需要再练。 */
export function latencyBoost(progress: ItemProgress, charCount: number): number {
  if (progress.avgLatencyMs === null || charCount <= 0) return 1;
  const perChar = progress.avgLatencyMs / charCount;
  return 1 + Math.min(0.5, perChar / LATENCY_REF_MS_PER_CHAR);
}

/** 单候选的选题优先级。 */
export function computePriority(
  item: TrainingItem,
  progress: ItemProgress,
  maxLogFrequency: number,
  layerWeight = 1,
): number {
  const frequencyNorm =
    maxLogFrequency === 0
      ? 0
      : Math.log1p(item.frequencyScore) / maxLogFrequency;
  const frequencyBoost = 0.75 + frequencyNorm * 0.75;
  const weakness = 1 + progress.wrong * 3 + (100 - progress.mastery) / 20;
  const unseenBoost = progress.attempts === 0 ? 1.4 : 1;
  return (
    frequencyBoost *
    weakness *
    unseenBoost *
    latencyBoost(progress, item.charCount) *
    layerWeight
  );
}

/** 加权随机选题;excludeIds 中的条目会被跳过,全部被排除时回退到整池。 */
export function pickWeighted(
  pool: QuestionPool,
  progressById: ReadonlyMap<string, ItemProgress>,
  excludeIds: ReadonlySet<string>,
  rng: Rng,
): TrainingItem | null {
  if (pool.items.length === 0) return null;
  const candidates =
    excludeIds.size === 0
      ? pool.items
      : pool.items.filter((item) => !excludeIds.has(item.id));
  const source = candidates.length > 0 ? candidates : pool.items;

  const priorityOf = (item: TrainingItem): number =>
    computePriority(
      item,
      progressById.get(item.id) ?? emptyProgress(),
      pool.maxLogFrequency,
      pool.layerWeight,
    );

  let total = 0;
  for (const item of source) {
    total += priorityOf(item);
  }
  if (total <= 0) return source[0] ?? null;

  let draw = rng() * total;
  for (const item of source) {
    draw -= priorityOf(item);
    if (draw < 0) return item;
  }
  return source[source.length - 1] ?? null;
}

/** 答错题目的会话内回炉项。 */
export type ReviewItem = {
  id: string;
  dueIn: number;
};

const REVIEW_MIN_DELAY = 3;
const REVIEW_MAX_DELAY = 8;
export const RECENT_LIMIT = 5;

/** 把答错的题加入回炉队列,3-8 题后到期。 */
export function scheduleReview(
  queue: readonly ReviewItem[],
  id: string,
  rng: Rng,
): ReviewItem[] {
  const dueIn =
    REVIEW_MIN_DELAY +
    Math.floor(rng() * (REVIEW_MAX_DELAY - REVIEW_MIN_DELAY + 1));
  return [...queue.filter((item) => item.id !== id), { id, dueIn }];
}

export type PickNextInput = {
  mode: PracticeMode;
  pools: readonly QuestionPool[];
  progressById: ReadonlyMap<string, ItemProgress>;
  recentIds: readonly string[];
  reviewQueue: readonly ReviewItem[];
  mixedCursor: number;
  rng: Rng;
};

export type PickNextResult = {
  item: TrainingItem;
  reviewQueue: ReviewItem[];
  mixedCursor: number;
};

function findInPools(
  pools: readonly QuestionPool[],
  id: string,
): TrainingItem | null {
  for (const pool of pools) {
    const found = pool.items.find((item) => item.id === id);
    if (found) return found;
  }
  return null;
}

/**
 * 选出下一题。回炉题到期优先(除非就是上一题);否则按模式的池轮换
 * 加权随机,并避开最近出现过的题。
 */
export function pickNext(input: PickNextInput): PickNextResult | null {
  const { mode, pools, progressById, recentIds, mixedCursor, rng } = input;

  // 每出一题,回炉队列全部向前推进一格。
  let reviewQueue = input.reviewQueue.map((item) => ({
    ...item,
    dueIn: item.dueIn - 1,
  }));

  const lastId = recentIds[recentIds.length - 1] ?? null;
  const dueIndex = reviewQueue.findIndex(
    (item) => item.dueIn <= 0 && item.id !== lastId,
  );
  if (dueIndex >= 0) {
    const due = reviewQueue[dueIndex];
    if (due) {
      const item = findInPools(pools, due.id);
      reviewQueue = reviewQueue.filter((_, index) => index !== dueIndex);
      if (item) return { item, reviewQueue, mixedCursor };
    }
  }

  const rotation = MODE_POOL_ROTATION[mode];
  const excludeIds = new Set(recentIds.slice(-RECENT_LIMIT));

  // 从轮换光标开始依次尝试各池,跳过空池。
  for (let step = 0; step < rotation.length; step += 1) {
    const poolId = rotation[(mixedCursor + step) % rotation.length];
    const pool = pools.find((candidate) => candidate.id === poolId);
    if (!pool || pool.items.length === 0) continue;
    const item = pickWeighted(pool, progressById, excludeIds, rng);
    if (item) {
      return {
        item,
        reviewQueue,
        mixedCursor: (mixedCursor + step + 1) % rotation.length,
      };
    }
  }
  return null;
}
