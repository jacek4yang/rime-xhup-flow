/**
 * 练习会话状态机(纯逻辑,不依赖 React)。
 *
 * V2:会话消费统一的 {@link TrainingItem} 抽象,单字/简码/词/组句共用
 * 同一个状态机;不为任何模式单写引擎。计时只累计活跃时间,暂停 /
 * 小结 / 设置页不计时。进度更新与持久化通过事件交给外层:引擎自己
 * 维护一份会话内进度副本供调度使用,store 侧用同一组纯函数
 * (applyPerfect / applyImperfect)独立重放,数学上保持一致。
 *
 * 简码评分契约(B5):简码条目携带备用合法码(全码)。引擎逐键跟踪
 * 实际输入路线——主练码(简码)完成且零错键 = perfect;改走备用路线
 * (全码)合法可完成,但按 imperfect 计,并把路线记入事件,鼓励
 * 简码回忆而不把全码当「无效输入」。
 */

import type { TrainingItem } from "@/lib/trainer-index";
import {
  applyImperfect,
  applyPerfect,
  emptyProgress,
  type ItemProgress,
} from "@/lib/progress";
import {
  pickNext,
  scheduleReview,
  RECENT_LIMIT,
  type QuestionPool,
  type ReviewItem,
  type Rng,
} from "./scheduler";
import type { PracticeMode, QuestionOutcome, QuestionRoute } from "./types";

export type SessionConfig = {
  mode: PracticeMode;
  /** 目标题数;0 表示无限。 */
  targetLength: number;
  pools: readonly QuestionPool[];
};

export type SessionPhase = "question" | "feedback" | "paused" | "completed";

export type SessionState = {
  config: SessionConfig;
  phase: SessionPhase;
  current: TrainingItem;
  /** 当前题实际输入路线(初始为主练码;首键分歧时切换到备用码)。 */
  route: QuestionRoute;
  typed: string;
  hadError: boolean;
  wrongKeysThisQuestion: number;
  /** 本题按过的键(去重;键位统计用),每题重置。 */
  wrongKeys: string[];
  /** 最近一次按错的键(用于键盘闪烁提示),下一个按键事件后清除。 */
  lastWrongKey: string | null;
  lastOutcome: QuestionOutcome | null;
  lastRoute: QuestionRoute | null;
  questionsCompleted: number;
  perfect: number;
  imperfect: number;
  keystrokes: number;
  wrongKeyEvents: number;
  /** 完成的汉字数(CPM 分母;组句一次贡献多字)。 */
  charsCompleted: number;
  /** 退格修正次数(会话累计)。 */
  corrections: number;
  /** 本题退格修正次数(事件用,每题重置)。 */
  correctionsThisQuestion: number;
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
      item: TrainingItem;
      outcome: QuestionOutcome;
      /** 实际使用的输入路线(primary = 主练码;alternate = 备用合法码)。 */
      routeUsed: QuestionRoute;
      keystrokes: number;
      wrongKeyEvents: number;
      /** 本题按错的键(去重)。 */
      wrongKeys: string[];
      /** 本题退格修正次数。 */
      corrections: number;
      practiceMs: number;
      bestStreak: number;
    }
  | { type: "time-flushed"; practiceMs: number }
  | { type: "session-completed" };

export type StepResult = {
  state: SessionState;
  events: SessionEvent[];
};

/** 当前题实际参与匹配的码(主练码或备用码)。 */
export function activeCode(state: Pick<SessionState, "route" | "current">): string {
  return state.route === "alternate" && state.current.alternateCode !== null
    ? state.current.alternateCode
    : state.current.primaryCode;
}

function drawQuestion(
  state: Omit<SessionState, "current" | "phase" | "route"> & {
    current: TrainingItem | null;
  },
  rng: Rng,
): {
  current: TrainingItem;
  route: QuestionRoute;
  reviewQueue: ReviewItem[];
  mixedCursor: number;
} | null {
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
    current: picked.item,
    route: "primary",
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
    route: "primary" as const,
    typed: "",
    hadError: false,
    wrongKeysThisQuestion: 0,
    wrongKeys: [] as string[],
    lastWrongKey: null,
    lastOutcome: null,
    lastRoute: null,
    questionsCompleted: 0,
    perfect: 0,
    imperfect: 0,
    keystrokes: 0,
    wrongKeyEvents: 0,
    charsCompleted: 0,
    corrections: 0,
    correctionsThisQuestion: 0,
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
  const { current } = state;
  const primary = current.primaryCode;
  const alternate = current.alternateCode;

  // 路线判定:主练码匹配则保持;主练码分歧且备用码在「当前已输入前缀 +
  // 本键」上连续匹配时,切换到备用合法码(全码)。已锁定的备用路线沿用。
  let route = state.route;
  const expectedOn = (code: string): string => code[state.typed.length];
  if (route === "primary" && key !== expectedOn(primary)) {
    if (alternate !== null && key === expectedOn(alternate)) {
      route = "alternate";
    }
  }
  const effective = route === "alternate" && alternate !== null ? alternate : primary;

  if (key !== effective[state.typed.length]) {
    return {
      state: {
        ...state,
        hadError: true,
        wrongKeysThisQuestion: state.wrongKeysThisQuestion + 1,
        wrongKeys: state.wrongKeys.includes(key)
          ? state.wrongKeys
          : [...state.wrongKeys, key],
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
    route,
    typed,
    keystrokes: state.keystrokes + 1,
    lastWrongKey: null,
  };
  if (typed !== effective) {
    return { state: next, events: [] };
  }
  return completeQuestion(next, rng, now);
}

function completeQuestion(
  state: SessionState,
  rng: Rng,
  now: number,
): StepResult {
  const routeUsed = state.route;
  const outcome: QuestionOutcome =
    state.hadError || routeUsed === "alternate" ? "imperfect" : "perfect";
  const id = state.current.id;
  const previous = state.progressById.get(id) ?? emptyProgress();
  const practiceMsThisQuestion = now - state.lastTickAt;
  const updated =
    outcome === "perfect"
      ? applyPerfect(previous, now, practiceMsThisQuestion)
      : applyImperfect(previous, now, practiceMsThisQuestion);
  const progressById = new Map(state.progressById);
  progressById.set(id, updated);

  const practiceMs = state.activeMs + practiceMsThisQuestion;
  const currentStreak = outcome === "perfect" ? state.currentStreak + 1 : 0;
  const bestStreak = Math.max(state.bestStreak, currentStreak);
  const reviewQueue =
    outcome === "imperfect"
      ? scheduleReview(state.reviewQueue, id, rng)
      : state.reviewQueue;

  const completed: SessionState = {
    ...state,
    phase: "feedback",
    lastOutcome: outcome,
    lastRoute: routeUsed,
    questionsCompleted: state.questionsCompleted + 1,
    perfect: state.perfect + (outcome === "perfect" ? 1 : 0),
    imperfect: state.imperfect + (outcome === "imperfect" ? 1 : 0),
    charsCompleted: state.charsCompleted + state.current.charCount,
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
        item: state.current,
        outcome,
        routeUsed,
        keystrokes: activeCode(state).length + state.wrongKeysThisQuestion,
        wrongKeyEvents: state.wrongKeysThisQuestion,
        wrongKeys: state.wrongKeys,
        corrections: state.correctionsThisQuestion,
        practiceMs: practiceMsThisQuestion,
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

  const recentIds = [...state.recentIds, state.current.id].slice(-RECENT_LIMIT);
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
      wrongKeys: [],
      correctionsThisQuestion: 0,
      lastWrongKey: null,
      lastOutcome: null,
      lastRoute: null,
      lastTickAt: now,
    },
    events: [],
  };
}

/** 退格:删除最后一个已接受的键,计入修正次数,不影响对错计数。 */
export function backspace(state: SessionState): SessionState {
  if (state.phase !== "question" || state.typed.length === 0) return state;
  return {
    ...state,
    typed: state.typed.slice(0, -1),
    corrections: state.corrections + 1,
    correctionsThisQuestion: state.correctionsThisQuestion + 1,
    lastWrongKey: null,
  };
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

/** 当前题的期望下一键(用于键盘高亮;跟随实际输入路线)。 */
export function expectedKey(state: SessionState): string | null {
  if (state.phase !== "question") return null;
  return activeCode(state)[state.typed.length] ?? null;
}
