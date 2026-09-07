/**
 * 错题/薄弱项查询(纯函数)。
 *
 * 只考虑用户实际见过且有错误历史的条目;排序:掌握度低 → 错误多 → 最近错过。
 */

import type { ItemProgress } from "./progress";
import type { TrainingItem, TrainerIndex } from "../data/trainer-index";

export type WeakListEntry = {
  item: TrainingItem;
  progress: ItemProgress;
};

export function listWeakItems(
  index: TrainerIndex,
  progressById: Record<string, ItemProgress>,
  limit?: number,
): WeakListEntry[] {
  const items: WeakListEntry[] = [];
  for (const [id, progress] of Object.entries(progressById)) {
    if (progress.attempts === 0 || progress.wrong === 0) continue;
    const item = index.byId.get(id);
    if (item) items.push({ item, progress });
  }
  items.sort(
    (a, b) =>
      a.progress.mastery - b.progress.mastery ||
      b.progress.wrong - a.progress.wrong ||
      (b.progress.lastSeenAt ?? 0) - (a.progress.lastSeenAt ?? 0),
  );
  return limit === undefined ? items : items.slice(0, limit);
}

/** 条目级准确率:完美次数 / 完成次数。 */
export function itemAccuracy(progress: ItemProgress): number | null {
  if (progress.attempts === 0) return null;
  return progress.correct / progress.attempts;
}

export function weakItemId(item: WeakListEntry): string {
  return item.item.id;
}
