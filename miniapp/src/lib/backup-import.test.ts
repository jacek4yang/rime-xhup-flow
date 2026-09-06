/**
 * 备份导入流程测试:解析 / 预览 / 应用分离(内存存储,参照 proof.test.ts)。
 *
 * 覆盖:合法备份(经核心 exportBackup 生成)→ 预览字段与逐字段应用;
 * 损坏 JSON / 错误 kind / 非法版本 / 非法字段 → 类型化失败且状态不变;
 * 超过 MAX_BACKUP_JSON_CHARS → 解析前直接拒绝。
 */

import { describe, expect, it } from "vitest";
import {
  MAX_BACKUP_JSON_CHARS,
  emptyDailyStats,
  exportBackup,
  type StorageAdapter,
} from "@xhup/trainer-core";
import { createAppStore } from "./app-store";
import { defaultAppState } from "./state-schema";
import { parseBackupForImport } from "./backup-import";

/** 内存实现 StorageAdapter(与真机 Taro 实现同一契约)。 */
function memoryStorage(): StorageAdapter & { map: Map<string, string> } {
  const map = new Map<string, string>();
  return {
    map,
    get: (key) => map.get(key) ?? null,
    set: (key, value) => void map.set(key, value),
    remove: (key) => void map.delete(key),
  };
}

/** 用共享核心导出器生成一份合法备份 JSON(与桌面端产物字节同构)。 */
function validBackupJson(): string {
  return exportBackup(
    {
      theme: "light",
      hintMode: "on-error",
      difficulty: "daily",
      sessionLength: 30,
      lastMode: "double",
      progress: {
        "char-2-a": {
          attempts: 12,
          correct: 10,
          wrong: 2,
          streak: 3,
          mastery: 55,
          lastSeenAt: 1788710400000,
          avgLatencyMs: 1400,
        },
      },
      daily: {
        "2026-09-05": {
          ...emptyDailyStats(),
          practiceMs: 720000,
          questions: 30,
          keystrokes: 720,
          wrongKeyEvents: 36,
          bestStreak: 9,
          chars: 240,
          corrections: 12,
        },
        "2026-09-06": { ...emptyDailyStats(), practiceMs: 60000, questions: 5 },
      },
      keyErrors: { j: 7, k: 5 },
      lessonEvidence: {},
      confusions: {},
    },
    1788710400000,
  );
}

describe("parseBackupForImport:解析与预览分离", () => {
  it("合法备份返回预览摘要与可应用数据", () => {
    const result = parseBackupForImport(validBackupJson());
    expect(result.ok).toBe(true);
    if (!result.ok) return;
    expect(result.preview.version).toBe(2);
    expect(result.preview.progressCount).toBe(1);
    expect(result.preview.dailyDays).toBe(2);
    expect(result.preview.dateRange).toEqual(["2026-09-05", "2026-09-06"]);
    expect(result.preview.keyErrorKeys).toBe(2);
    expect(Object.keys(result.data.progress)).toEqual(["char-2-a"]);
  });

  it("损坏 JSON → invalid,原因可读", () => {
    const result = parseBackupForImport("{not json");
    expect(result).toMatchObject({ ok: false, kind: "invalid" });
  });

  it("错误 kind / 版本 / 非法字段 → invalid(核心校验边界)", () => {
    const notBackup = JSON.stringify({ hello: "world" });
    expect(parseBackupForImport(notBackup)).toMatchObject({
      ok: false,
      kind: "invalid",
    });

    const wrongVersion = validBackupJson().replace('"version": 2', '"version": 1');
    expect(parseBackupForImport(wrongVersion)).toMatchObject({
      ok: false,
      kind: "invalid",
    });

    const badProgress = validBackupJson().replace(
      '"mastery": 55',
      '"mastery": -1',
    );
    expect(parseBackupForImport(badProgress)).toMatchObject({
      ok: false,
      kind: "invalid",
    });
  });

  it("超过 MAX_BACKUP_JSON_CHARS 在解析前直接拒绝", () => {
    const oversized = "x".repeat(MAX_BACKUP_JSON_CHARS + 1);
    expect(parseBackupForImport(oversized)).toMatchObject({
      ok: false,
      kind: "tooLarge",
    });
  });
});

describe("应用语义:与 applyBackup 一致的整体替换", () => {
  it("应用后进度/统计/键位错误/混淆被替换,偏好保留", () => {
    const storage = memoryStorage();
    const store = createAppStore(storage, "import.test");

    // 先写入既有状态:一条旧进度 + 一条旧按日统计 + 一个语言偏好。
    store.recordQuestionResult({
      id: "char-3-old",
      outcome: "imperfect",
      keystrokes: 4,
      wrongKeyEvents: 1,
      wrongKeys: ["j"],
      chars: 1,
      corrections: 0,
      practiceMs: 3000,
      bestStreak: 0,
      now: new Date(2026, 8, 1, 10, 0, 0).getTime(),
    });
    store.updateSettings({ language: "en" });

    const parsed = parseBackupForImport(validBackupJson());
    expect(parsed.ok).toBe(true);
    if (!parsed.ok) return;

    store.applyBackup({
      progress: parsed.data.progress,
      daily: parsed.data.daily,
      keyErrors: parsed.data.keyErrors,
      confusions: parsed.data.confusions,
    });

    const state = store.getState();
    expect(Object.keys(state.progress)).toEqual(["char-2-a"]);
    expect(Object.keys(state.daily)).toEqual(["2026-09-05", "2026-09-06"]);
    expect(state.daily["2026-09-06"].questions).toBe(5);
    expect(state.keyErrors).toEqual({ j: 7, k: 5 });
    // 偏好保留:导入不触碰 settings(与 applyBackup 既有语义一致)。
    expect(state.settings.language).toBe("en");
    // 持久化落盘(下次启动可恢复同一状态)。
    const persisted = JSON.parse(storage.map.get("import.test") ?? "{}");
    expect(persisted.state.progress["char-2-a"].mastery).toBe(55);
  });

  it("失败路径不触碰既有状态(解析失败先于任何写入)", () => {
    const storage = memoryStorage();
    const store = createAppStore(storage, "import.test.safe");
    store.updateSettings({ language: "en" });
    const before = store.getState();

    const failed = parseBackupForImport("{broken");
    expect(failed.ok).toBe(false);
    // 未调用 applyBackup ⇒ 状态逐字段不变。
    expect(store.getState()).toEqual(before);
  });

  it("空进度/空按日备份也可导入(新设备首次恢复)", () => {
    const empty = exportBackup(
      {
        theme: "light",
        hintMode: "on-error",
        difficulty: "daily",
        sessionLength: 30,
        lastMode: "double",
        progress: {},
        daily: {},
        keyErrors: {},
      },
      1788710400000,
    );
    const result = parseBackupForImport(empty);
    expect(result.ok).toBe(true);
    if (!result.ok) return;
    expect(result.preview).toMatchObject({
      version: 2,
      progressCount: 0,
      dailyDays: 0,
      dateRange: null,
      keyErrorKeys: 0,
    });
    expect(result.data).toEqual({
      progress: {},
      daily: {},
      keyErrors: {},
      confusions: {},
    });
  });

  it("默认状态可直接作为 applyBackup 的重置基线", () => {
    expect(defaultAppState().daily).toEqual({});
  });
});
