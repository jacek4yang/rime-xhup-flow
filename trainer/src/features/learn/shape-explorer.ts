/**
 * 形码探索器聚合:把规范单字数据按「首形 / 次形」键位归组。
 *
 * 事实来源只有训练数据本身(全码第 3、4 位 = 首形/次形),不引入任何
 * 数据之外的字根表。纯函数;例字按频率证据排序。
 */

import type { TrainerEntry } from "@/lib/trainer-data";
import { charItem, compareByFrequency, type TrainingItem } from "@/lib/trainer-index";

/** 单个形键的聚合结果。 */
export type ShapeKeyStat = {
  /** 字母键。 */
  key: string;
  /** 以该键为首形的字数。 */
  firstCount: number;
  /** 以该键为次形的字数。 */
  secondCount: number;
  /** 首形高频例字(频率证据降序)。 */
  firstSamples: readonly { char: string; code: string }[];
  /** 次形高频例字(频率证据降序)。 */
  secondSamples: readonly { char: string; code: string }[];
};

/** 每键例字数(探索器展示用)。 */
export const SHAPE_SAMPLES_PER_KEY = 8;

/**
 * 聚合全码(4 码)单字:code[2] = 首形,code[3] = 次形。
 * 返回按字母键排序的聚合表(空组键也会出现,便于完整展示键位分布)。
 */
export function buildShapeKeyStats(
  entries: readonly TrainerEntry[],
  perKey: number = SHAPE_SAMPLES_PER_KEY,
): ShapeKeyStat[] {
  const first = new Map<string, TrainingItem[]>();
  const second = new Map<string, TrainingItem[]>();
  for (const entry of entries) {
    if (entry.length !== 4 || entry.code.length !== 4) continue;
    const item = charItem(entry);
    const firstKey = entry.code[2];
    const secondKey = entry.code[3];
    if (!first.has(firstKey)) first.set(firstKey, []);
    first.get(firstKey)!.push(item);
    if (!second.has(secondKey)) second.set(secondKey, []);
    second.get(secondKey)!.push(item);
  }
  const keys = new Set<string>([...first.keys(), ...second.keys()]);
  const samples = (bucket: readonly TrainingItem[] | undefined) =>
    (bucket ?? [])
      .slice()
      .sort(compareByFrequency)
      .slice(0, perKey)
      .map((item) => ({ char: item.target, code: item.primaryCode }));
  return [...keys]
    .sort()
    .map((key) => ({
      key,
      firstCount: first.get(key)?.length ?? 0,
      secondCount: second.get(key)?.length ?? 0,
      firstSamples: samples(first.get(key)),
      secondSamples: samples(second.get(key)),
    }));
}

/** 探索器默认展示的键子集:按组内总字数取前 N 个「信息量最大」的键。 */
export function topShapeKeys(stats: readonly ShapeKeyStat[], limit = 12): ShapeKeyStat[] {
  return [...stats]
    .sort((a, b) => b.firstCount + b.secondCount - (a.firstCount + a.secondCount))
    .slice(0, limit);
}
