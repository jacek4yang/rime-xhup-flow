/**
 * 练习调度器(纯逻辑,不依赖 React,可注入 RNG 测试)。
 *
 * 选题优先级 = 词频增益 × 薄弱度 × 未见增益:
 *
 *   frequencyNorm  = maxLog === 0 ? 0 : log1p(score) / maxLog
 *   frequencyBoost = 0.75 + frequencyNorm * 0.75      (约 0.75..1.50)
 *   weakness       = 1 + wrong * 3 + (100 - mastery) / 20
 *   unseenBoost    = attempts === 0 ? 1.4 : 1
 *   priority       = frequencyBoost * weakness * unseenBoost
 *
 * 另有两条规则:最近 5 题防重复;答错的题 3-8 题后自然回炉(仅会话内,不持久化)。
 * mixed 模式按 2 → 3 → 4 码长均衡轮换,到期回炉题可临时打破轮换。
 */

import { itemId, type TrainerEntry } from "@/lib/trainer-data";
import { emptyProgress, type ItemProgress } from "@/lib/progress";
import type { CodeLength } from "@/lib/trainer-index";
import { MODE_LENGTHS, type PracticeMode } from "./types";

export type Rng = () => number;

/** 一个码长的候选池:题目列表 + 预计算的 log 词频上限。 */
export type QuestionPool = {
  length: CodeLength;
  entries: readonly TrainerEntry[];
  maxLogFrequency: number;
};

export function buildPool(
  length: CodeLength,
  entries: readonly TrainerEntry[],
): QuestionPool {
  let maxLogFrequency = 0;
  for (const entry of entries) {
    maxLogFrequency = Math.max(maxLogFrequency, Math.log1p(entry.frequencyScore));
  }
  return { length, entries, maxLogFrequency };
}

/** 单候选的选题优先级。 */
export function computePriority(
  entry: TrainerEntry,
  progress: ItemProgress,
  maxLogFrequency: number,
): number {
  const frequencyNorm =
    maxLogFrequency === 0 ? 0 : Math.log1p(entry.frequencyScore) / maxLogFrequency;
  const frequencyBoost = 0.75 + frequencyNorm * 0.75;
  const weakness = 1 + progress.wrong * 3 + (100 - progress.mastery) / 20;
  const unseenBoost = progress.attempts === 0 ? 1.4 : 1;
  return frequencyBoost * weakness * unseenBoost;
}

/** 加权随机选题;excludeIds 中的条目会被跳过,全部被排除时回退到整池。 */
export function pickWeighted(
  pool: QuestionPool,
  progressById: ReadonlyMap<string, ItemProgress>,
  excludeIds: ReadonlySet<string>,
  rng: Rng,
): TrainerEntry | null {
  if (pool.entries.length === 0) return null;
  const candidates =
    excludeIds.size === 0
      ? pool.entries
      : pool.entries.filter((entry) => !excludeIds.has(itemId(entry)));
  const source = candidates.length > 0 ? candidates : pool.entries;

  let total = 0;
  for (const entry of source) {
    total += computePriority(
      entry,
      progressById.get(itemId(entry)) ?? emptyProgress(),
      pool.maxLogFrequency,
    );
  }
  if (total <= 0) return source[0] ?? null;

  let draw = rng() * total;
  for (const entry of source) {
    draw -= computePriority(
      entry,
      progressById.get(itemId(entry)) ?? emptyProgress(),
      pool.maxLogFrequency,
    );
    if (draw < 0) return entry;
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
  entry: TrainerEntry;
  reviewQueue: ReviewItem[];
  mixedCursor: number;
};

function findInPools(
  pools: readonly QuestionPool[],
  id: string,
): TrainerEntry | null {
  for (const pool of pools) {
    const found = pool.entries.find((entry) => itemId(entry) === id);
    if (found) return found;
  }
  return null;
}

/**
 * 选出下一题。回炉题到期优先(除非就是上一题);否则按模式的码长轮换
 * 在对应池内加权随机,并避开最近出现过的题。
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
      const entry = findInPools(pools, due.id);
      reviewQueue = reviewQueue.filter((_, index) => index !== dueIndex);
      if (entry) return { entry, reviewQueue, mixedCursor };
    }
  }

  const lengths = MODE_LENGTHS[mode];
  const excludeIds = new Set(recentIds.slice(-RECENT_LIMIT));

  // 从轮换光标开始依次尝试各码长,跳过空池。
  for (let step = 0; step < lengths.length; step += 1) {
    const length = lengths[(mixedCursor + step) % lengths.length];
    const pool = pools.find((candidate) => candidate.length === length);
    if (!pool || pool.entries.length === 0) continue;
    const entry = pickWeighted(pool, progressById, excludeIds, rng);
    if (entry) {
      return {
        entry,
        reviewQueue,
        mixedCursor: (mixedCursor + step + 1) % lengths.length,
      };
    }
  }
  return null;
}
