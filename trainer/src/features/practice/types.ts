import type { CodeLength, Difficulty, PoolId } from "@/lib/trainer-index";

/**
 * 练习模式(V2)。
 *
 * 前四个值与 V1 完全兼容(单字练习;既有持久化 `lastMode` 与进度键不变);
 * 其余为 V2 新增的简码层 / 词语 / 组句模式。
 */
export type PracticeMode =
  /** 双拼(2 码单字)。 */
  | "double"
  /** 音形(3 码单字)。 */
  | "sound-shape"
  /** 全码(4 码单字)。 */
  | "full"
  /** 单字综合(2/3/4 码轮换)。 */
  | "mixed"
  /** 一级简码(备用合法码 = 该字全码)。 */
  | "level1"
  /** 二码零冲突词语简码。 */
  | "two-key-word"
  /** ZERO_REGRESSION 词语简码。 */
  | "zero-regression"
  /** FIXED_FIRST 词语简码。 */
  | "fixed-first"
  /** 固定词全码(2/3/4 字词轮换)。 */
  | "fixed-word"
  /** 简码综合(三个生产简码层轮换)。 */
  | "mixed-shortcut"
  /** 组句(连续输入整句拼接码)。 */
  | "sentence"
  /** 全模式综合(全部池轮换)。 */
  | "mixed-all";

/** 提示方式。on-delay:出题后迟疑一段时间自动显示(见 HINT_DELAY_MS)。 */
export type HintMode = "always" | "on-delay" | "on-error" | "hidden";

/** on-delay 提示的迟疑时长(活跃练习毫秒)。 */
export const HINT_DELAY_MS = 2000;

/** 会话目标题数;0 表示无限。 */
export type SessionLength = 20 | 30 | 50 | 100 | 0;

export const DEFAULT_SESSION_LENGTH: SessionLength = 30;
export const SESSION_LENGTH_OPTIONS: SessionLength[] = [20, 30, 50, 100, 0];

export const DEFAULT_MODE: PracticeMode = "double";
export const DEFAULT_HINT_MODE: HintMode = "always";
export const DEFAULT_DIFFICULTY: Difficulty = "daily";

/**
 * 模式 → 池轮换(均衡轮换顺序)。单字四模式沿用 V1 的 MODE_LENGTHS
 * 语义;新模式的池 ID 见 trainer-index 的 PoolId。
 */
export const MODE_POOL_ROTATION: Record<PracticeMode, readonly PoolId[]> = {
  double: ["char-2"],
  "sound-shape": ["char-3"],
  full: ["char-4"],
  mixed: ["char-2", "char-3", "char-4"],
  level1: ["level1"],
  "two-key-word": ["shortcut-two-key-zero-regression"],
  "zero-regression": ["shortcut-zero-regression"],
  "fixed-first": ["shortcut-fixed-first"],
  "fixed-word": ["word-4", "word-6", "word-8"],
  "mixed-shortcut": [
    "shortcut-zero-regression",
    "shortcut-fixed-first",
    "shortcut-two-key-zero-regression",
  ],
  sentence: ["sentence"],
  "mixed-all": [
    "char-2",
    "char-3",
    "char-4",
    "level1",
    "shortcut-zero-regression",
    "shortcut-fixed-first",
    "shortcut-two-key-zero-regression",
    "word-4",
    "word-6",
    "word-8",
    "sentence",
  ],
};

/** V1 兼容:模式对应的单字段码长集合(仅对单字四模式有意义)。 */
export const MODE_LENGTHS: Record<PracticeMode, readonly CodeLength[]> = {
  double: [2],
  "sound-shape": [3],
  full: [4],
  mixed: [2, 3, 4],
  level1: [2],
  "two-key-word": [2],
  "zero-regression": [2],
  "fixed-first": [2],
  "fixed-word": [2],
  "mixed-shortcut": [2],
  sentence: [2],
  "mixed-all": [2],
};

/** 单字模式判定(池构建与 UI 共用)。 */
export function isCharMode(mode: PracticeMode): boolean {
  return (
    mode === "double" ||
    mode === "sound-shape" ||
    mode === "full" ||
    mode === "mixed"
  );
}

export const MODE_LABELS: Record<PracticeMode, string> = {
  double: "双拼",
  "sound-shape": "音形",
  full: "全码",
  mixed: "单字综合",
  level1: "一级简码",
  "two-key-word": "二码词简码",
  "zero-regression": "零冲突词简码",
  "fixed-first": "固定首码词简码",
  "fixed-word": "固定词",
  "mixed-shortcut": "简码综合",
  sentence: "组句",
  "mixed-all": "全模式综合",
};

export const MODE_DESCRIPTIONS: Record<PracticeMode, string> = {
  double: "两字音码,熟悉小鹤双拼",
  "sound-shape": "双拼 + 首形,过渡到音形",
  full: "四位全码,核心肌肉记忆",
  mixed: "2 / 3 / 4 码均衡轮换",
  level1: "26 个一级简码,最高频入口",
  "two-key-word": "二键直达的零冲突词简码",
  "zero-regression": "高稳健零冲突词语简码",
  "fixed-first": "高稳健 FIXED_FIRST 词语简码",
  "fixed-word": "固定词全码(4 / 6 / 8 键轮换)",
  "mixed-shortcut": "三个生产简码层均衡轮换",
  sentence: "连续组句:整句拼接码连续输入",
  "mixed-all": "全部内容均衡轮换",
};

/** UI 默认展示的模式分组(完整模式选择器在产品 UX 版本展开)。 */
export const CHARMODE_GROUP: readonly PracticeMode[] = [
  "double",
  "sound-shape",
  "full",
  "mixed",
];

export const DIFFICULTY_LABELS: Record<Difficulty, string> = {
  beginner: "入门",
  daily: "日常",
  full: "完整",
};

export const HINT_MODE_LABELS: Record<HintMode, string> = {
  always: "始终显示",
  "on-delay": "迟疑后显示",
  "on-error": "错误后显示",
  hidden: "隐藏",
};

/** 一题的判定结果。 */
export type QuestionOutcome = "perfect" | "imperfect";

/** 一题完成时实际使用的输入路线(简码评分契约,B5)。 */
export type QuestionRoute = "primary" | "alternate";
