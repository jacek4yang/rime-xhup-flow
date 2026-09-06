/**
 * 小程序宿主适配层:把 Taro/wx 本地存储实现为共享核心的
 * {@link StorageAdapter} 契约。学习语义不接触任何平台 API。
 */

import Taro from "@tarojs/taro";
import type { ItemProgress, StorageAdapter } from "@xhup/trainer-core";

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

/** 本机进度记录键(与桌面端持久化 Schema 共用同一份 ItemProgress 形状)。 */
export const PROGRESS_KEY = "xhup.miniapp.progress.v1";

export function loadProgress(): Record<string, ItemProgress> {
  const raw = taroStorage.get(PROGRESS_KEY);
  if (!raw) return {};
  try {
    const parsed = JSON.parse(raw);
    return parsed && typeof parsed === "object" ? parsed : {};
  } catch {
    // 损坏存储:回退为空进度,绝不静默清空后假装无事——证据留在此日志。
    console.warn("[xhup] 本机进度损坏,已回退为空进度");
    return {};
  }
}

export function saveProgress(progress: Record<string, ItemProgress>): void {
  taroStorage.set(PROGRESS_KEY, JSON.stringify(progress));
}
