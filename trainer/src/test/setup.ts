import "@testing-library/jest-dom/vitest";
import { cleanup } from "@testing-library/react";
import { afterEach } from "vitest";

// 未启用 vitest globals,Testing Library 的自动清理需要显式注册。
afterEach(cleanup);

// jsdom 没有 matchMedia:主题控制等逻辑依赖它,提供可控的最小实现。
if (typeof window !== "undefined" && !window.matchMedia) {
  window.matchMedia = ((query: string) => ({
    matches: false,
    media: query,
    onchange: null,
    addEventListener: () => {},
    removeEventListener: () => {},
    addListener: () => {},
    removeListener: () => {},
    dispatchEvent: () => false,
  })) as unknown as typeof window.matchMedia;
}

// Node ≥ 26 把 Storage/localStorage 暴露为 globalThis 自有属性(未提供
// --localstorage-file 时值为 undefined);vitest 的 populateGlobal 见
// 「键已存在」即跳过 jsdom 的 localStorage,导致测试环境缺存储。
// 这里补一个最小内存实现;jsdom 自带 localStorage 时此分支不生效。
if (typeof window !== "undefined" && typeof window.localStorage === "undefined") {
  const store = new Map<string, string>();
  const storage = {
    get length() {
      return store.size;
    },
    clear: () => store.clear(),
    getItem: (key: string) => (store.has(key) ? (store.get(key) as string) : null),
    key: (index: number) => [...store.keys()][index] ?? null,
    removeItem: (key: string) => {
      store.delete(key);
    },
    setItem: (key: string, value: string) => {
      store.set(key, String(value));
    },
  };
  Object.defineProperty(window, "localStorage", {
    value: storage,
    configurable: true,
  });
}
