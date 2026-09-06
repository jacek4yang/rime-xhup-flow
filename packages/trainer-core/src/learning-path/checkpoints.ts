/**
 * 里程碑检查点(纯函数,确定性;里程碑 44)。
 *
 * 五个检查点(入门/基础/进阶/高级/精通)各由 2-3 个可度量组件构成,
 * 全部从本机证据(稀疏条目进度 + 章节学习证据)评估:
 * - 组件进度 = min(1, 当前值 / 目标值),四舍五入到 3 位小数;
 * - 检查点进度 = 组件进度的平均值;
 * - 达成 = 全部组件进度 ≥ 1。
 *
 * 检查点是建议性的(ADVISORY):只用于展示与激励,永不锁课、永不拦截
 * 任何练习入口。组件标签为规范中文(与学习中心章节内容同一策略,即
 * canonical 内容不进 i18n 表;展示层如需译制再行接key)。
 */

import type { ItemProgress } from "../learning/progress";
import type { TrainingItem, TrainerIndex } from "../data/trainer-index";
import { LEARN_CHAPTERS } from "../lessons/content";
import type { LessonEvidence } from "./lesson-state";

export type CheckpointId =
  | "beginner"
  | "foundation"
  | "intermediate"
  | "advanced"
  | "mastery";

/** 检查点的单个可度量组件。 */
export type CheckpointComponent = {
  /** 组件名(规范中文)。 */
  label: string;
  /** 当前进度 0..1。 */
  progress: number;
};

export type CheckpointResult = {
  id: CheckpointId;
  /** 达成 = 全部组件进度 ≥ 1。 */
  achieved: boolean;
  /** 总进度 = 组件进度均值(0..1,3 位小数)。 */
  progress: number;
  components: CheckpointComponent[];
};

export type CheckpointInput = {
  index: TrainerIndex;
  /** 稀疏条目进度。 */
  progressById: Record<string, ItemProgress>;
  /** 章节学习证据。 */
  lessonEvidence: Record<string, LessonEvidence>;
  now: number;
};

function round3(value: number): number {
  return Math.round(value * 1000) / 1000;
}

type KindStat = {
  attempted: number;
  totalMastery: number;
  correct: number;
  attempts: number;
};

/** 按 kind(可选再按单字段码长)统计已练条目。 */
function kindStat(
  input: CheckpointInput,
  kind: TrainingItem["kind"],
  codeLength?: number,
): KindStat {
  const stat: KindStat = { attempted: 0, totalMastery: 0, correct: 0, attempts: 0 };
  for (const [id, progress] of Object.entries(input.progressById)) {
    if (progress.attempts === 0) continue;
    const item = input.index.byId.get(id);
    if (!item || item.kind !== kind) continue;
    if (codeLength !== undefined && item.codeLength !== codeLength) continue;
    stat.attempted += 1;
    stat.totalMastery += progress.mastery;
    stat.correct += progress.correct;
    stat.attempts += progress.attempts;
  }
  return stat;
}

/** 组件进度 = min(1, 当前值 / 目标值)。 */
function coverage(value: number, target: number): number {
  return round3(Math.min(1, target <= 0 ? 1 : value / target));
}

/** 章节被练习过 = 该章节证据里 practiceSessions > 0。 */
function practicedChapters(input: CheckpointInput): number {
  let count = 0;
  for (const chapter of LEARN_CHAPTERS) {
    const evidence = input.lessonEvidence[chapter.id];
    if (evidence && evidence.practiceSessions > 0) count += 1;
  }
  return count;
}

function checkpointOf(
  id: CheckpointId,
  components: CheckpointComponent[],
): CheckpointResult {
  const progress = round3(
    components.reduce((sum, c) => sum + Math.max(0, Math.min(1, c.progress)), 0) /
      components.length,
  );
  return {
    id,
    achieved: components.every((c) => c.progress >= 1),
    progress,
    components,
  };
}

/** 平均掌握度进度(已练条目均值的代理;无样本为 0)。 */
function masteryProgress(stat: KindStat, target: number): number {
  const avg = stat.attempted === 0 ? 0 : stat.totalMastery / stat.attempted;
  return coverage(avg, target);
}

/**
 * 评估全部检查点(按 beginner → mastery 顺序返回)。
 * 阈值为确定性常量;相同输入必然得到相同输出。
 */
export function evaluateCheckpoints(input: CheckpointInput): CheckpointResult[] {
  const double = kindStat(input, "char", 2);
  const soundShape = kindStat(input, "char", 3);
  const fullCode = kindStat(input, "char", 4);
  const level1 = kindStat(input, "level1");
  const word = kindStat(input, "word");
  const shortcut = kindStat(input, "shortcut");
  const sentence = kindStat(input, "sentence");
  // 精通层覆盖全部条目种类(单字 + 一级简码 + 词 + 词语简码 + 组句)。
  const all = kindStat(input, "char");
  for (const stat of [level1, word, shortcut, sentence]) {
    all.attempted += stat.attempted;
    all.correct += stat.correct;
    all.attempts += stat.attempts;
  }
  const overallAccuracy =
    all.attempts === 0 ? 0 : all.correct / all.attempts;

  // 基础章节(基础层)中带练习入口的章节:shape、shape-memory。
  const basicPracticeChapters = LEARN_CHAPTERS.filter(
    (chapter) =>
      chapter.level === "basic" &&
      chapter.sections.some((section) => section.kind === "practice"),
  );
  const basicPracticed = basicPracticeChapters.filter(
    (chapter) => input.lessonEvidence[chapter.id]?.practiceSessions,
  ).length;

  return [
    checkpointOf("beginner", [
      { label: "双拼条目练过 20 个", progress: coverage(double.attempted, 20) },
      { label: "双拼平均掌握 70", progress: masteryProgress(double, 70) },
    ]),
    checkpointOf("foundation", [
      { label: "音形条目练过 20 个", progress: coverage(soundShape.attempted, 20) },
      { label: "音形平均掌握 70", progress: masteryProgress(soundShape, 70) },
      {
        label: "基础章节动手练过",
        progress: coverage(basicPracticed, basicPracticeChapters.length),
      },
    ]),
    checkpointOf("intermediate", [
      { label: "全码条目练过 30 个", progress: coverage(fullCode.attempted, 30) },
      { label: "全码平均掌握 70", progress: masteryProgress(fullCode, 70) },
      { label: "一级简码练过 20 个", progress: coverage(level1.attempted, 20) },
      { label: "一级简码平均掌握 80", progress: masteryProgress(level1, 80) },
    ]),
    checkpointOf("advanced", [
      { label: "固定词练过 20 个", progress: coverage(word.attempted, 20) },
      { label: "固定词平均掌握 70", progress: masteryProgress(word, 70) },
      { label: "词语简码练过 10 个", progress: coverage(shortcut.attempted, 10) },
      { label: "组句练过 5 句", progress: coverage(sentence.attempted, 5) },
    ]),
    checkpointOf("mastery", [
      { label: "全部条目练过 100 个", progress: coverage(all.attempted, 100) },
      {
        label: "总体完成准确率 95%",
        progress: coverage(overallAccuracy, 0.95),
      },
      {
        label: "各章节都动手练过",
        progress: coverage(practicedChapters(input), LEARN_CHAPTERS.length),
      },
    ]),
  ];
}
