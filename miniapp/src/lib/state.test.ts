/**
 * 持久化状态测试:V2 文档往返、逐字段损坏降级、旧版进度迁移、
 * store 动作与按日统计聚合。全部走内存 StorageAdapter(node 环境)。
 */

import { describe, expect, it } from "vitest";
import {
  DEFAULT_SESSION_LENGTH,
  DEFAULT_HINT_MODE,
  applyPerfect,
  emptyDailyStats,
  emptyProgress,
  recordConfusion,
  type StorageAdapter,
} from "@xhup/trainer-core";
import {
  STORE_VERSION,
  defaultAppState,
  migrateLegacyProgress,
  parsePersistedDocument,
  sanitizePersisted,
  serializeDocument,
} from "./state-schema";
import { createAppStore } from "./app-store";

function memoryStorage(): StorageAdapter & { dump(): Map<string, string> } {
  const memory = new Map<string, string>();
  return {
    get: (key) => memory.get(key) ?? null,
    set: (key, value) => void memory.set(key, value),
    remove: (key) => void memory.delete(key),
    dump: () => memory,
  };
}

describe("状态 Schema 与持久化校验边界", () => {
  it("默认状态:偏好与桌面端默认值一致", () => {
    const state = defaultAppState();
    expect(state.settings.hintMode).toBe(DEFAULT_HINT_MODE);
    expect(state.settings.sessionLength).toBe(DEFAULT_SESSION_LENGTH);
    expect(state.progress).toEqual({});
    expect(state.daily).toEqual({});
    expect(state.keyErrors).toEqual({});
  });

  it("serialize → parse 往返保真", () => {
    const state = defaultAppState();
    state.settings.language = "en";
    state.progress["的:de"] = { ...emptyProgress(), attempts: 3, correct: 2, wrong: 1, mastery: 20, lastSeenAt: 42 };
    state.daily["2026-09-06"] = { ...emptyDailyStats(), questions: 5, keystrokes: 18 };
    state.keyErrors = { a: 2, q: 1 };
    state.confusions = recordConfusion(recordConfusion({}, "k", "m", "shape1"), "x", "z", "sound1");

    const raw = serializeDocument(state);
    const restored = parsePersistedDocument(raw);
    expect(restored).not.toBeNull();
    expect(restored!.settings.language).toBe("en");
    expect(restored!.progress["的:de"]!.attempts).toBe(3);
    expect(restored!.daily["2026-09-06"]!.keystrokes).toBe(18);
    expect(restored!.keyErrors).toEqual({ a: 2, q: 1 });
    expect(restored!.confusions).toEqual(state.confusions);
  });

  it("损坏 JSON 返回 null(调用方回退默认值)", () => {
    expect(parsePersistedDocument("not json {")).toBeNull();
    expect(parsePersistedDocument(null)).toBeNull();
    expect(parsePersistedDocument("")).toBeNull();
    expect(parsePersistedDocument("42")).toBeNull();
  });

  it("逐字段降级:坏字段回退默认,好字段保留,绝不整体清空", () => {
    const sanitized = sanitizePersisted({
      version: STORE_VERSION,
      state: {
        settings: {
          language: "fr", // 非法 → 默认 zh
          hintMode: "always", // 合法 → 保留
          sessionLength: 33, // 非法 → 默认
          lastMode: "sentence", // 合法 → 保留
        },
        progress: {
          "的:de": { attempts: 2, correct: 1, wrong: 1 }, // 合法,其余字段回退默认
          "坏条目": { attempts: "no" }, // 非法 → 丢弃
          "零条目": { attempts: 0 }, // 空条目 → 丢弃
        },
        daily: {
          "2026-09-06": { questions: 4, practiceMs: 60_000 }, // 部分字段缺失 → 补默认
          "坏日期": "oops", // 非法 → 空统计
        },
        keyErrors: { a: 3, BAD: 1, zz: -2 }, // 非法键/负值 → 过滤
      },
    });

    expect(sanitized.settings.language).toBe("zh");
    expect(sanitized.settings.hintMode).toBe("always");
    expect(sanitized.settings.sessionLength).toBe(DEFAULT_SESSION_LENGTH);
    expect(sanitized.settings.lastMode).toBe("sentence");
    expect(sanitized.progress["的:de"]).toEqual({
      attempts: 2, correct: 1, wrong: 1, streak: 0, mastery: 0, lastSeenAt: null, avgLatencyMs: null,
    });
    expect(Object.keys(sanitized.progress)).toEqual(["的:de"]);
    expect(sanitized.daily["2026-09-06"]).toEqual({
      ...emptyDailyStats(), questions: 4, practiceMs: 60_000,
    });
    expect(sanitized.keyErrors).toEqual({ a: 3 });
  });

  it("confusions:损坏条目逐条丢弃,好条目保留,缺省回 {}", () => {
    const sanitized = sanitizePersisted({
      state: {
        confusions: {
          "k>m@shape1": { expected: "k", actual: "m", position: "shape1", count: 3 },
          bad: { expected: "k", actual: "MM", position: "shape1", count: 1 },
          worse: "nope",
        },
      },
    });
    expect(Object.keys(sanitized.confusions)).toEqual(["k>m@shape1"]);
    expect(sanitizePersisted(null).confusions).toEqual({});
    expect(defaultAppState().confusions).toEqual({});
  });

  it("V1 旧进度迁移:可恢复条目保留,新字段取默认", () => {
    const migrated = migrateLegacyProgress({
      "的:de": { attempts: 5, correct: 4, wrong: 1, streak: 2, mastery: 30, lastSeenAt: 100 },
      "坏": null,
    });
    expect(migrated["的:de"]).toEqual({
      attempts: 5, correct: 4, wrong: 1, streak: 2, mastery: 30, lastSeenAt: 100, avgLatencyMs: null,
    });
    expect(Object.keys(migrated)).toEqual(["的:de"]);
  });
});

describe("应用状态仓库(存储注入)", () => {
  it("recordQuestionResult:进度 + 键位错误 + 按日统计一次落账并持久化", () => {
    const storage = memoryStorage();
    const store = createAppStore(storage, "test.state.v2");
    const now = new Date(2026, 8, 6, 12, 0, 0).getTime();

    store.recordQuestionResult({
      id: "的:de",
      outcome: "perfect",
      keystrokes: 2,
      wrongKeyEvents: 0,
      wrongKeys: [],
      chars: 1,
      corrections: 0,
      practiceMs: 4_000,
      bestStreak: 1,
      now,
    });
    store.recordQuestionResult({
      id: "的:de",
      outcome: "imperfect",
      keystrokes: 3,
      wrongKeyEvents: 1,
      wrongKeys: ["q"],
      confusions: recordConfusion({}, "k", "m", "shape1"),
      chars: 1,
      corrections: 1,
      practiceMs: 6_000,
      bestStreak: 1,
      now: now + 30_000,
    });

    const state = store.getState();
    const expected = applyPerfect(emptyProgress(), now, 4_000);
    // 第二次 imperfect 在 perfect 结果之上。
    const afterPerfect = applyPerfect(emptyProgress(), now, 4_000);
    expect(state.progress["的:de"]!.attempts).toBe(afterPerfect.attempts + 1);
    expect(state.progress["的:de"]!.wrong).toBe(1);
    expect(state.keyErrors).toEqual({ q: 1 });

    const dateKey = "2026-09-06";
    expect(state.daily[dateKey]!.questions).toBe(2);
    expect(state.daily[dateKey]!.keystrokes).toBe(5);
    expect(state.daily[dateKey]!.wrongKeyEvents).toBe(1);
    expect(state.daily[dateKey]!.practiceMs).toBe(10_000);
    expect(state.daily[dateKey]!.chars).toBe(2);
    expect(state.daily[dateKey]!.corrections).toBe(1);
    // 数学与直接调用共享核心纯函数一致。
    expect(expected.attempts).toBe(1);

    // 已持久化:新 store 实例从同一存储读到相同状态。
    const reloaded = createAppStore(storage, "test.state.v2");
    expect(reloaded.getState().progress["的:de"]!.attempts).toBe(2);
    expect(reloaded.getState().daily[dateKey]!.keystrokes).toBe(5);
  });

  it("损坏存储回退默认值,不崩溃", () => {
    const storage = memoryStorage();
    storage.set("test.state.v2", "{{{broken");
    const store = createAppStore(storage, "test.state.v2");
    expect(store.getState().settings.language).toBe("zh");
    expect(store.getState().progress).toEqual({});
  });

  it("recordQuestionResult:混淆对按题合并落库并持久化(缺省载荷不破坏)", () => {
    const storage = memoryStorage();
    const store = createAppStore(storage, "test.state.v2");
    const now = new Date(2026, 8, 6, 12, 0, 0).getTime();
    const base = {
      id: "好:hknc",
      outcome: "imperfect" as const,
      keystrokes: 5,
      wrongKeyEvents: 1,
      wrongKeys: ["m"],
      chars: 1,
      corrections: 0,
      practiceMs: 600,
      bestStreak: 0,
    };
    store.recordQuestionResult({ ...base, confusions: recordConfusion({}, "n", "m", "shape1"), now });
    store.recordQuestionResult({
      ...base,
      confusions: recordConfusion(recordConfusion({}, "n", "m", "shape1"), "c", "d", "shape2"),
      now: now + 1_000,
    });
    // 未带 confusions 的旧调用方:合并结果不变。
    store.recordQuestionResult({ ...base, now: now + 2_000 });

    const confusions = store.getState().confusions;
    expect(confusions["n>m@shape1"]!.count).toBe(2);
    expect(confusions["c>d@shape2"]!.count).toBe(1);

    // 已持久化:新 store 实例读到相同聚合。
    const reloaded = createAppStore(storage, "test.state.v2");
    expect(reloaded.getState().confusions["n>m@shape1"]!.count).toBe(2);
  });

  it("resetProgress 清进度保留偏好;updateSettings 持久化", () => {
    const storage = memoryStorage();
    const store = createAppStore(storage, "test.state.v2");
    store.updateSettings({ language: "en", lastMode: "sentence" });
    store.recordQuestionResult({
      id: "的:de", outcome: "perfect", keystrokes: 2, wrongKeyEvents: 0,
      wrongKeys: [], chars: 1, corrections: 0, practiceMs: 1000, bestStreak: 1,
      now: new Date(2026, 8, 6).getTime(),
    });
    store.resetProgress();

    expect(store.getState().progress).toEqual({});
    expect(store.getState().settings.language).toBe("en");
    expect(store.getState().settings.lastMode).toBe("sentence");

    const reloaded = createAppStore(storage, "test.state.v2");
    expect(reloaded.getState().settings.language).toBe("en");
    expect(reloaded.getState().progress).toEqual({});
  });

  it("订阅:动作后通知监听者", () => {
    const store = createAppStore(memoryStorage(), "test.state.v2");
    let notified = 0;
    const unsubscribe = store.subscribe(() => {
      notified += 1;
    });
    store.updateSettings({ haptics: "off" });
    expect(notified).toBe(1);
    unsubscribe();
    store.updateSettings({ haptics: "medium" });
    expect(notified).toBe(1);
  });
});
