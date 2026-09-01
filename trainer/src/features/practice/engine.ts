/**
 * 练习会话状态机(纯逻辑,不依赖 React)。
 *
 * React 层只在事件回调里调用这些函数并 setState;计时只累计活跃时间,
 * 暂停 / 小结 / 设置页不计时。进度更新与持久化通过事件交给外层:
 * 引擎自己维护一份会话内进度副本供调度使用,store 侧用同一组纯函数
 * (applyPerfect / applyImperfect)独立重放,数学上保持一致。
 */

import type { TrainerEntry } from "@/lib/trainer-data";
import { itemId } from "@/lib/trainer-data";
import {
  applyImperfect,
  applyPerfect,
  emptyProgress,
  type ItemProgress,
} from "@/lib/progress";
import type { Difficulty } from "@/lib/trainer-index";
import {
  pickNext,
  scheduleReview,
  RECENT_LIMIT,
  type QuestionPool,
  type ReviewItem,
  type Rng,
} from "./scheduler";
import type { PracticeMode, QuestionOutcome } from "./types";

export type SessionConfig = {
  mode: PracticeMode;
  difficulty: Difficulty;
  /** 目标题数;0 表示无限。 */
  targetLength: number;
  pools: readonly QuestionPool[];
};

export type SessionPhase = "question" | "feedback" | "paused" | "completed";

export type SessionState = {
  config: SessionConfig;
  phase: SessionPhase;
  current: TrainerEntry;
  typed: string;
  hadError: boolean;
  wrongKeysThisQuestion: number;
  /** 最近一次按错的键(用于键盘闪烁提示),下一个按键事件后清除。 */
  lastWrongKey: string | null;
  lastOutcome: QuestionOutcome | null;
  questionsCompleted: number;
  perfect: number;
  imperfect: number;
  keystrokes: number;
  wrongKeyEvents: number;
  currentStreak: number;
  bestStreak: number;
  activeMs: number;
  /** 最近一次开始/恢复/报账的时间戳。 */
  lastTickAt: number;
  progressById: Map<string, ItemProgress>;
  recentIds: string[];
  reviewQueue: ReviewItem[];
  mixedCursor: number;
};

export type SessionEvent =
  | {
      type: "question-completed";
      entry: TrainerEntry;
      outcome: QuestionOutcome;
      keystrokes: number;
      wrongKeyEvents: number;
      practiceMs: number;
      bestStreak: number;
    }
  | { type: "time-flushed"; practiceMs: number }
  | { type: "session-completed" };

export type StepResult = {
  state: SessionState;
  events: SessionEvent[];
};

function drawQuestion(
  state: Omit<SessionState, "current" | "phase"> & {
    current: TrainerEntry | null;
  },
  rng: Rng,
): { current: TrainerEntry; reviewQueue: ReviewItem[]; mixedCursor: number } | null {
  const picked = pickNext({
    mode: state.config.mode,
    pools: state.config.pools,
    progressById: state.progressById,
    recentIds: state.recentIds,
    reviewQueue: state.reviewQueue,
    mixedCursor: state.mixedCursor,
    rng,
  });
  if (!picked) return null;
  return {
    current: picked.entry,
    reviewQueue: picked.reviewQueue,
    mixedCursor: picked.mixedCursor,
  };
}

/** 开一场会话;第一题立即抽出。池为空返回 null(调用方显示空态)。 */
export function createSession(
  config: SessionConfig,
  progressById: ReadonlyMap<string, ItemProgress>,
  rng: Rng,
  now: number,
): SessionState | null {
  const base = {
    config,
    phase: "question" as const,
    current: null,
    typed: "",
    hadError: false,
    wrongKeysThisQuestion: 0,
    lastWrongKey: null,
    lastOutcome: null,
    questionsCompleted: 0,
    perfect: 0,
    imperfect: 0,
    keystrokes: 0,
    wrongKeyEvents: 0,
    currentStreak: 0,
    bestStreak: 0,
    activeMs: 0,
    lastTickAt: now,
    progressById: new Map(progressById),
    recentIds: [] as string[],
    reviewQueue: [] as ReviewItem[],
    mixedCursor: 0,
  };
  const drawn = drawQuestion(base, rng);
  if (!drawn) return null;
  return { ...base, ...drawn, phase: "question" };
}

/** 处理一个字母键。非出题阶段或非 a-z 返回原状态。 */
export function typeKey(
  state: SessionState,
  key: string,
  rng: Rng,
  now: number,
): StepResult {
  if (state.phase !== "question" || !/^[a-z]$/.test(key)) {
    return { state, events: [] };
  }
  const expected = state.current.code[state.typed.length];
  if (key !== expected) {
    return {
      state: {
        ...state,
        hadError: true,
        wrongKeysThisQuestion: state.wrongKeysThisQuestion + 1,
        keystrokes: state.keystrokes + 1,
        wrongKeyEvents: state.wrongKeyEvents + 1,
        lastWrongKey: key,
      },
      events: [],
    };
  }

  const typed = state.typed + key;
  const next: SessionState = {
    ...state,
    typed,
    keystrokes: state.keystrokes + 1,
    lastWrongKey: null,
  };
  if (typed !== state.current.code) {
    return { state: next, events: [] };
  }
  return completeQuestion(next, rng, now);
}

function completeQuestion(
  state: SessionState,
  rng: Rng,
  now: number,
): StepResult {
  const outcome: QuestionOutcome = state.hadError ? "imperfect" : "perfect";
  const id = itemId(state.current);
  const previous =
    state.progressById.get(id) ?? emptyProgress();
  const updated =
    outcome === "perfect"
      ? applyPerfect(previous, now)
      : applyImperfect(previous, now);
  const progressById = new Map(state.progressById);
  progressById.set(id, updated);

  const practiceMs = state.activeMs + (now - state.lastTickAt);
  const currentStreak =
    outcome === "perfect" ? state.currentStreak + 1 : 0;
  const bestStreak = Math.max(state.bestStreak, currentStreak);
  const reviewQueue =
    outcome === "imperfect"
      ? scheduleReview(state.reviewQueue, id, rng)
      : state.reviewQueue;

  const completed: SessionState = {
    ...state,
    phase: "feedback",
    lastOutcome: outcome,
    questionsCompleted: state.questionsCompleted + 1,
    perfect: state.perfect + (outcome === "perfect" ? 1 : 0),
    imperfect: state.imperfect + (outcome === "imperfect" ? 1 : 0),
    currentStreak,
    bestStreak,
    activeMs: practiceMs,
    lastTickAt: now,
    progressById,
    reviewQueue,
  };

  return {
    state: completed,
    events: [
      {
        type: "question-completed",
        entry: state.current,
        outcome,
        keystrokes: state.current.code.length + state.wrongKeysThisQuestion,
        wrongKeyEvents: state.wrongKeysThisQuestion,
        practiceMs: practiceMs - state.activeMs,
        bestStreak,
      },
    ],
  };
}

/**
 * 反馈展示完毕后推进到下一题;达到目标题数则结束会话。
 * 无限模式(targetLength = 0)永不自动结束。
 */
export function advance(
  state: SessionState,
  rng: Rng,
  now: number,
): StepResult {
  if (state.phase !== "feedback") return { state, events: [] };

  const { targetLength } = state.config;
  if (targetLength > 0 && state.questionsCompleted >= targetLength) {
    return {
      state: { ...state, phase: "completed" },
      events: [{ type: "session-completed" }],
    };
  }

  const recentIds = [...state.recentIds, itemId(state.current)].slice(
    -RECENT_LIMIT,
  );
  const drawn = drawQuestion({ ...state, current: null, recentIds }, rng);
  if (!drawn) {
    // 池意外耗尽:视为会话结束,不产生错误。
    return {
      state: { ...state, recentIds, phase: "completed" },
      events: [{ type: "session-completed" }],
    };
  }
  return {
    state: {
      ...state,
      ...drawn,
      recentIds,
      phase: "question",
      typed: "",
      hadError: false,
      wrongKeysThisQuestion: 0,
      lastWrongKey: null,
      lastOutcome: null,
      lastTickAt: now,
    },
    events: [],
  };
}

/** 退格:删除最后一个已接受的键,不影响计数。 */
export function backspace(state: SessionState): SessionState {
  if (state.phase !== "question" || state.typed.length === 0) return state;
  return { ...state, typed: state.typed.slice(0, -1), lastWrongKey: null };
}

/** 暂停:结清活跃时间。只在出题阶段可暂停(反馈阶段转瞬即过,不可暂停)。 */
export function pause(state: SessionState, now: number): StepResult {
  if (state.phase !== "question") {
    return { state, events: [] };
  }
  const practiceMs = state.activeMs + (now - state.lastTickAt);
  return {
    state: { ...state, phase: "paused", activeMs: practiceMs, lastTickAt: now },
    events: [{ type: "time-flushed", practiceMs: practiceMs - state.activeMs }],
  };
}

/** 恢复练习。 */
export function resume(state: SessionState, now: number): SessionState {
  if (state.phase !== "paused") return state;
  return { ...state, phase: "question", lastTickAt: now };
}

/** 主动结束本次练习(无限模式或中途退出):结清时间并进入小结。 */
export function finish(state: SessionState, now: number): StepResult {
  if (state.phase === "completed") return { state, events: [] };
  const practiceMs =
    state.phase === "paused"
      ? state.activeMs
      : state.activeMs + (now - state.lastTickAt);
  return {
    state: {
      ...state,
      phase: "completed",
      activeMs: practiceMs,
      lastTickAt: now,
    },
    events: [
      { type: "time-flushed", practiceMs: practiceMs - state.activeMs },
      { type: "session-completed" },
    ],
  };
}

/** 当前题的期望下一键(用于键盘高亮)。 */
export function expectedKey(state: SessionState): string | null {
  if (state.phase !== "question") return null;
  return state.current.code[state.typed.length] ?? null;
}
