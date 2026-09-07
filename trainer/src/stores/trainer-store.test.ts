import { beforeEach, describe, expect, it } from "vitest";
import { localDateKey } from "@/lib/stats";
import {
  emptyDailyStats,
  migratePersisted,
  resetTrainerStore,
  sanitizePersisted,
  STORAGE_KEY,
  STORAGE_VERSION,
  useTrainerStore,
} from "./trainer-store";

beforeEach(() => {
  localStorage.clear();
  resetTrainerStore();
});

describe("默认状态", () => {
  it("偏好与进度为全新默认值(V2 含 keyErrors)", () => {
    const state = useTrainerStore.getState();
    expect(state.theme).toBe("system");
    expect(state.hintMode).toBe("always");
    expect(state.difficulty).toBe("daily");
    expect(state.sessionLength).toBe(30);
    expect(state.lastMode).toBe("double");
    expect(state.progress).toEqual({});
    expect(state.daily).toEqual({});
    expect(state.keyErrors).toEqual({});
  });
});

describe("recordQuestionResult", () => {
  it("稀疏创建条目进度并累计当日统计(含汉字数与修正)", () => {
    const now = Date.now();
    useTrainerStore.getState().recordQuestionResult({
      id: "行:xk",
      outcome: "perfect",
      routeUsed: "primary",
      keystrokes: 2,
      wrongKeyEvents: 0,
      wrongKeys: [],
      chars: 1,
      corrections: 0,
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
      avgLatencyMs: 1000,
    });
    const today = state.daily[localDateKey(new Date(now))];
    expect(today).toMatchObject({
      questions: 1,
      keystrokes: 2,
      wrongKeyEvents: 0,
      practiceMs: 1000,
      chars: 1,
      corrections: 0,
    });
  });

  it("按错的键进入键位错误统计", () => {
    const now = Date.now();
    useTrainerStore.getState().recordQuestionResult({
      id: "行:xk",
      outcome: "imperfect",
      routeUsed: "primary",
      keystrokes: 4,
      wrongKeyEvents: 3,
      wrongKeys: ["z", "q", "z"],
      chars: 1,
      corrections: 1,
      practiceMs: 800,
      bestStreak: 0,
      now,
    });
    const state = useTrainerStore.getState();
    expect(state.keyErrors).toEqual({ z: 1, q: 1 });
    expect(state.daily[localDateKey(new Date(now))]).toMatchObject({
      wrongKeyEvents: 3,
      corrections: 1,
    });
  });

  it("备用路线完成按 imperfect 记进度", () => {
    const now = Date.now();
    useTrainerStore.getState().recordQuestionResult({
      id: "shortcut:时间:uij",
      outcome: "imperfect",
      routeUsed: "alternate",
      keystrokes: 4,
      wrongKeyEvents: 0,
      wrongKeys: [],
      chars: 2,
      corrections: 0,
      practiceMs: 2000,
      bestStreak: 0,
      now,
    });
    expect(useTrainerStore.getState().progress["shortcut:时间:uij"]).toMatchObject({
      attempts: 1,
      correct: 0,
      wrong: 1,
    });
  });

  it("同一日多次答题累计", () => {
    const now = Date.now();
    const payload = {
      id: "行:xk",
      routeUsed: "primary" as const,
      keystrokes: 2,
      wrongKeyEvents: 1,
      wrongKeys: ["z"],
      chars: 1,
      corrections: 0,
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
  it("写入 localStorage 时带 version 2 与 keyErrors 字段", () => {
    useTrainerStore.getState().setTheme("dark");
    const raw = localStorage.getItem(STORAGE_KEY);
    expect(raw).not.toBeNull();
    const persisted = JSON.parse(raw!) as { version: number; state: unknown };
    expect(persisted.version).toBe(STORAGE_VERSION);
    expect(persisted.state).toMatchObject({ theme: "dark", keyErrors: {} });
  });

  it("不持久化运行态字段", () => {
    useTrainerStore.getState().setTheme("dark");
    const raw = JSON.parse(localStorage.getItem(STORAGE_KEY)!) as {
      state: Record<string, unknown>;
    };
    expect(Object.keys(raw.state).sort()).toEqual(
      [
        "language",
        "theme",
        "hintMode",
        "difficulty",
        "sessionLength",
        "lastMode",
        "progress",
        "daily",
        "keyErrors",
      ].sort(),
    );
  });
});

describe("V1 → V2 迁移(B9)", () => {
  const v1Fixture = {
    theme: "dark",
    hintMode: "on-error",
    difficulty: "beginner",
    sessionLength: 50,
    lastMode: "mixed",
    progress: {
      "行:xk": {
        attempts: 7,
        correct: 5,
        wrong: 2,
        streak: 1,
        mastery: 55,
        lastSeenAt: 1700000000000,
      },
      broken: { attempts: "nope" },
      empty: { attempts: 0, correct: 0, wrong: 0, streak: 0, mastery: 0, lastSeenAt: null },
    },
    daily: {
      "2026-08-01": {
        practiceMs: 60000,
        questions: 40,
        keystrokes: 320,
        wrongKeyEvents: 9,
        bestStreak: 12,
      },
      garbage: null,
    },
  };

  it("migratePersisted 保留设置/进度/按日统计,新字段取默认值", () => {
    const migrated = migratePersisted(v1Fixture, 1);
    expect(migrated.theme).toBe("dark");
    expect(migrated.hintMode).toBe("on-error");
    expect(migrated.difficulty).toBe("beginner");
    expect(migrated.sessionLength).toBe(50);
    expect(migrated.lastMode).toBe("mixed"); // V1 值仍是合法 V2 模式
    expect(migrated.progress["行:xk"]).toEqual({
      attempts: 7,
      correct: 5,
      wrong: 2,
      streak: 1,
      mastery: 55,
      lastSeenAt: 1700000000000,
      avgLatencyMs: null, // V1 无延迟样本
    });
    expect(migrated.progress.broken).toBeUndefined();
    expect(migrated.progress.empty).toBeUndefined(); // 空进度条目丢弃
    expect(migrated.daily["2026-08-01"]).toEqual({
      practiceMs: 60000,
      questions: 40,
      keystrokes: 320,
      wrongKeyEvents: 9,
      bestStreak: 12,
      chars: 0, // V1 无该字段
      corrections: 0,
    });
    expect(migrated.keyErrors).toEqual({});
  });

  it("V1 迁移补默认语言 zh", () => {
    const migrated = migratePersisted(v1Fixture, 1);
    expect(migrated.language).toBe("zh");
  });

  it("V2 lastMode 值在迁移边界合法", () => {
    const migrated = migratePersisted(
      { ...v1Fixture, lastMode: "sentence" },
      2,
    );
    expect(migrated.lastMode).toBe("sentence");
  });

  it("migratePersisted 对未知版本/损坏数据走校验边界", () => {
    expect(migratePersisted("garbage", 99).progress).toEqual({});
    expect(migratePersisted(null, 1)).toMatchObject({ theme: "system" });
  });
});

describe("language 与 resetItemProgress", () => {
  it("语言偏好可设置并持久化", () => {
    useTrainerStore.getState().setLanguage("en");
    expect(useTrainerStore.getState().language).toBe("en");
    const persisted = JSON.parse(localStorage.getItem(STORAGE_KEY)!) as {
      state: { language: string };
    };
    expect(persisted.state.language).toBe("en");
  });

  it("resetItemProgress 只清指定条目", () => {
    const now = Date.now();
    const payload = {
      id: "行:xk",
      outcome: "perfect" as const,
      routeUsed: "primary" as const,
      keystrokes: 2,
      wrongKeyEvents: 0,
      wrongKeys: [],
      chars: 1,
      corrections: 0,
      practiceMs: 100,
      bestStreak: 1,
      now,
    };
    useTrainerStore.getState().recordQuestionResult(payload);
    useTrainerStore
      .getState()
      .recordQuestionResult({ ...payload, id: "word:我们:womf" });
    useTrainerStore.getState().resetItemProgress(["行:xk"]);
    const state = useTrainerStore.getState();
    expect(state.progress["行:xk"]).toBeUndefined();
    expect(state.progress["word:我们:womf"]).toBeDefined();
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
          avgLatencyMs: 800,
        },
        broken: { attempts: "yes" },
      },
      daily: "not-an-object",
      keyErrors: { z: 2, BAD: 1 },
    });
    expect(sanitized.theme).toBe("dark");
    expect(sanitized.hintMode).toBe("always");
    expect(sanitized.difficulty).toBe("beginner");
    expect(sanitized.sessionLength).toBe(30);
    expect(sanitized.lastMode).toBe("mixed");
    expect(Object.keys(sanitized.progress)).toEqual(["行:xk"]);
    expect(sanitized.daily).toEqual({});
    expect(sanitized.keyErrors).toEqual({ z: 2 });
  });
});

describe("emptyDailyStats", () => {
  it("V2 形状含 chars 与 corrections", () => {
    expect(emptyDailyStats()).toEqual({
      practiceMs: 0,
      questions: 0,
      keystrokes: 0,
      wrongKeyEvents: 0,
      bestStreak: 0,
      chars: 0,
      corrections: 0,
    });
  });
});
