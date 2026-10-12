import type { SessionState } from "./engine";
import { HINT_DELAY_MS } from "./types";

/** 活跃片段累计；暂停、反馈和小结不增加本题时钟。 */
export function questionElapsedMs(state: SessionState, now: number): number {
  return state.questionActiveMs +
    (state.phase === "question" ? Math.max(0, now - state.lastTickAt) : 0);
}

export function hintAccess(state: SessionState, now: number): {
  revealAnswer: boolean;
  showErrorKind: boolean;
  showHelpOffer: boolean;
} {
  const mode = state.config.hintMode;
  const triggered = mode === "always" ||
    (mode === "on-error" && state.hadError) ||
    (mode === "on-delay" && questionElapsedMs(state, now) >= HINT_DELAY_MS);
  return {
    revealAnswer: state.usedAnswerHint || state.phase === "feedback" ||
      (state.config.intent === "learn" && triggered),
    showErrorKind: state.hadError,
    showHelpOffer: triggered || state.hadError,
  };
}

export function revealAnswer(state: SessionState): SessionState {
  if (state.phase === "completed" || state.phase === "feedback" || state.usedAnswerHint) return state;
  return { ...state, usedAnswerHint: true };
}

/** 自动教学也必须在输入结算前记录，不能在 render 中隐式修改统计。 */
export function tickQuestion(state: SessionState, now: number): SessionState {
  return state.phase === "question" && hintAccess(state, now).revealAnswer
    ? revealAnswer(state) : state;
}
