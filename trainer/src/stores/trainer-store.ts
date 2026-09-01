/**
 * 训练器持久化状态(zustand + localStorage)。
 *
 * 只持久化小型稀疏数据:偏好、见过的条目进度、按日统计。
 * 规范数据集(2.5MB)、当前题、已输入、回炉队列等运行态一律不进 store。
 * 持久化版本 1;未知/损坏的旧状态在校验边界逐字段回退为默认值。
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
import { DEFAULT_THEME, type ThemePreference } from "@/lib/theme";
import type { Difficulty } from "@/lib/trainer-index";
import {
  DEFAULT_DIFFICULTY,
  DEFAULT_HINT_MODE,
  DEFAULT_MODE,
  DEFAULT_SESSION_LENGTH,
  SESSION_LENGTH_OPTIONS,
  type HintMode,
  type PracticeMode,
  type SessionLength,
} from "@/features/practice/types";
import type { QuestionOutcome } from "@/features/practice/types";

/** 按本地日历日累计的统计。 */
export type DailyStats = {
  practiceMs: number;
  questions: number;
  keystrokes: number;
  wrongKeyEvents: number;
  bestStreak: number;
};

export function emptyDailyStats(): DailyStats {
  return {
    practiceMs: 0,
    questions: 0,
    keystrokes: 0,
    wrongKeyEvents: 0,
    bestStreak: 0,
  };
}

export type TrainerData = {
  theme: ThemePreference;
  hintMode: HintMode;
  difficulty: Difficulty;
  sessionLength: SessionLength;
  lastMode: PracticeMode;
  /** 稀疏进度:只有实际见过的条目才有记录。 */
  progress: Record<string, ItemProgress>;
  /** 按日统计:YYYY-MM-DD → DailyStats。 */
  daily: Record<string, DailyStats>;
};

export type QuestionResultPayload = {
  id: string;
  outcome: QuestionOutcome;
  keystrokes: number;
  wrongKeyEvents: number;
  practiceMs: number;
  bestStreak: number;
  now: number;
};

export type TrainerActions = {
  setTheme: (theme: ThemePreference) => void;
  setHintMode: (hintMode: HintMode) => void;
  setDifficulty: (difficulty: Difficulty) => void;
  setSessionLength: (sessionLength: SessionLength) => void;
  setLastMode: (lastMode: PracticeMode) => void;
  /** 一题完成:更新条目进度 + 当日统计(低频写入,每题一次)。 */
  recordQuestionResult: (payload: QuestionResultPayload) => void;
  /** 暂停/结束时结清的纯练习时长。 */
  addPracticeTime: (practiceMs: number, now: number) => void;
  /** 重置学习进度与练习偏好;保留主题。 */
  resetProgress: () => void;
};

export type TrainerStore = TrainerData & TrainerActions;

export const STORAGE_KEY = "xhup-flow.trainer.v1";

function defaultData(): TrainerData {
  return {
    theme: DEFAULT_THEME,
    hintMode: DEFAULT_HINT_MODE,
    difficulty: DEFAULT_DIFFICULTY,
    sessionLength: DEFAULT_SESSION_LENGTH,
    lastMode: DEFAULT_MODE,
    progress: {},
    daily: {},
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
    (value.lastSeenAt === null || isNumber(value.lastSeenAt))
  );
}

function isDailyStats(value: unknown): value is DailyStats {
  return (
    isRecord(value) &&
    isNumber(value.practiceMs) &&
    isNumber(value.questions) &&
    isNumber(value.keystrokes) &&
    isNumber(value.wrongKeyEvents) &&
    isNumber(value.bestStreak)
  );
}

const THEMES: readonly ThemePreference[] = ["system", "light", "dark"];
const HINT_MODES: readonly HintMode[] = ["always", "on-error", "hidden"];
const DIFFICULTIES: readonly Difficulty[] = ["beginner", "daily", "full"];
const MODES: readonly PracticeMode[] = ["double", "sound-shape", "full", "mixed"];

function pickEnum<T extends string>(
  value: unknown,
  allowed: readonly T[],
  fallback: T,
): T {
  return typeof value === "string" && (allowed as readonly string[]).includes(value)
    ? (value as T)
    : fallback;
}

function pickRecord<V>(
  value: unknown,
  guard: (entry: unknown) => entry is V,
): Record<string, V> {
  if (!isRecord(value)) return {};
  const result: Record<string, V> = {};
  for (const [key, entry] of Object.entries(value)) {
    if (guard(entry)) result[key] = entry;
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
    theme: pickEnum(value.theme, THEMES, defaults.theme),
    hintMode: pickEnum(value.hintMode, HINT_MODES, defaults.hintMode),
    difficulty: pickEnum(value.difficulty, DIFFICULTIES, defaults.difficulty),
    sessionLength,
    lastMode: pickEnum(value.lastMode, MODES, defaults.lastMode),
    progress: pickRecord(value.progress, isProgress),
    daily: pickRecord(value.daily, isDailyStats),
  };
}

export const useTrainerStore = create<TrainerStore>()(
  persist(
    (set) => ({
      ...defaultData(),

      setTheme: (theme) => set({ theme }),
      setHintMode: (hintMode) => set({ hintMode }),
      setDifficulty: (difficulty) => set({ difficulty }),
      setSessionLength: (sessionLength) => set({ sessionLength }),
      setLastMode: (lastMode) => set({ lastMode }),

      recordQuestionResult: (payload) =>
        set((state) => {
          const previous = state.progress[payload.id] ?? emptyProgress();
          const updated =
            payload.outcome === "perfect"
              ? applyPerfect(previous, payload.now)
              : applyImperfect(previous, payload.now);
          const dateKey = localDateKey(new Date(payload.now));
          return {
            progress: { ...state.progress, [payload.id]: updated },
            daily: mergeDaily(state.daily, dateKey, (stats) => ({
              practiceMs: stats.practiceMs + payload.practiceMs,
              questions: stats.questions + 1,
              keystrokes: stats.keystrokes + payload.keystrokes,
              wrongKeyEvents: stats.wrongKeyEvents + payload.wrongKeyEvents,
              bestStreak: Math.max(stats.bestStreak, payload.bestStreak),
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

      resetProgress: () =>
        set((state) => ({ ...defaultData(), theme: state.theme })),
    }),
    {
      name: STORAGE_KEY,
      version: 1,
      partialize: (state): TrainerData => ({
        theme: state.theme,
        hintMode: state.hintMode,
        difficulty: state.difficulty,
        sessionLength: state.sessionLength,
        lastMode: state.lastMode,
        progress: state.progress,
        daily: state.daily,
      }),
      migrate: (persisted) => sanitizePersisted(persisted),
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
