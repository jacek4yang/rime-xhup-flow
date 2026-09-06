/**
 * 页面导航助手:集中维护路径,tab 页走 switchTab,其余走 navigateTo。
 */

import Taro from "@tarojs/taro";

export const ROUTES = {
  home: "/pages/index/index",
  learn: "/pages/learn/index",
  lesson: (id: string) => `/pages/lesson/index?id=${encodeURIComponent(id)}`,
  practice: "/pages/practice/index",
  practiceSetup: (mode: string) =>
    `/pages/practice-setup/index?mode=${encodeURIComponent(mode)}`,
  session: (mode: string, src: "normal" | "weak" = "normal") =>
    `/pages/session/index?mode=${encodeURIComponent(mode)}&src=${src}`,
  summary: "/pages/summary/index",
  keyboard: "/pages/keyboard/index",
  keyDetail: (key: string) =>
    `/pages/key-detail/index?key=${encodeURIComponent(key)}`,
  mistakes: "/pages/mistakes/index",
  settings: "/pages/settings/index",
} as const;

export function switchTab(url: string): void {
  Taro.switchTab({ url });
}

export function navigateTo(url: string): void {
  Taro.navigateTo({ url });
}

/** 页面栈可能已满(navigateTo 上限 10 层):失败时降级为重定向。 */
export function navigateToOrRedirect(url: string): void {
  Taro.navigateTo({ url, fail: () => Taro.redirectTo({ url }) });
}

/** 读取当前页参数(Taro.RouterInfo 的松散访问;缺参返回 null)。 */
export function pageParam(name: string): string | null {
  const router = Taro.getCurrentInstance().router;
  const value = router?.params?.[name];
  if (typeof value !== "string" || value === "") return null;
  // navigateTo 二次进入时 params 值可能被编码一次。
  try {
    return decodeURIComponent(value);
  } catch {
    return value;
  }
}
