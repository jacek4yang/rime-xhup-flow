/**
 * 会话页 ↔ 小结页的进程内传递(纯模块,无持久化)。
 *
 * SessionState 含 Map/RNG 等不可序列化结构,不进 URL 也不进存储;
 * 小程序页面栈内用模块单例传递即可:会话结束 → 写入 holder →
 * redirectTo 小结页;小结页读取并在缺失时回退首页。
 */

import type { SessionState, TrainingItem } from "@xhup/trainer-core";

/** 小结页需要的一段会话结果。 */
export type FinishedSession = {
  /** 结束时的完整会话状态(只读消费)。 */
  state: SessionState;
  /** 本场练习模式(重试/重开用)。 */
  mode: string;
  /** 本场来源(weak = 错题重练)。 */
  src: "normal" | "weak";
  /** 本场出错的条目(小结的「最需要复习」)。 */
  weakItems: TrainingItem[];
};

let finished: FinishedSession | null = null;

export function setFinishedSession(session: FinishedSession): void {
  finished = session;
}

export function getFinishedSession(): FinishedSession | null {
  return finished;
}

export function clearFinishedSession(): void {
  finished = null;
}
