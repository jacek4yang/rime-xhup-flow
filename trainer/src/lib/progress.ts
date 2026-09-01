/**
 * 单条目学习进度与掌握度更新规则(纯函数)。
 *
 * 进度是稀疏的:只有用户实际见过的条目才有记录。
 * 掌握度是确定性的简单算法,用于打字肌肉记忆训练,不是间隔重复。
 */

export type ItemProgress = {
  /** 完成展示次数(无论完美与否)。 */
  attempts: number;
  /** 完美完成次数(全程无错键)。 */
  correct: number;
  /** 有误完成次数(过程中至少按错一个键)。 */
  wrong: number;
  /** 连续完美次数。 */
  streak: number;
  /** 掌握度 0..100。 */
  mastery: number;
  /** 最近一次完成时间戳;未见过为 null。 */
  lastSeenAt: number | null;
};

export function emptyProgress(): ItemProgress {
  return {
    attempts: 0,
    correct: 0,
    wrong: 0,
    streak: 0,
    mastery: 0,
    lastSeenAt: null,
  };
}

const MASTERY_MAX = 100;
const IMPERFECT_PENALTY = 15;

/** 完美完成:+1 次,+1 连对,掌握度按连对加速提升(上限 100)。 */
export function applyPerfect(
  progress: ItemProgress,
  now: number,
): ItemProgress {
  const streak = progress.streak + 1;
  return {
    attempts: progress.attempts + 1,
    correct: progress.correct + 1,
    wrong: progress.wrong,
    streak,
    mastery: Math.min(MASTERY_MAX, progress.mastery + Math.min(8, 3 + streak)),
    lastSeenAt: now,
  };
}

/** 有误完成:+1 次,连对清零,掌握度固定回落(下限 0)。 */
export function applyImperfect(
  progress: ItemProgress,
  now: number,
): ItemProgress {
  return {
    attempts: progress.attempts + 1,
    correct: progress.correct,
    wrong: progress.wrong + 1,
    streak: 0,
    mastery: Math.max(0, progress.mastery - IMPERFECT_PENALTY),
    lastSeenAt: now,
  };
}
