import type { I18nKey } from "@/lib/i18n";
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
// 默认「错误后显示」:练习时先自己回忆编码,答错才给出提示;
// 想要对照练习的用户可在设置中选择「始终显示」。
export const DEFAULT_HINT_MODE: HintMode = "on-error";
export const DEFAULT_DIFFICULTY: Difficulty = "daily";

/**
 * 键帽参考内容模式:控制屏显键盘上出现「哪类」教育信息。
 *
 * 与提示策略(何时显示答案类信息,HintMode)正交:参考内容永远不包含
 * 「下一正确键」高亮,高亮由提示策略单独门控,防止答案泄露回归。
 */
export type KeyRefMode =
  /** 按当前练习模式自动选择(默认)。 */
  | "contextual"
  | "none"
  /** 双拼参考:声母/韵母标签。 */
  | "double"
  /** 形码参考:代表字标签(来自规范数据聚合)。 */
  | "shape"
  /** 双拼 + 形码同时显示。 */
  | "both";

export const KEY_REF_MODES: readonly KeyRefMode[] = [
  "contextual",
  "none",
  "double",
  "shape",
  "both",
];
export const DEFAULT_KEY_REF_MODE: KeyRefMode = "contextual";

/** 键位触感反馈强度(Android 默认轻;桌面不支持时自动无效)。 */
export type HapticsMode = "off" | "light" | "medium";
export const HAPTICS_MODES: readonly HapticsMode[] = ["off", "light", "medium"];
export const DEFAULT_HAPTICS_MODE: HapticsMode = "light";

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

/** 模式标签走 i18n 字典(practice.mode*),渲染处用 t() 查找。 */
export const MODE_LABELS: Record<PracticeMode, I18nKey> = {
  double: "practice.modeDouble",
  "sound-shape": "practice.modeSoundShape",
  full: "practice.modeFull",
  mixed: "practice.modeMixed",
  level1: "practice.modeLevel1",
  "two-key-word": "practice.modeTwoKeyWord",
  "zero-regression": "practice.modeZeroRegression",
  "fixed-first": "practice.modeFixedFirst",
  "fixed-word": "practice.modeFixedWord",
  "mixed-shortcut": "practice.modeMixedShortcut",
  sentence: "practice.modeSentence",
  "mixed-all": "practice.modeMixedAll",
};

export const MODE_DESCRIPTIONS: Record<PracticeMode, I18nKey> = {
  double: "practice.descDouble",
  "sound-shape": "practice.descSoundShape",
  full: "practice.descFull",
  mixed: "practice.descMixed",
  level1: "practice.descLevel1",
  "two-key-word": "practice.descTwoKeyWord",
  "zero-regression": "practice.descZeroRegression",
  "fixed-first": "practice.descFixedFirst",
  "fixed-word": "practice.descFixedWord",
  "mixed-shortcut": "practice.descMixedShortcut",
  sentence: "practice.descSentence",
  "mixed-all": "practice.descMixedAll",
};

/** UI 默认展示的模式分组(完整模式选择器在产品 UX 版本展开)。 */
export const CHARMODE_GROUP: readonly PracticeMode[] = [
  "double",
  "sound-shape",
  "full",
  "mixed",
];

export const DIFFICULTY_LABELS: Record<Difficulty, I18nKey> = {
  beginner: "practice.diffBeginner",
  daily: "practice.diffDaily",
  full: "practice.diffFull",
};

export const HINT_MODE_LABELS: Record<HintMode, I18nKey> = {
  always: "practice.hintAlways",
  "on-delay": "practice.hintOnDelay",
  "on-error": "practice.hintOnError",
  hidden: "practice.hintHidden",
};

/** 一题的判定结果。 */
export type QuestionOutcome = "perfect" | "imperfect";

/** 一题完成时实际使用的输入路线(简码评分契约,B5)。 */
export type QuestionRoute = "primary" | "alternate";
