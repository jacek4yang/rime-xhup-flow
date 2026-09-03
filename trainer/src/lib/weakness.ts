/**
 * 弱点聚合(纯逻辑,确定性;V2)。
 *
 * 从稀疏进度 + 键位错误统计聚合出弱点视图,供错题重练与后续 UX:
 * - 条目级:按「错误率 × 掌握度缺口」排序的薄弱条目;
 * - 维度级:练习模式 / 码长 / 条目种类的错误率;
 * - 键位级:按键聚合的累积错误次数(热力图数据源);
 * - 最近错题:有错且最近见过的条目。
 *
 * 不修改任何 canonical 数据;只读进度。
 */

import type { ItemProgress } from "@/lib/progress";
import type { TrainingItem, TrainerIndex } from "@/lib/trainer-index";

/** 单个薄弱条目的视图。 */
export type WeakItem = {
  id: string;
  target: string;
  kind: TrainingItem["kind"];
  attempts: number;
  wrong: number;
  mastery: number;
  /** 错误率 = wrong / attempts(0..1)。 */
  wrongRate: number;
  /** 排序分:错误率 × 掌握度缺口(越大越优先重练)。 */
  score: number;
};

/** 维度聚合(模式 / 码长 / 种类通用)。 */
export type DimensionStat = {
  attempts: number;
  wrong: number;
  wrongRate: number;
  items: number;
};

export type WeaknessReport = {
  /** 薄弱条目(按 score 降序;至多 limit 条)。 */
  items: WeakItem[];
  /** 按条目种类聚合。 */
  byKind: Record<TrainingItem["kind"], DimensionStat>;
  /** 按主练码键数聚合。 */
  byCodeLength: Record<number, DimensionStat>;
  /** 按键聚合的累积错误次数(含 0 错的键省略;来自 store 的 keyErrors)。 */
  keyErrors: Record<string, number>;
  /** 最近出错的条目(按 lastSeenAt 降序;至多 limit 条)。 */
  recentMistakes: WeakItem[];
};

function dimension(): DimensionStat {
  return { attempts: 0, wrong: 0, wrongRate: 0, items: 0 };
}

function accumulate(
  stats: DimensionStat,
  progress: ItemProgress,
): void {
  stats.attempts += progress.attempts;
  stats.wrong += progress.wrong;
  stats.items += 1;
}

function finalize(stats: DimensionStat): DimensionStat {
  stats.wrongRate =
    stats.attempts === 0 ? 0 : Math.round((stats.wrong / stats.attempts) * 1000) / 1000;
  return stats;
}

function weakItemOf(
  item: TrainingItem,
  progress: ItemProgress,
): WeakItem {
  const wrongRate =
    progress.attempts === 0 ? 0 : progress.wrong / progress.attempts;
  return {
    id: item.id,
    target: item.target,
    kind: item.kind,
    attempts: progress.attempts,
    wrong: progress.wrong,
    mastery: progress.mastery,
    wrongRate: Math.round(wrongRate * 1000) / 1000,
    score: Math.round(wrongRate * (100 - progress.mastery) * 100) / 100,
  };
}

/** 聚合弱点报告。limit 控制条目列表长度(默认 20)。 */
export function aggregateWeakness(
  index: TrainerIndex,
  progress: ReadonlyMap<string, ItemProgress>,
  keyErrors: Readonly<Record<string, number>>,
  limit = 20,
): WeaknessReport {
  const byKind: Record<TrainingItem["kind"], DimensionStat> = {
    char: dimension(),
    level1: dimension(),
    word: dimension(),
    shortcut: dimension(),
    sentence: dimension(),
  };
  const byCodeLength: Record<number, DimensionStat> = {};
  const weakItems: WeakItem[] = [];
  const recentMistakes: { item: WeakItem; seenAt: number }[] = [];

  for (const [id, item] of index.byId) {
    const itemProgress = progress.get(id);
    if (!itemProgress || itemProgress.attempts === 0) continue;

    accumulate(byKind[item.kind], itemProgress);
    const lengthStats = byCodeLength[item.codeLength] ?? dimension();
    accumulate(lengthStats, itemProgress);
    byCodeLength[item.codeLength] = lengthStats;

    const weak = weakItemOf(item, itemProgress);
    if (itemProgress.wrong > 0) {
      weakItems.push(weak);
      recentMistakes.push({ item: weak, seenAt: itemProgress.lastSeenAt ?? 0 });
    }
  }

  weakItems.sort(
    (a, b) => b.score - a.score || a.id.localeCompare(b.id),
  );
  recentMistakes.sort((a, b) => b.seenAt - a.seenAt || a.item.id.localeCompare(b.item.id));

  return {
    items: weakItems.slice(0, limit),
    byKind: Object.fromEntries(
      Object.entries(byKind).map(([kind, stats]) => [kind, finalize(stats)]),
    ) as Record<TrainingItem["kind"], DimensionStat>,
    byCodeLength: Object.fromEntries(
      Object.entries(byCodeLength).map(([length, stats]) => [
        Number(length),
        finalize(stats),
      ]),
    ),
    keyErrors: { ...keyErrors },
    recentMistakes: recentMistakes.slice(0, limit).map((entry) => entry.item),
  };
}

/** 当前键位错误热力(按 QWERTY 小写键归一;供热力图渲染)。 */
export function keyHeatmap(
  keyErrors: Readonly<Record<string, number>>,
): Record<string, number> {
  const heat: Record<string, number> = {};
  for (const [key, count] of Object.entries(keyErrors)) {
    if (/^[a-z]$/.test(key) && count > 0) {
      heat[key] = count;
    }
  }
  return heat;
}
