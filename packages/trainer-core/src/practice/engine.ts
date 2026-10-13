/**
 * 练习会话状态机(纯逻辑,不依赖 React)。
 *
 * V2:会话消费统一的 {@link TrainingItem} 抽象,单字/简码/词/组句共用
 * 同一个状态机;不为任何模式单写引擎。计时只累计活跃时间,暂停 /
 * 小结 / 设置页不计时。进度更新与持久化通过事件交给外层:引擎自己
 * 维护一份会话内进度副本供调度使用,store 侧用同一组纯函数
 * (applyQuestionProgress)独立重放,数学上保持一致。
 *
 * 简码评分契约(B5):简码条目携带备用合法码(全码)。引擎逐键跟踪
 * 实际输入路线——主练码(简码)完成且零错键 = perfect;改走备用路线
 * (全码)合法可完成,但按 imperfect 计,并把路线记入事件,鼓励
 * 简码回忆而不把全码当「无效输入」。
 *
 * 键位混淆捕获:每次错键在会话状态累计「期望键 → 实际键 + 码位」的
 * 紧凑聚合(learning/confusion),随 question-completed 事件交外层落库;
 * 引擎不保存任何原始按键序列。
 */

import type { TrainingItem } from "../data/trainer-index";
import {
  positionForIndex,
  recordConfusion,
  type ConfusionMap,
} from "../learning/confusion";
import {
  applyQuestionProgress,
  emptyProgress,
  type ItemProgress,
} from "../learning/progress";
import {
  pickNext,
  scheduleReview,
  RECENT_LIMIT,
  type QuestionPool,
  type ReviewItem,
  type Rng,
} from "./scheduler";
import type { HintMode, PracticeIntent, PracticeMode, QuestionOutcome, QuestionRoute } from "./types";
import { questionElapsedMs, tickQuestion } from "./hints";

export type SessionConfig = {
  mode: PracticeMode;
  intent: PracticeIntent;
  hintMode: HintMode;
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
  matchedCode: string | null;
  typed: string;
  hadError: boolean;
  usedAnswerHint: boolean;
  questionActiveMs: number;
  wrongKeysThisQuestion: number;
  /** 本题按过的键(去重;键位统计用),每题重置。 */
  wrongKeys: string[];
  /**
   * 本题产生的混淆对(期望键 → 实际键 + 码位),每题重置;
   * 会话级累计见 {@link SessionState.confusions}。
   */
  confusionsThisQuestion: ConfusionMap;
  /** 会话级混淆累计(紧凑聚合,非原始日志;小结与落库数据源)。 */
  confusions: ConfusionMap;
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
      /** 本题混淆对(期望键 → 实际键 + 码位;紧凑聚合)。 */
      confusions: ConfusionMap;
      /** 本题退格修正次数。 */
      corrections: number;
      practiceMs: number;
      questionMs: number;
      usedAnswerHint: boolean;
      independentRecall: boolean;
      bestStreak: number;
    }
  | { type: "time-flushed"; practiceMs: number }
  | { type: "session-completed" };

export type StepResult = {
  state: SessionState;
  events: SessionEvent[];
};

/** 当前题实际参与匹配的码(主练码或备用码)。 */
export function activeCode(state: Pick<SessionState, "route" | "current" | "matchedCode">): string {
  if (state.matchedCode !== null) return state.matchedCode;
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
    now: state.lastTickAt,
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
    matchedCode: null as string | null,
    typed: "",
    hadError: false,
    usedAnswerHint: false,
    questionActiveMs: 0,
    wrongKeysThisQuestion: 0,
    wrongKeys: [] as string[],
    confusionsThisQuestion: {},
    confusions: {},
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
  state = tickQuestion(state, now);
  const { current } = state;
  const alternate = current.alternateCode;

  // 必须匹配整个已接受前缀，不能拼接两条合法路线形成非法编码。
  const prefix = state.typed + key;
  const primary = current.acceptedCodes.find((code) => code.startsWith(prefix));
  const alternateMatches = alternate !== null && alternate.startsWith(prefix);
  const route = primary !== undefined ? "primary" : "alternate";
  const effective = primary ?? (alternateMatches ? alternate : null);

  if (effective === null) {
    // 混淆捕获:期望键 = 当前活动路线在该码位的字符,实际键 = 所按之键;
    // 码位由条目种类与序号推导(positionForIndex,词/句降级为 other:N)。
    const expected = expectedKey(state) ?? "";
    const position = positionForIndex(current, state.typed.length);
    return {
      state: {
        ...state,
        hadError: true,
        wrongKeysThisQuestion: state.wrongKeysThisQuestion + 1,
        wrongKeys: state.wrongKeys.includes(key)
          ? state.wrongKeys
          : [...state.wrongKeys, key],
        confusionsThisQuestion: recordConfusion(
          state.confusionsThisQuestion,
          expected,
          key,
          position,
        ),
        confusions: recordConfusion(state.confusions, expected, key, position),
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
    matchedCode: effective,
    typed,
    keystrokes: state.keystrokes + 1,
    lastWrongKey: null,
  };
  if (typed !== effective ||
      (alternate !== null && alternate.length > typed.length && alternate.startsWith(typed))) {
    return { state: next, events: [] };
  }
  return completeQuestion(next, rng, now);
}

/** 短码为更长合法码前缀时，只有显式确认才提交该短码。 */
export function confirmQuestion(state: SessionState, rng: Rng, now: number): StepResult {
  if (state.phase !== "question" || state.typed.length === 0 ||
      state.typed !== activeCode(state)) return { state, events: [] };
  return completeQuestion(tickQuestion(state, now), rng, now);
}

function completeQuestion(
  state: SessionState,
  rng: Rng,
  now: number,
): StepResult {
  const routeUsed = state.route;
  const outcome: QuestionOutcome =
    state.hadError || state.usedAnswerHint || routeUsed === "alternate" ? "imperfect" : "perfect";
  const id = state.current.id;
  const previous = state.progressById.get(id) ?? emptyProgress();
  const practiceMsThisQuestion = Math.max(0, now - state.lastTickAt);
  const questionMs = questionElapsedMs(state, now);
  const updated = applyQuestionProgress(previous, {
    outcome, routeUsed, independentRecall: outcome === "perfect",
    usedAnswerHint: state.usedAnswerHint, wrongKeyEvents: state.wrongKeysThisQuestion,
    questionMs, chars: state.current.charCount, now,
  });
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
    questionActiveMs: questionMs,
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
        confusions: state.confusionsThisQuestion,
        corrections: state.correctionsThisQuestion,
        practiceMs: practiceMsThisQuestion,
        questionMs,
        usedAnswerHint: state.usedAnswerHint,
        independentRecall: outcome === "perfect",
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
  const drawn = drawQuestion({ ...state, current: null, recentIds, lastTickAt: now }, rng);
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
      matchedCode: null,
      hadError: false,
      usedAnswerHint: false,
      questionActiveMs: 0,
      wrongKeysThisQuestion: 0,
      wrongKeys: [],
      confusionsThisQuestion: {},
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
  const typed = state.typed.slice(0, -1);
  const primary = state.current.acceptedCodes.find((code) => code.startsWith(typed));
  return {
    ...state,
    typed,
    route: primary !== undefined ? "primary" : "alternate",
    matchedCode: typed.length === 0 ? null : primary ?? state.current.alternateCode,
    corrections: state.corrections + 1,
    correctionsThisQuestion: state.correctionsThisQuestion + 1,
    lastWrongKey: null,
  };
}

/**
 * 重试当前题(错误教学路径):清空已输入与错键标记,保留同一题。
 * 清过任何内容时计入一次修正;无可清内容时原样返回。
 */
export function retryCurrent(state: SessionState): SessionState {
  if (state.phase !== "question") return state;
  const hadContent = state.typed.length > 0 || state.lastWrongKey !== null;
  if (!hadContent) return state;
  return {
    ...state,
    typed: "",
    route: "primary",
    matchedCode: null,
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
  state = tickQuestion(state, now);
  const elapsed = Math.max(0, now - state.lastTickAt);
  const practiceMs = state.activeMs + elapsed;
  return {
    state: { ...state, phase: "paused", activeMs: practiceMs,
      questionActiveMs: state.questionActiveMs + elapsed, lastTickAt: now },
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
    state.phase === "question"
      ? state.activeMs + Math.max(0, now - state.lastTickAt)
      : state.activeMs;
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
  if (state.phase !== "question" && state.phase !== "paused") return null;
  let expected: string | null | undefined;
  for (const code of state.current.acceptedCodes) {
    if (!code.startsWith(state.typed)) continue;
    const next = code[state.typed.length] ?? null;
    if (expected !== undefined && expected !== next) return null;
    expected = next;
  }
  const alternate = state.current.alternateCode;
  if (alternate !== null && alternate.startsWith(state.typed)) {
    const next = alternate[state.typed.length] ?? null;
    if (expected !== undefined && expected !== next) return null;
    expected = next;
  }
  return expected ?? null;
}
