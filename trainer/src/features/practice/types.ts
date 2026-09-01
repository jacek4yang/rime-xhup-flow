import type { CodeLength, Difficulty } from "@/lib/trainer-index";

/** 四种练习模式。 */
export type PracticeMode = "double" | "sound-shape" | "full" | "mixed";

/** 提示方式。 */
export type HintMode = "always" | "on-error" | "hidden";

/** 会话目标题数;0 表示无限。 */
export type SessionLength = 20 | 30 | 50 | 100 | 0;

export const DEFAULT_SESSION_LENGTH: SessionLength = 30;
export const SESSION_LENGTH_OPTIONS: SessionLength[] = [20, 30, 50, 100, 0];

export const DEFAULT_MODE: PracticeMode = "double";
export const DEFAULT_HINT_MODE: HintMode = "always";
export const DEFAULT_DIFFICULTY: Difficulty = "daily";

/** 模式对应的码长集合;mixed 为 2/3/4 均衡轮换。 */
export const MODE_LENGTHS: Record<PracticeMode, readonly CodeLength[]> = {
  double: [2],
  "sound-shape": [3],
  full: [4],
  mixed: [2, 3, 4],
};

export const MODE_LABELS: Record<PracticeMode, string> = {
  double: "双拼",
  "sound-shape": "音形",
  full: "全码",
  mixed: "综合",
};

export const MODE_DESCRIPTIONS: Record<PracticeMode, string> = {
  double: "两字音码,熟悉小鹤双拼",
  "sound-shape": "双拼 + 首形,过渡到音形",
  full: "四位全码,核心肌肉记忆",
  mixed: "2 / 3 / 4 码均衡轮换",
};

export const DIFFICULTY_LABELS: Record<Difficulty, string> = {
  beginner: "入门",
  daily: "日常",
  full: "完整",
};

export const HINT_MODE_LABELS: Record<HintMode, string> = {
  always: "始终显示",
  "on-error": "错误后显示",
  hidden: "隐藏",
};

/** 一题的判定结果。 */
export type QuestionOutcome = "perfect" | "imperfect";
