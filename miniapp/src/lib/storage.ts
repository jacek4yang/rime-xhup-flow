/**
 * 小程序宿主适配层:把 Taro/wx 本地存储实现为共享核心的
 * {@link StorageAdapter} 契约。学习语义不接触任何平台 API。
 */

import Taro from "@tarojs/taro";
import type { ItemProgress, StorageAdapter } from "@xhup/trainer-core";
import {
  LEGACY_PROGRESS_KEY,
  STORE_KEY,
  defaultAppState,
  migrateLegacyProgress,
  parsePersistedDocument,
  serializeDocument,
} from "./state-schema";
import type { AppState } from "./state-schema";

export const taroStorage = {
  get(key: string): string | null {
    try {
      const value = Taro.getStorageSync(key);
      return typeof value === "string" && value !== "" ? value : null;
    } catch {
      return null;
    }
  },
  set(key: string, value: string): void {
    Taro.setStorageSync(key, value);
  },
  remove(key: string): void {
    Taro.removeStorageSync(key);
  },
} satisfies StorageAdapter;

/**
 * 读取 V1 旧进度(仅迁移用):旧版只有稀疏 ItemProgress 记录。
 * 迁移走同一校验边界,不可信条目丢弃。
 */
export function loadLegacyProgress(): Record<string, ItemProgress> | null {
  const raw = taroStorage.get(LEGACY_PROGRESS_KEY);
  if (raw === null) return null;
  try {
    return migrateLegacyProgress(JSON.parse(raw));
  } catch {
    return null;
  }
}

/**
 * 读取全量应用状态(V2 文档)。文档缺失时尝试把 V1 旧进度迁入
 * progress 字段,其余字段取默认;此后状态统一从 V2 文档读写。
 */
export function loadAppState(): AppState {
  const parsed = parsePersistedDocument(taroStorage.get(STORE_KEY));
  if (parsed) return parsed;
  const legacy = loadLegacyProgress();
  if (legacy && Object.keys(legacy).length > 0) {
    return { ...defaultAppState(), progress: legacy };
  }
  return defaultAppState();
}

/** 写入全量应用状态(V2 文档,带版本号)。 */
export function saveAppState(state: AppState): void {
  taroStorage.set(STORE_KEY, serializeDocument(state));
}

/** 本机进度记录键保留给旧读取方;新代码统一走 AppState 文档。 */
export const PROGRESS_KEY = LEGACY_PROGRESS_KEY;

export function loadProgress(): Record<string, ItemProgress> {
  return loadAppState().progress;
}

export function saveProgress(progress: Record<string, ItemProgress>): void {
  const state = loadAppState();
  saveAppState({ ...state, progress });
}
