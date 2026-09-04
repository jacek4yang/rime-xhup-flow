/**
 * 训练器持久化状态(zustand + localStorage)。
 *
 * 只持久化小型稀疏数据:偏好、见过的条目进度、按日统计、键位错误。
 * 规范数据集、当前题、已输入、回炉队列等运行态一律不进 store。
 *
 * 持久化版本 2(V2):相对 V1 新增 `DailyStats.chars/corrections` 与
 * `keyErrors`;V1 状态经 migrate 逐字段迁移(进度/统计/偏好保留,
 * 新字段回默认值),未知/损坏字段在校验边界逐字段回退为默认值。
 */

import { create } from "zustand";
import { persist } from "zustand/middleware";
import {
  applyImperfect,
  applyPerfect,
  emptyProgress,
  type ItemProgress,
} from "@/lib/progress";
import { localDateKey } from "@/lib/stats";
import { LANGUAGES, type Language } from "@/lib/i18n";
import { DEFAULT_THEME, type ThemePreference } from "@/lib/theme";
import type { Difficulty } from "@/lib/trainer-index";
import type { BackupSettings } from "@/lib/backup";
import {
  DEFAULT_DIFFICULTY,
  DEFAULT_HINT_MODE,
  DEFAULT_HAPTICS_MODE,
  DEFAULT_KEY_REF_MODE,
  DEFAULT_ERROR_TEACHING,
  DEFAULT_MODE,
  DEFAULT_SESSION_LENGTH,
  ERROR_TEACHING_MODES,
  HAPTICS_MODES,
  KEY_REF_MODES,
  type ErrorTeachingMode,
  type HapticsMode,
  type KeyRefMode,
  SESSION_LENGTH_OPTIONS,
  type HintMode,
  type PracticeMode,
  type QuestionOutcome,
  type QuestionRoute,
  type SessionLength,
} from "@/features/practice/types";

/** 按本地日历日累计的统计。 */
export type DailyStats = {
  practiceMs: number;
  questions: number;
  keystrokes: number;
  wrongKeyEvents: number;
  bestStreak: number;
  /** 完成的汉字数(V2;CPM 分母)。 */
  chars: number;
  /** 退格修正次数(V2)。 */
  corrections: number;
};

export function emptyDailyStats(): DailyStats {
  return {
    practiceMs: 0,
    questions: 0,
    keystrokes: 0,
    wrongKeyEvents: 0,
    bestStreak: 0,
    chars: 0,
    corrections: 0,
  };
}

export type TrainerData = {
  language: Language;
  theme: ThemePreference;
  hintMode: HintMode;
  /** 键帽参考内容模式(与提示策略正交;见 KeyRefMode)。 */
  keyRefMode: KeyRefMode;
  /** 键位触感反馈(off/light/medium;桌面不支持时自动无效)。 */
  keyHaptics: HapticsMode;
  /** 错误教学深度(quick/adaptive/detailed)。 */
  errorTeaching: ErrorTeachingMode;
  difficulty: Difficulty;
  sessionLength: SessionLength;
  lastMode: PracticeMode;
  /** 稀疏进度:只有实际见过的条目才有记录。 */
  progress: Record<string, ItemProgress>;
  /** 按日统计:YYYY-MM-DD → DailyStats。 */
  daily: Record<string, DailyStats>;
  /** 键位累积错误(小写字母 → 次数;V2,弱点热力图数据源)。 */
  keyErrors: Record<string, number>;
};

export type QuestionResultPayload = {
  id: string;
  outcome: QuestionOutcome;
  /** 实际输入路线(V2;alternate = 全码完成,不算掌握)。 */
  routeUsed: QuestionRoute;
  keystrokes: number;
  wrongKeyEvents: number;
  /** 本题按错的键(去重;键位统计用)。 */
  wrongKeys: string[];
  /** 完成汉字数(组句 > 1)。 */
  chars: number;
  corrections: number;
  practiceMs: number;
  bestStreak: number;
  now: number;
};

export type TrainerActions = {
  setLanguage: (language: Language) => void;
  setTheme: (theme: ThemePreference) => void;
  setHintMode: (hintMode: HintMode) => void;
  setKeyRefMode: (keyRefMode: KeyRefMode) => void;
  setKeyHaptics: (keyHaptics: HapticsMode) => void;
  setErrorTeaching: (errorTeaching: ErrorTeachingMode) => void;
  setDifficulty: (difficulty: Difficulty) => void;
  setSessionLength: (sessionLength: SessionLength) => void;
  setLastMode: (lastMode: PracticeMode) => void;
  /** 一题完成:更新条目进度 + 当日统计 + 键位错误(低频写入,每题一次)。 */
  recordQuestionResult: (payload: QuestionResultPayload) => void;
  /** 暂停/结束时结清的纯练习时长。 */
  addPracticeTime: (practiceMs: number, now: number) => void;
  /** 导入备份:整体替换进度/统计/偏好(主题与语言保留本地值)。 */
  applyBackup: (backup: {
    settings: BackupSettings;
    progress: Record<string, ItemProgress>;
    daily: Record<string, DailyStats>;
    keyErrors: Record<string, number>;
  }) => void;
  /** 重置指定条目的掌握度(弱点中心操作;保留偏好)。 */
  resetItemProgress: (ids: readonly string[]) => void;
  /** 重置学习进度与练习偏好;保留主题。 */
  resetProgress: () => void;
};

export type TrainerStore = TrainerData & TrainerActions;

export const STORAGE_KEY = "xhup-flow.trainer.v2";
export const STORAGE_VERSION = 2;

function defaultData(): TrainerData {
  return {
    language: "zh",
    theme: DEFAULT_THEME,
    hintMode: DEFAULT_HINT_MODE,
    keyRefMode: DEFAULT_KEY_REF_MODE,
    keyHaptics: DEFAULT_HAPTICS_MODE,
    errorTeaching: DEFAULT_ERROR_TEACHING,
    difficulty: DEFAULT_DIFFICULTY,
    sessionLength: DEFAULT_SESSION_LENGTH,
    lastMode: DEFAULT_MODE,
    progress: {},
    daily: {},
    keyErrors: {},
  };
}

function mergeDaily(
  daily: Record<string, DailyStats>,
  dateKey: string,
  update: (stats: DailyStats) => DailyStats,
): Record<string, DailyStats> {
  const current = daily[dateKey] ?? emptyDailyStats();
  return { ...daily, [dateKey]: update(current) };
}

// ---------- 持久化数据校验边界 ----------

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function isNumber(value: unknown): value is number {
  return typeof value === "number" && Number.isFinite(value);
}

function isProgress(value: unknown): value is ItemProgress {
  return (
    isRecord(value) &&
    isNumber(value.attempts) &&
    isNumber(value.correct) &&
    isNumber(value.wrong) &&
    isNumber(value.streak) &&
    isNumber(value.mastery) &&
    (value.lastSeenAt === null || isNumber(value.lastSeenAt)) &&
    (value.avgLatencyMs === null || isNumber(value.avgLatencyMs))
  );
}

function isKeyErrorCount(value: unknown): value is number {
  return isNumber(value);
}

function isDailyStats(value: unknown): value is DailyStats {
  return (
    isRecord(value) &&
    isNumber(value.practiceMs) &&
    isNumber(value.questions) &&
    isNumber(value.keystrokes) &&
    isNumber(value.wrongKeyEvents) &&
    isNumber(value.bestStreak) &&
    isNumber(value.chars) &&
    isNumber(value.corrections)
  );
}

const THEMES: readonly ThemePreference[] = ["system", "light", "dark"];
const HINT_MODES: readonly HintMode[] = ["always", "on-delay", "on-error", "hidden"];
const DIFFICULTIES: readonly Difficulty[] = ["beginner", "daily", "full"];
/** V1 四模式 + V2 新模式(全部可作为 lastMode 合法值)。 */
const MODES: readonly PracticeMode[] = [
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

function pickEnum<T extends string>(
  value: unknown,
  allowed: readonly T[],
  fallback: T,
): T {
  return typeof value === "string" && (allowed as readonly string[]).includes(value)
    ? (value as T)
    : fallback;
}

function pickRecord<V>(value: unknown, guard: (entry: unknown) => entry is V): Record<string, V> {
  if (!isRecord(value)) return {};
  const result: Record<string, V> = {};
  for (const [key, entry] of Object.entries(value)) {
    if (guard(entry)) result[key] = entry;
  }
  return result;
}

/** V1 单条进度 → V2(补 avgLatencyMs 默认)。 */
function migrateProgressV1(value: unknown): Record<string, ItemProgress> {
  if (!isRecord(value)) return {};
  const result: Record<string, ItemProgress> = {};
  for (const [id, entry] of Object.entries(value)) {
    if (!isRecord(entry) || !isNumber(entry.attempts) || entry.attempts <= 0) {
      continue;
    }
    result[id] = {
      attempts: entry.attempts,
      correct: isNumber(entry.correct) ? entry.correct : 0,
      wrong: isNumber(entry.wrong) ? entry.wrong : 0,
      streak: isNumber(entry.streak) ? entry.streak : 0,
      mastery: isNumber(entry.mastery) ? entry.mastery : 0,
      lastSeenAt: isNumber(entry.lastSeenAt) ? entry.lastSeenAt : null,
      avgLatencyMs: null,
    };
  }
  return result;
}

/** V1 按日统计 → V2(补 chars/corrections 默认)。 */
function migrateDailyV1(value: unknown): Record<string, DailyStats> {
  if (!isRecord(value)) return {};
  const result: Record<string, DailyStats> = {};
  for (const [dateKey, entry] of Object.entries(value)) {
    if (
      !isRecord(entry) ||
      !isNumber(entry.practiceMs) ||
      !isNumber(entry.questions)
    ) {
      continue;
    }
    result[dateKey] = {
      practiceMs: entry.practiceMs,
      questions: entry.questions,
      keystrokes: isNumber(entry.keystrokes) ? entry.keystrokes : 0,
      wrongKeyEvents: isNumber(entry.wrongKeyEvents) ? entry.wrongKeyEvents : 0,
      bestStreak: isNumber(entry.bestStreak) ? entry.bestStreak : 0,
      chars: 0,
      corrections: 0,
    };
  }
  return result;
}

/** 逐字段校验持久化数据;任何字段不可信就回退该字段默认值。 */
export function sanitizePersisted(value: unknown): TrainerData {
  const defaults = defaultData();
  if (!isRecord(value)) return defaults;
  const sessionLength = SESSION_LENGTH_OPTIONS.includes(
    value.sessionLength as SessionLength,
  )
    ? (value.sessionLength as SessionLength)
    : defaults.sessionLength;
  return {
    language: pickEnum(value.language, LANGUAGES, defaults.language),
    theme: pickEnum(value.theme, THEMES, defaults.theme),
    hintMode: pickEnum(value.hintMode, HINT_MODES, defaults.hintMode),
    keyRefMode: pickEnum(value.keyRefMode, KEY_REF_MODES, defaults.keyRefMode),
    keyHaptics: pickEnum(value.keyHaptics, HAPTICS_MODES, defaults.keyHaptics),
    errorTeaching: pickEnum(
      value.errorTeaching,
      ERROR_TEACHING_MODES,
      defaults.errorTeaching,
    ),
    difficulty: pickEnum(value.difficulty, DIFFICULTIES, defaults.difficulty),
    sessionLength,
    lastMode: pickEnum(value.lastMode, MODES, defaults.lastMode),
    progress: pickRecord(value.progress, isProgress),
    daily: pickRecord(value.daily, isDailyStats),
    keyErrors: Object.fromEntries(
      Object.entries(
        pickRecord(value.keyErrors, isKeyErrorCount),
      ).filter(([key]) => /^[a-z]$/.test(key)),
    ),
  };
}

/**
 * zustand persist 版本迁移:v1 → v2 显式迁移(进度/统计/偏好保留,
 * 新字段取默认值);其余版本交给校验边界兜底。
 */
export function migratePersisted(persisted: unknown, version: number): TrainerData {
  if (version < 2 && isRecord(persisted)) {
    const migrated: TrainerData = {
      ...defaultData(),
      language: pickEnum(persisted.language, LANGUAGES, "zh"),
      theme: pickEnum(persisted.theme, THEMES, DEFAULT_THEME),
      hintMode: pickEnum(persisted.hintMode, HINT_MODES, DEFAULT_HINT_MODE),
      difficulty: pickEnum(persisted.difficulty, DIFFICULTIES, DEFAULT_DIFFICULTY),
      sessionLength: SESSION_LENGTH_OPTIONS.includes(
        persisted.sessionLength as SessionLength,
      )
        ? (persisted.sessionLength as SessionLength)
        : DEFAULT_SESSION_LENGTH,
      lastMode: pickEnum(persisted.lastMode, MODES, DEFAULT_MODE),
      progress: migrateProgressV1(persisted.progress),
      daily: migrateDailyV1(persisted.daily),
      keyErrors: {},
    };
    return migrated;
  }
  return sanitizePersisted(persisted);
}

export const useTrainerStore = create<TrainerStore>()(
  persist(
    (set) => ({
      ...defaultData(),

      setLanguage: (language) => set({ language }),
      setTheme: (theme) => set({ theme }),
      setHintMode: (hintMode) => set({ hintMode }),
      setKeyRefMode: (keyRefMode) => set({ keyRefMode }),
      setKeyHaptics: (keyHaptics) => set({ keyHaptics }),
      setErrorTeaching: (errorTeaching) => set({ errorTeaching }),
      setDifficulty: (difficulty) => set({ difficulty }),
      setSessionLength: (sessionLength) => set({ sessionLength }),
      setLastMode: (lastMode) => set({ lastMode }),

      recordQuestionResult: (payload) =>
        set((state) => {
          const previous = state.progress[payload.id] ?? emptyProgress();
          const updated =
            payload.outcome === "perfect"
              ? applyPerfect(previous, payload.now, payload.practiceMs)
              : applyImperfect(previous, payload.now, payload.practiceMs);
          const dateKey = localDateKey(new Date(payload.now));
          const keyErrors = { ...state.keyErrors };
          for (const key of new Set(payload.wrongKeys)) {
            keyErrors[key] = (keyErrors[key] ?? 0) + 1;
          }
          return {
            progress: { ...state.progress, [payload.id]: updated },
            keyErrors,
            daily: mergeDaily(state.daily, dateKey, (stats) => ({
              practiceMs: stats.practiceMs + payload.practiceMs,
              questions: stats.questions + 1,
              keystrokes: stats.keystrokes + payload.keystrokes,
              wrongKeyEvents: stats.wrongKeyEvents + payload.wrongKeyEvents,
              bestStreak: Math.max(stats.bestStreak, payload.bestStreak),
              chars: stats.chars + payload.chars,
              corrections: stats.corrections + payload.corrections,
            })),
          };
        }),

      addPracticeTime: (practiceMs, now) =>
        set((state) => ({
          daily: mergeDaily(
            state.daily,
            localDateKey(new Date(now)),
            (stats) => ({ ...stats, practiceMs: stats.practiceMs + practiceMs }),
          ),
        })),

      applyBackup: (backup) =>
        set((state) => ({
          language: state.language,
          theme: state.theme,
          hintMode: backup.settings.hintMode,
          difficulty: backup.settings.difficulty,
          sessionLength: backup.settings.sessionLength,
          lastMode: backup.settings.lastMode,
          progress: backup.progress,
          daily: backup.daily,
          keyErrors: backup.keyErrors,
        })),

      resetItemProgress: (ids) =>
        set((state) => {
          const progress = { ...state.progress };
          for (const id of ids) delete progress[id];
          return { ...state, progress };
        }),

      resetProgress: () =>
        set((state) => ({
          ...defaultData(),
          language: state.language,
          theme: state.theme,
        })),
    }),
    {
      name: STORAGE_KEY,
      version: STORAGE_VERSION,
      partialize: (state): TrainerData => ({
        language: state.language,
        theme: state.theme,
        hintMode: state.hintMode,
        keyRefMode: state.keyRefMode,
        keyHaptics: state.keyHaptics,
        errorTeaching: state.errorTeaching,
        difficulty: state.difficulty,
        sessionLength: state.sessionLength,
        lastMode: state.lastMode,
        progress: state.progress,
        daily: state.daily,
        keyErrors: state.keyErrors,
      }),
      migrate: migratePersisted,
      merge: (persisted, current) => ({
        ...current,
        ...sanitizePersisted(persisted),
      }),
    },
  ),
);

/** 测试辅助:把 store 重置为全新默认状态。 */
export function resetTrainerStore(): void {
  useTrainerStore.setState({ ...defaultData() });
}
