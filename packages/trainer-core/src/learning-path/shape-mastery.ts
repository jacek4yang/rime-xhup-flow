/**
 * 形码掌握度分析(纯函数,确定性;里程碑 44)。
 *
 * 期望形键来自规范数据:单字全码(音 2 码 + 形 2 码)的第 3、4 位分别
 * 是首形(shape1)与次形(shape2);3 码条目只有首形。
 *
 * 数据来源分层(诚实记录,不虚构数据):
 * - confusionPairs:真实的「期望键 → 实际键」混淆对,来自引擎逐键捕获并
 *   持久化的 ConfusionMap(learning/confusion),过滤到形位(shape1/shape2)
 *   ——这是逐键准确信号,可信度高于条目级代理;
 * - shapeAccuracy 仍是条目级代理:见过的含形条目中「全程无错键完成」的
 *   占比,混入了音码位的失误,只能作参考而非逐键准确率;
 * - confusedKeys 是键级回退:没有混淆数据时(旧持久化/尚未错键)列出
 *   被误按的键并标注是否为规范形键;消费方应优先用 confusionPairs,
 *   为空时回退 confusedKeys(优雅降级)。
 */

import type { ItemProgress } from "../learning/progress";
import type { KeyConfusion, ConfusionMap } from "../learning/confusion";
import { sortedConfusions } from "../learning/confusion";
import type { TrainerIndex } from "../data/trainer-index";

/** 形键在编码中的位置角色。 */
export type ShapeRole = "shape1" | "shape2";

/** 单个薄弱形键(期望形键,错误集中在它身上)。 */
export type WeakShapeKey = {
  /** 小写字母键。 */
  key: string;
  /** 该键在已练条目中充当的形位(两者都有为 both)。 */
  role: "shape1" | "shape2" | "both";
  /** 期望该键作为形键的已练条目数(曝光)。 */
  exposure: number;
  /** 其中出现错键完成的条目数(条目级代理)。 */
  wrongItems: number;
  /** 该键的全局误按次数(keyErrors,跨音/形角色累计)。 */
  keyErrors: number;
  /** 条目级错误率 = wrongItems / exposure(曝光 0 时为 0)。 */
  wrongRate: number;
};

/**
 * 键级混淆嫌疑:被误按的键 + 它是否为规范数据中的形键。
 * 键级回退路径:没有混淆对数据时的优雅降级(见模块注释)。
 */
export type ConfusedKeyEntry = {
  /** 被误按的键(小写)。 */
  actual: string;
  /** 全局误按次数。 */
  count: number;
  /** 该键在规范数据中是否充当形键(首形或次形)。 */
  isShapeKey: boolean;
};

/** 真实混淆对(期望形键 → 实际键,来自 ConfusionMap 的形位条目)。 */
export type ShapeConfusionPair = {
  /** 期望形键(小写)。 */
  expected: string;
  /** 实际按成的键(小写)。 */
  actual: string;
  /** 形位(仅 shape1/shape2;音位条目不在此列)。 */
  position: "shape1" | "shape2";
  /** 累计次数。 */
  count: number;
};

export type ShapeMasteryReport = {
  /** 形位准确率代理(0..1;无样本为 null)。 */
  shapeAccuracy: { shape1: number | null; shape2: number | null };
  /** 薄弱形键(按 wrongRate → keyErrors → key 排序;至多 limit 条)。 */
  weakShapeKeys: WeakShapeKey[];
  /** 真实形位混淆对(按 count 降序;至多 limit 条;无数据时为空)。 */
  confusionPairs: ShapeConfusionPair[];
  /** 键级混淆嫌疑(按 count 降序;至多 limit 条;回退路径)。 */
  confusedKeys: ConfusedKeyEntry[];
};

export type ShapeMasteryInput = {
  index: TrainerIndex;
  /** 稀疏条目进度。 */
  progressById: Record<string, ItemProgress>;
  /** 按键累积错误(小写字母 → 次数)。 */
  keyErrors: Record<string, number>;
  /** 键位混淆聚合(可选;缺省视为尚无数据,走键级回退)。 */
  confusions?: ConfusionMap;
  /** 列表截断(默认 8)。 */
  limit?: number;
};

/** 从全码中取期望形键(长度不足的码返回空)。 */
export function expectedShapeKeys(primaryCode: string): {
  shape1: string | null;
  shape2: string | null;
} {
  return {
    shape1: primaryCode.length >= 3 ? primaryCode[2] : null,
    shape2: primaryCode.length >= 4 ? primaryCode[3] : null,
  };
}

function round3(value: number): number {
  return Math.round(value * 1000) / 1000;
}

/** 汇总形码掌握度报告(纯读;不修改任何输入)。 */
export function analyzeShapeMastery(input: ShapeMasteryInput): ShapeMasteryReport {
  const { index, progressById, keyErrors } = input;
  const limit = input.limit ?? 8;

  // 形位曝光/失误统计(条目级代理)。
  const stats = new Map<string, { shape1: number; shape2: number; wrong1: number; wrong2: number }>();
  const statOf = (key: string) => {
    let entry = stats.get(key);
    if (!entry) {
      entry = { shape1: 0, shape2: 0, wrong1: 0, wrong2: 0 };
      stats.set(key, entry);
    }
    return entry;
  };

  let cleanWithShape1 = 0;
  let totalWithShape1 = 0;
  let cleanWithShape2 = 0;
  let totalWithShape2 = 0;

  // 形键只出现在 3/4 码单字上;这两个池覆盖全部含形条目。
  for (const item of [...index.pools["char-3"], ...index.pools["char-4"]]) {
    const progress = progressById[item.id];
    if (!progress || progress.attempts === 0) continue;
    const expected = expectedShapeKeys(item.primaryCode);
    const clean = progress.wrong === 0;
    if (expected.shape1) {
      const stat = statOf(expected.shape1);
      stat.shape1 += 1;
      totalWithShape1 += 1;
      if (clean) cleanWithShape1 += 1;
      else stat.wrong1 += 1;
    }
    if (expected.shape2) {
      const stat = statOf(expected.shape2);
      stat.shape2 += 1;
      totalWithShape2 += 1;
      if (clean) cleanWithShape2 += 1;
      else stat.wrong2 += 1;
    }
  }

  const weakShapeKeys: WeakShapeKey[] = [];
  for (const [key, stat] of stats) {
    const exposure = stat.shape1 + stat.shape2;
    const wrongItems = stat.wrong1 + stat.wrong2;
    const keyErrorCount = keyErrors[key] ?? 0;
    // 只报告有错误信号的键:有条目级失误或该键被误按过。
    if (wrongItems === 0 && keyErrorCount === 0) continue;
    const role: WeakShapeKey["role"] =
      stat.shape1 > 0 && stat.shape2 > 0
        ? "both"
        : stat.shape1 > 0
          ? "shape1"
          : "shape2";
    weakShapeKeys.push({
      key,
      role,
      exposure,
      wrongItems,
      keyErrors: keyErrorCount,
      wrongRate: exposure === 0 ? 0 : round3(wrongItems / exposure),
    });
  }
  weakShapeKeys.sort(
    (a, b) =>
      b.wrongRate - a.wrongRate ||
      b.keyErrors - a.keyErrors ||
      a.key.localeCompare(b.key),
  );

  const confusedKeys: ConfusedKeyEntry[] = Object.entries(keyErrors)
    .filter(([key, count]) => /^[a-z]$/.test(key) && count > 0)
    .map(([key, count]) => ({
      actual: key,
      count,
      isShapeKey: stats.has(key),
    }))
    .sort((a, b) => b.count - a.count || a.actual.localeCompare(b.actual))
    .slice(0, limit);

  // 真实形位混淆对:只取具名 shape1/shape2 条目(sortedConfusions 已按
  // count 降序 + 确定性决胜);音位与 {other:N} 条目不属于形码分析。
  const confusionPairs: ShapeConfusionPair[] = (input.confusions
    ? sortedConfusions(input.confusions)
    : []
  )
    .filter(
      (entry): entry is KeyConfusion & { position: "shape1" | "shape2" } =>
        entry.position === "shape1" || entry.position === "shape2",
    )
    .map((entry) => ({
      expected: entry.expected,
      actual: entry.actual,
      position: entry.position,
      count: entry.count,
    }))
    .slice(0, limit);

  return {
    shapeAccuracy: {
      shape1: totalWithShape1 === 0 ? null : round3(cleanWithShape1 / totalWithShape1),
      shape2: totalWithShape2 === 0 ? null : round3(cleanWithShape2 / totalWithShape2),
    },
    weakShapeKeys: weakShapeKeys.slice(0, limit),
    confusionPairs,
    confusedKeys,
  };
}
