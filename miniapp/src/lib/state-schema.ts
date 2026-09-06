/**
 * 小程序应用状态 Schema(纯逻辑,不依赖 Taro)。
 *
 * 与桌面端 trainer store 的 V2 语义对齐:偏好 + 稀疏条目进度 + 按日统计
 * + 键位累积错误。持久化校验边界在这里:任何字段不可信就回退该字段的
 * 默认值,绝不整体清空用户进度;损坏/未知字段按字段降级。
 *
 * 本模块保持纯 TS(node 测试可直接运行);Taro 接线在 storage.ts/store.ts。
 */

import type {
  Difficulty,
  HintMode,
  ItemProgress,
} from "@xhup/trainer-core";
import {
  emptyDailyStats,
  emptyProgress,
  LANGUAGES,
  sanitizeConfusionMap,
  type ConfusionMap,
  type DailyStats,
  type ErrorTeachingMode,
  type HapticsMode,
  type KeyRefMode,
  type Language,
  type PracticeMode,
  type SessionLength,
} from "@xhup/trainer-core";

/** 持久化文档版本(V2:与桌面端语义一致)。 */
export const STORE_VERSION = 2;

/** 本机持久化键(带版本;旧 v1 进度键仅在迁移时读取)。 */
export const STORE_KEY = "xhup.miniapp.state.v2";
/** V1 旧进度键(只读迁移;新代码不再写入)。 */
export const LEGACY_PROGRESS_KEY = "xhup.miniapp.progress.v1";

/** 用户偏好(与桌面端偏好字段对齐;主题为桌面端概念,小程序不带)。 */
export type AppSettings = {
  language: Language;
  hintMode: HintMode;
  keyRefMode: KeyRefMode;
  haptics: HapticsMode;
  errorTeaching: ErrorTeachingMode;
  difficulty: Difficulty;
  sessionLength: SessionLength;
  lastMode: PracticeMode;
};

/** 小程序全量应用状态(持久化单位)。 */
export type AppState = {
  settings: AppSettings;
  /** 稀疏条目进度:`${id}` → 进度。 */
  progress: Record<string, ItemProgress>;
  /** 按本地日历日累计的统计。 */
  daily: Record<string, DailyStats>;
  /** 键位累积错误(小写字母键 → 次数)。 */
  keyErrors: Record<string, number>;
  /**
   * 键位混淆聚合(confusionId → 条目;与桌面端 V2 可选字段对齐)。
   * 旧数据缺省 → {};紧凑聚合而非原始按键日志,损坏条目逐条丢弃。
   */
  confusions: ConfusionMap;
};

export const DEFAULT_SETTINGS: AppSettings = {
  language: "zh",
  hintMode: "on-error",
  keyRefMode: "contextual",
  haptics: "light",
  errorTeaching: "adaptive",
  difficulty: "daily",
  sessionLength: 30,
  lastMode: "double",
};

export function defaultAppState(): AppState {
  return {
    settings: { ...DEFAULT_SETTINGS },
    progress: {},
    daily: {},
    keyErrors: {},
    confusions: {},
  };
}

/** 持久化文档外层形状(版本号 + 状态)。 */
export type PersistedDocument = { version: number; state: AppState };

const HINT_MODES: readonly HintMode[] = ["always", "on-delay", "on-error", "hidden"];
const KEY_REF_MODES: readonly KeyRefMode[] = [
  "contextual",
  "none",
  "double",
  "shape",
  "both",
];
const HAPTICS_MODES: readonly HapticsMode[] = ["off", "light", "medium"];
const ERROR_TEACHING_MODES: readonly ErrorTeachingMode[] = [
  "quick",
  "adaptive",
  "detailed",
];
const DIFFICULTIES: readonly Difficulty[] = ["beginner", "daily", "full"];
/** 与核心 SESSION_LENGTH_OPTIONS 一致(此处显式枚举以校验持久化值)。 */
const SESSION_LENGTHS: readonly SessionLength[] = [20, 30, 50, 100, 0];
/** 全部合法练习模式(V1 四模式 + V2 新模式)。 */
const PRACTICE_MODES: readonly PracticeMode[] = [
  "double",
  "sound-shape",
  "full",
  "mixed",
  "level1",
  "two-key-word",
  "zero-regression",
  "fixed-first",
  "fixed-word",
  "mixed-shortcut",
  "sentence",
  "mixed-all",
];

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function pickEnum<T extends string>(
  value: unknown,
  allowed: readonly T[],
  fallback: T,
): T {
  return typeof value === "string" && (allowed as readonly string[]).includes(value)
    ? (value as T)
    : fallback;
}

/** 单条进度校验:字段缺失/类型错误回退该字段默认,条目本身保留。 */
function sanitizeProgressEntry(value: unknown): ItemProgress | null {
  if (!isRecord(value)) return null;
  const attempts = value.attempts;
  if (typeof attempts !== "number" || !Number.isFinite(attempts) || attempts <= 0) {
    // 无有效完成次数的条目视为空进度,直接丢弃。
    return null;
  }
  const base = emptyProgress();
  const lastSeenAt =
    typeof value.lastSeenAt === "number" && Number.isFinite(value.lastSeenAt)
      ? value.lastSeenAt
      : base.lastSeenAt;
  const avgLatencyMs =
    typeof value.avgLatencyMs === "number" && Number.isFinite(value.avgLatencyMs) && value.avgLatencyMs >= 0
      ? value.avgLatencyMs
      : base.avgLatencyMs;
  return {
    attempts,
    correct: typeof value.correct === "number" && Number.isFinite(value.correct) ? value.correct : base.correct,
    wrong: typeof value.wrong === "number" && Number.isFinite(value.wrong) ? value.wrong : base.wrong,
    streak: typeof value.streak === "number" && Number.isFinite(value.streak) ? value.streak : base.streak,
    mastery: typeof value.mastery === "number" && Number.isFinite(value.mastery) ? value.mastery : base.mastery,
    lastSeenAt,
    avgLatencyMs,
  };
}

function sanitizeProgress(value: unknown): Record<string, ItemProgress> {
  if (!isRecord(value)) return {};
  const progress: Record<string, ItemProgress> = {};
  for (const [id, entry] of Object.entries(value)) {
    const sanitized = sanitizeProgressEntry(entry);
    if (sanitized) progress[id] = sanitized;
  }
  return progress;
}

function sanitizeDaily(value: unknown): Record<string, DailyStats> {
  if (!isRecord(value)) return {};
  const daily: Record<string, DailyStats> = {};
  for (const [dateKey, entry] of Object.entries(value)) {
    // 逐条校验:单日损坏只丢该日,不影响其他日期。
    daily[dateKey] = isRecord(entry)
      ? { ...emptyDailyStats(), ...pickNumbers(entry) }
      : emptyDailyStats();
  }
  return daily;
}

const DAILY_NUMBER_FIELDS = [
  "practiceMs",
  "questions",
  "keystrokes",
  "wrongKeyEvents",
  "bestStreak",
  "chars",
  "corrections",
] as const;

function pickNumbers(entry: Record<string, unknown>): Partial<DailyStats> {
  const picked: Partial<DailyStats> = {};
  for (const field of DAILY_NUMBER_FIELDS) {
    const value = entry[field];
    if (typeof value === "number" && Number.isFinite(value) && value >= 0) {
      picked[field] = value;
    }
  }
  return picked;
}

function sanitizeKeyErrors(value: unknown): Record<string, number> {
  if (!isRecord(value)) return {};
  const keyErrors: Record<string, number> = {};
  for (const [key, count] of Object.entries(value)) {
    if (/^[a-z]$/.test(key) && typeof count === "number" && Number.isFinite(count) && count >= 0) {
      keyErrors[key] = count;
    }
  }
  return keyErrors;
}

function sanitizeSettings(value: unknown): AppSettings {
  const defaults = DEFAULT_SETTINGS;
  if (!isRecord(value)) return { ...defaults };
  const sessionLength = SESSION_LENGTHS.includes(value.sessionLength as SessionLength)
    ? (value.sessionLength as SessionLength)
    : defaults.sessionLength;
  return {
    language: pickEnum(value.language, LANGUAGES, defaults.language),
    hintMode: pickEnum(value.hintMode, HINT_MODES, defaults.hintMode),
    keyRefMode: pickEnum(value.keyRefMode, KEY_REF_MODES, defaults.keyRefMode),
    haptics: pickEnum(value.haptics, HAPTICS_MODES, defaults.haptics),
    errorTeaching: pickEnum(value.errorTeaching, ERROR_TEACHING_MODES, defaults.errorTeaching),
    difficulty: pickEnum(value.difficulty, DIFFICULTIES, defaults.difficulty),
    sessionLength,
    lastMode: pickEnum(value.lastMode, PRACTICE_MODES, defaults.lastMode),
  };
}

/**
 * 持久化校验边界:逐字段降级,损坏字段回退默认值,绝不整体清空。
 * 接受未知版本(仍按当前语义尽力恢复可识别字段)。
 */
export function sanitizePersisted(value: unknown): AppState {
  if (!isRecord(value)) return defaultAppState();
  const state = isRecord(value.state) ? value.state : value;
  return {
    settings: sanitizeSettings(state.settings),
    progress: sanitizeProgress(state.progress),
    daily: sanitizeDaily(state.daily),
    keyErrors: sanitizeKeyErrors(state.keyErrors),
    // 混淆聚合走共享核心的同一校验边界(逐条降级,上限由核心约束)。
    confusions: sanitizeConfusionMap(state.confusions),
  };
}

/**
 * 解析持久化 JSON 文本;返回 null 表示没有可恢复的文档(调用方用默认值)。
 * JSON 损坏 / 顶层结构非法 → null(调用方记录证据)。
 */
export function parsePersistedDocument(raw: string | null): AppState | null {
  if (raw === null || raw === "") return null;
  let parsed: unknown;
  try {
    parsed = JSON.parse(raw);
  } catch {
    return null;
  }
  if (!isRecord(parsed)) return null;
  return sanitizePersisted(parsed);
}

/** 序列化为持久化文档(带版本号)。 */
export function serializeDocument(state: AppState): string {
  const document: PersistedDocument = { version: STORE_VERSION, state };
  return JSON.stringify(document);
}

/**
 * V1 → V2 迁移:旧版只有稀疏进度;逐条走同一校验边界。
 * 不可信条目丢弃,可恢复条目保留(avgLatencyMs 等新字段取默认)。
 */
export function migrateLegacyProgress(value: unknown): Record<string, ItemProgress> {
  return sanitizeProgress(value);
}
