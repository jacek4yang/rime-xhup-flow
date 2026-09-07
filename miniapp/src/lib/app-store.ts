/**
 * 应用状态仓库工厂(纯逻辑,存储适配注入;node 测试可直接运行)。
 *
 * 订阅式最小 store:无 zustand,React 侧经 useAppState 订阅。
 * 学习语义全部复用共享核心的纯函数(applyPerfect/applyImperfect/
 * emptyDailyStats);本模块只负责把核心语义落到持久化状态上,
 * 与桌面端 trainer store 的 recordQuestionResult 保持同一套数学。
 */

import {
  applyImperfect,
  applyPerfect,
  emptyDailyStats,
  emptyProgress,
  localDateKey,
  mergeConfusionMaps,
  type ConfusionMap,
  type ItemProgress,
  type StorageAdapter,
} from "@xhup/trainer-core";
import {
  defaultAppState,
  sanitizePersisted,
  serializeDocument,
  type AppState,
  type AppSettings,
} from "./state-schema";

/** 一题完成时的记账载荷(与桌面端 store 同名概念一致)。 */
export type QuestionResultPayload = {
  id: string;
  outcome: "perfect" | "imperfect";
  keystrokes: number;
  wrongKeyEvents: number;
  /** 本题按错的键(去重)。 */
  wrongKeys: readonly string[];
  /** 本题混淆对(期望键 → 实际键 + 码位;来自会话事件,可选增量)。 */
  confusions?: ConfusionMap;
  chars: number;
  corrections: number;
  practiceMs: number;
  bestStreak: number;
  now: number;
};

export type AppStore = {
  getState(): AppState;
  subscribe(listener: () => void): () => void;
  updateSettings(patch: Partial<AppSettings>): void;
  recordQuestionResult(payload: QuestionResultPayload): void;
  addPracticeTime(practiceMs: number, now: number): void;
  /** 重置学习进度与按日统计;偏好保留(与桌面端 resetProgress 一致)。 */
  resetProgress(): void;
  /** 用校验过的备份数据整体替换进度/统计/键位错误与混淆聚合。 */
  applyBackup(data: {
    progress: Record<string, ItemProgress>;
    daily: AppState["daily"];
    keyErrors: Record<string, number>;
    confusions?: ConfusionMap;
  }): void;
};

export function createAppStore(storage: StorageAdapter, storeKey: string): AppStore {
  let state: AppState = defaultAppState();
  let loaded = false;
  const listeners = new Set<() => void>();

  function ensureLoaded(): void {
    if (loaded) return;
    loaded = true;
    try {
      const raw = storage.get(storeKey);
      if (
        raw !== null &&
        typeof raw === "object" &&
        typeof (raw as Promise<string | null>).then === "function"
      ) {
        // 异步适配(契约允许):就绪后再应用,错误回退默认值。
        void (raw as Promise<string | null>)
          .then((text) => {
            if (text !== null && text !== "") {
              state = sanitizePersisted(tryParse(text));
            }
          })
          .catch((cause) => {
            console.warn("[xhup] 读取本机状态失败,已回退默认值", cause);
          });
        return;
      }
      if (typeof raw === "string" && raw !== "") {
        state = sanitizePersisted(tryParse(raw));
      }
    } catch (cause) {
      // 存储适配层抛错:回退默认值,绝不崩溃。
      console.warn("[xhup] 读取本机状态失败,已回退默认值", cause);
    }
  }

  function tryParse(raw: string): unknown {
    try {
      return JSON.parse(raw);
    } catch {
      // 损坏存储:回退默认值,绝不静默清空后假装无事——证据留在此日志。
      console.warn("[xhup] 本机状态损坏,已回退默认值");
      return null;
    }
  }

  function persist(): void {
    try {
      storage.set(storeKey, serializeDocument(state));
    } catch (cause) {
      // 写失败不崩溃:内存态继续,下次变更再试。
      console.warn("[xhup] 写入本机状态失败", cause);
    }
  }

  function set(next: AppState): void {
    state = next;
    persist();
    for (const listener of listeners) listener();
  }

  return {
    getState() {
      ensureLoaded();
      return state;
    },
    subscribe(listener) {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
    updateSettings(patch) {
      ensureLoaded();
      set({ ...state, settings: { ...state.settings, ...patch } });
    },
    recordQuestionResult(payload) {
      ensureLoaded();
      const previous = state.progress[payload.id] ?? emptyProgress();
      const updated =
        payload.outcome === "perfect"
          ? applyPerfect(previous, payload.now, payload.practiceMs)
          : applyImperfect(previous, payload.now, payload.practiceMs);
      const dateKey = localDateKey(new Date(payload.now));
      const day = state.daily[dateKey] ?? emptyDailyStats();
      const keyErrors = { ...state.keyErrors };
      for (const key of new Set(payload.wrongKeys)) {
        keyErrors[key] = (keyErrors[key] ?? 0) + 1;
      }
      set({
        ...state,
        progress: { ...state.progress, [payload.id]: updated },
        keyErrors,
        // 混淆对按题合并落库(低频写入;计数相加,上限由核心约束)。
        confusions: mergeConfusionMaps(state.confusions, payload.confusions ?? {}),
        daily: {
          ...state.daily,
          [dateKey]: {
            practiceMs: day.practiceMs + payload.practiceMs,
            questions: day.questions + 1,
            keystrokes: day.keystrokes + payload.keystrokes,
            wrongKeyEvents: day.wrongKeyEvents + payload.wrongKeyEvents,
            bestStreak: Math.max(day.bestStreak, payload.bestStreak),
            chars: day.chars + payload.chars,
            corrections: day.corrections + payload.corrections,
          },
        },
      });
    },
    addPracticeTime(practiceMs, now) {
      ensureLoaded();
      if (practiceMs <= 0) return;
      const dateKey = localDateKey(new Date(now));
      const day = state.daily[dateKey] ?? emptyDailyStats();
      set({
        ...state,
        daily: {
          ...state.daily,
          [dateKey]: { ...day, practiceMs: day.practiceMs + practiceMs },
        },
      });
    },
    resetProgress() {
      ensureLoaded();
      set({ ...defaultAppState(), settings: state.settings });
    },
    applyBackup(data) {
      ensureLoaded();
      set({
        ...state,
        progress: data.progress,
        daily: data.daily,
        keyErrors: data.keyErrors,
        confusions: data.confusions ?? {},
      });
    },
  };
}
