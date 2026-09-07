/**
 * 键位混淆聚合(纯逻辑,确定性)。
 *
 * 记录「期望键 → 实际键 + 码位」三元组的紧凑累计,不是原始按键日志:
 * 每次错键产生一个三元组,同一三元组只累加计数。持久化为 Record,键由
 * {@link confusionId} 生成,格式:`${expected}>${actual}@${positionId}`,
 * 其中 positionId 为 sound1 / sound2 / shape1 / shape2,或 {other:N} 序列化
 * 为 oN(如第 5 位 → o5)。例:期望 k 按成 m、形 1 位 → "k>m@shape1"。
 *
 * 上限与合并语义(诚实记录):
 * - 计数本身不封顶;聚合表条目数封顶 {@link CONFUSION_MAP_CAP}(2000):
 *   超出后忽略新的三元组(已有条目计数继续累加),防止极端输入撑爆持久化;
 * - {@link mergeConfusionMaps} 为计数相加的并集(b 的条目按插入序并入,
 *   超上限的新条目丢弃);备份导入采用整体替换(与 progress/daily/keyErrors
 *   一致),不使用本函数——见 backup.ts 的导入校验;
 * - 词/句条目的码位没有音形语义:只有单字条目(kind = "char")的主练码
 *   才按 0→声1、1→声2、2→形1、3→形2 命名,其余一律降级为 {other:N},
 *   不臆测音形角色。
 */

import type { I18nKey } from "../i18n";
import type { TrainingItem } from "../data/trainer-index";

/** 码位:双拼声 1/声 2 与形 1/形 2,或超出四位的具名位置(第 N 位)。 */
export type CodePosition =
  | "sound1"
  | "sound2"
  | "shape1"
  | "shape2"
  | { other: number };

/** 单条键位混淆(紧凑聚合;不含任何按键序列)。 */
export type KeyConfusion = {
  /** 期望键(小写 a-z)。 */
  expected: string;
  /** 实际按成的键(小写 a-z)。 */
  actual: string;
  /** 出错码位。 */
  position: CodePosition;
  /** 累计次数。 */
  count: number;
};

/** 混淆聚合表:confusionId → 条目。 */
export type ConfusionMap = Record<string, KeyConfusion>;

/** 聚合表条目数上限:超出后忽略新的三元组(见模块注释)。 */
export const CONFUSION_MAP_CAP = 2000;

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

/** 码位校验(持久化边界用):具名四值或 { other: 非负整数 }。 */
export function isCodePosition(value: unknown): value is CodePosition {
  if (
    value === "sound1" ||
    value === "sound2" ||
    value === "shape1" ||
    value === "shape2"
  ) {
    return true;
  }
  return (
    isRecord(value) &&
    typeof value.other === "number" &&
    Number.isInteger(value.other) &&
    value.other >= 0
  );
}

/** 码位 → 稳定字符串(sound1|sound2|shape1|shape2|oN)。 */
export function positionId(position: CodePosition): string {
  return typeof position === "string" ? position : `o${position.other}`;
}

/** 混淆三元组的确定性键,格式 "k>m@shape1"(见模块注释)。 */
export function confusionId(
  expected: string,
  actual: string,
  position: CodePosition,
): string {
  return `${expected}>${actual}@${positionId(position)}`;
}

/**
 * 记录一次混淆:同 id 计数 +1,新条目受 {@link CONFUSION_MAP_CAP} 约束。
 * 纯函数:非法键(非单个小写 a-z)或超限时原样返回输入表。
 */
export function recordConfusion(
  map: ConfusionMap,
  expected: string,
  actual: string,
  position: CodePosition,
): ConfusionMap {
  if (!/^[a-z]$/.test(expected) || !/^[a-z]$/.test(actual)) return map;
  const id = confusionId(expected, actual, position);
  const known = map[id];
  if (known) {
    return { ...map, [id]: { ...known, count: known.count + 1 } };
  }
  if (Object.keys(map).length >= CONFUSION_MAP_CAP) return map;
  return { ...map, [id]: { expected, actual, position, count: 1 } };
}

/**
 * 合并两张聚合表:同 id 计数相加,b 的新条目按插入序并入,
 * 超出 {@link CONFUSION_MAP_CAP} 的新条目丢弃(见模块注释)。
 */
export function mergeConfusionMaps(a: ConfusionMap, b: ConfusionMap): ConfusionMap {
  const merged: ConfusionMap = { ...a };
  for (const [id, entry] of Object.entries(b)) {
    const known = merged[id];
    if (known) {
      merged[id] = { ...known, count: known.count + entry.count };
    } else if (Object.keys(merged).length < CONFUSION_MAP_CAP) {
      merged[id] = entry;
    }
  }
  return merged;
}

/**
 * 从训练条目 + 码位序号推导码位:单字条目的主练码按
 * 0→sound1、1→sound2、2→shape1、3→shape2、≥4→{other:i} 命名;
 * 一级简码/简码/词/句的码位语义随路线与结构变化,统一降级为
 * {other:i}(已知限制,诚实降级而非臆测)。
 */
export function positionForIndex(item: TrainingItem, index: number): CodePosition {
  if (item.kind === "char") {
    if (index === 0) return "sound1";
    if (index === 1) return "sound2";
    if (index === 2) return "shape1";
    if (index === 3) return "shape2";
  }
  return { other: index };
}

/** 确定性全排序:count 降序 → expected → actual → positionId。 */
export function sortedConfusions(map: ConfusionMap): KeyConfusion[] {
  return Object.values(map).sort(
    (a, b) =>
      b.count - a.count ||
      (a.expected < b.expected ? -1 : a.expected > b.expected ? 1 : 0) ||
      (a.actual < b.actual ? -1 : a.actual > b.actual ? 1 : 0) ||
      positionId(a.position).localeCompare(positionId(b.position)),
  );
}

/** 按次数取前 N 条混淆(确定性;见 sortedConfusions)。 */
export function topConfusions(map: ConfusionMap, limit: number): KeyConfusion[] {
  return sortedConfusions(map).slice(0, limit);
}

/** 与某键相关的混淆(期望键或实际键命中),排序同 sortedConfusions。 */
export function confusionsFor(
  map: ConfusionMap,
  key: string,
  limit?: number,
): KeyConfusion[] {
  const related = sortedConfusions(map).filter(
    (entry) => entry.expected === key || entry.actual === key,
  );
  return limit === undefined ? related : related.slice(0, limit);
}

/** 码位展示标签(i18n 键 + 插值;第 N 位用 confusion.position.other)。 */
export function positionLabel(position: CodePosition): {
  key: I18nKey;
  params?: Record<string, string | number>;
} {
  if (typeof position === "string") {
    return { key: `confusion.position.${position}` as I18nKey };
  }
  return { key: "confusion.position.other", params: { n: position.other } };
}

/**
 * 持久化校验边界(宽松,逐条降级):非法条目单独丢弃,绝不因个别
 * 损坏条目整体清空;超出上限的条目按插入序丢弃。
 */
export function sanitizeConfusionMap(value: unknown): ConfusionMap {
  if (!isRecord(value)) return {};
  const map: ConfusionMap = {};
  for (const entry of Object.values(value)) {
    if (!isRecord(entry)) continue;
    const { expected, actual, position, count } = entry;
    if (
      typeof expected !== "string" ||
      typeof actual !== "string" ||
      !/^[a-z]$/.test(expected) ||
      !/^[a-z]$/.test(actual) ||
      !isCodePosition(position) ||
      typeof count !== "number" ||
      !Number.isFinite(count) ||
      count < 0
    ) {
      continue;
    }
    if (Object.keys(map).length >= CONFUSION_MAP_CAP) break;
    map[confusionId(expected, actual, position)] = {
      expected,
      actual,
      position,
      count,
    };
  }
  return map;
}
