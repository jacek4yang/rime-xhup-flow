/**
 * 全局应用状态单例:Taro 本地存储 + 订阅式 store + React 订阅钩子。
 *
 * 页面统一经 useAppState 订阅;动作经 store 动作方法,持久化在动作内
 * 同步完成(本地优先,无异步依赖)。
 */

import { useSyncExternalStore } from "react";
import { STORE_KEY } from "./state-schema";
import type { AppSettings, AppState } from "./state-schema";
import { createAppStore, type QuestionResultPayload } from "./app-store";
import { taroStorage } from "./storage";

export const appStore = createAppStore(taroStorage, STORE_KEY);

export type { QuestionResultPayload };

function subscribe(listener: () => void): () => void {
  return appStore.subscribe(listener);
}

function getSnapshot(): AppState {
  return appStore.getState();
}

/** 订阅全量应用状态(引用稳定:每次动作后返回新对象)。 */
export function useAppState(): AppState {
  return useSyncExternalStore(subscribe, getSnapshot, getSnapshot);
}

/** 动作快捷方式(setter 自动持久化)。 */
export const actions = {
  updateSettings: (patch: Partial<AppSettings>) => appStore.updateSettings(patch),
  recordQuestionResult: (payload: QuestionResultPayload) =>
    appStore.recordQuestionResult(payload),
  addPracticeTime: (practiceMs: number, now: number) =>
    appStore.addPracticeTime(practiceMs, now),
  resetProgress: () => appStore.resetProgress(),
};
