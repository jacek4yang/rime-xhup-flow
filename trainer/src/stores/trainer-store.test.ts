import { beforeEach, describe, expect, it } from "vitest";
import { localDateKey } from "@/lib/stats";
import {
  resetTrainerStore,
  sanitizePersisted,
  STORAGE_KEY,
  useTrainerStore,
} from "./trainer-store";

beforeEach(() => {
  localStorage.clear();
  resetTrainerStore();
});

describe("默认状态", () => {
  it("偏好与进度为全新默认值", () => {
    const state = useTrainerStore.getState();
    expect(state.theme).toBe("system");
    expect(state.hintMode).toBe("always");
    expect(state.difficulty).toBe("daily");
    expect(state.sessionLength).toBe(30);
    expect(state.lastMode).toBe("double");
    expect(state.progress).toEqual({});
    expect(state.daily).toEqual({});
  });
});

describe("recordQuestionResult", () => {
  it("稀疏创建条目进度并累计当日统计", () => {
    const now = Date.now();
    useTrainerStore.getState().recordQuestionResult({
      id: "行:xk",
      outcome: "perfect",
      keystrokes: 2,
      wrongKeyEvents: 0,
      practiceMs: 1000,
      bestStreak: 1,
      now,
    });
    const state = useTrainerStore.getState();
    expect(Object.keys(state.progress)).toEqual(["行:xk"]);
    expect(state.progress["行:xk"]).toMatchObject({
      attempts: 1,
      correct: 1,
      streak: 1,
    });
    const today = state.daily[localDateKey(new Date(now))];
    expect(today).toMatchObject({
      questions: 1,
      keystrokes: 2,
      wrongKeyEvents: 0,
      practiceMs: 1000,
      bestStreak: 1,
    });
  });

  it("同一日多次答题累计", () => {
    const now = Date.now();
    const payload = {
      id: "行:xk",
      keystrokes: 2,
      wrongKeyEvents: 1,
      practiceMs: 500,
      bestStreak: 3,
      now,
    };
    useTrainerStore
      .getState()
      .recordQuestionResult({ ...payload, outcome: "imperfect" });
    useTrainerStore
      .getState()
      .recordQuestionResult({ ...payload, outcome: "perfect" });
    const today = useTrainerStore.getState().daily[localDateKey(new Date(now))];
    expect(today?.questions).toBe(2);
    expect(today?.keystrokes).toBe(4);
    expect(today?.wrongKeyEvents).toBe(2);
    expect(today?.bestStreak).toBe(3);
  });
});

describe("addPracticeTime", () => {
  it("只累计时长,不产生题数", () => {
    const now = Date.now();
    useTrainerStore.getState().addPracticeTime(2000, now);
    const today = useTrainerStore.getState().daily[localDateKey(new Date(now))];
    expect(today?.practiceMs).toBe(2000);
    expect(today?.questions).toBe(0);
  });
});

describe("持久化", () => {
  it("写入 localStorage 时带 version 1", () => {
    useTrainerStore.getState().setTheme("dark");
    const raw = localStorage.getItem(STORAGE_KEY);
    expect(raw).not.toBeNull();
    const persisted = JSON.parse(raw!) as { version: number; state: unknown };
    expect(persisted.version).toBe(1);
    expect(persisted.state).toMatchObject({ theme: "dark" });
  });

  it("不持久化运行态字段", () => {
    useTrainerStore.getState().setTheme("dark");
    const raw = JSON.parse(localStorage.getItem(STORAGE_KEY)!) as {
      state: Record<string, unknown>;
    };
    expect(Object.keys(raw.state).sort()).toEqual(
      [
        "theme",
        "hintMode",
        "difficulty",
        "sessionLength",
        "lastMode",
        "progress",
        "daily",
      ].sort(),
    );
  });
});

describe("resetProgress", () => {
  it("清空进度与练习偏好,保留主题", () => {
    const store = useTrainerStore.getState();
    store.setTheme("dark");
    store.setHintMode("hidden");
    store.recordQuestionResult({
      id: "行:xk",
      outcome: "perfect",
      keystrokes: 2,
      wrongKeyEvents: 0,
      practiceMs: 1000,
      bestStreak: 1,
      now: Date.now(),
    });
    useTrainerStore.getState().resetProgress();
    const state = useTrainerStore.getState();
    expect(state.theme).toBe("dark");
    expect(state.hintMode).toBe("always");
    expect(state.progress).toEqual({});
    expect(state.daily).toEqual({});
  });
});

describe("sanitizePersisted(损坏/旧数据回退)", () => {
  it("完全损坏时回退默认", () => {
    expect(sanitizePersisted(null)).toMatchObject({ theme: "system" });
    expect(sanitizePersisted("garbage")).toMatchObject({ sessionLength: 30 });
    expect(sanitizePersisted(42).progress).toEqual({});
  });

  it("逐字段回退:可信字段保留,不可信字段回默认", () => {
    const sanitized = sanitizePersisted({
      theme: "dark",
      hintMode: "nonsense",
      difficulty: "beginner",
      sessionLength: 999,
      lastMode: "mixed",
      progress: {
        "行:xk": {
          attempts: 1,
          correct: 1,
          wrong: 0,
          streak: 1,
          mastery: 4,
          lastSeenAt: 123,
        },
        broken: { attempts: "yes" },
      },
      daily: "not-an-object",
    });
    expect(sanitized.theme).toBe("dark");
    expect(sanitized.hintMode).toBe("always");
    expect(sanitized.difficulty).toBe("beginner");
    expect(sanitized.sessionLength).toBe(30);
    expect(sanitized.lastMode).toBe("mixed");
    expect(Object.keys(sanitized.progress)).toEqual(["行:xk"]);
    expect(sanitized.daily).toEqual({});
  });
});
