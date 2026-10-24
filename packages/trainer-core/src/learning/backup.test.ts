import { describe, expect, it } from "vitest";
import {
  BACKUP_KIND,
  BACKUP_VERSION,
  MAX_BACKUP_JSON_CHARS,
  exportBackup,
  importBackup,
} from "./backup";
import { applyQuestionProgress, emptyProgress } from "./progress";
import { emptyDailyStats } from "./daily-stats";
import type { ItemProgress } from "./progress";
import { recordConfusion } from "./confusion";

const sampleData = () => ({
  theme: "dark" as const,
  hintMode: "on-error" as const,
  difficulty: "beginner" as const,
  sessionLength: 50 as const,
  lastMode: "sentence" as const,
  progress: {
    "行:xk": {
      ...emptyProgress(),
      attempts: 3,
      correct: 2,
      wrong: 1,
      mastery: 30,
      lastSeenAt: 123,
      avgLatencyMs: 900,
    } satisfies ItemProgress,
  },
  daily: {
    "2026-09-01": { ...emptyDailyStats(), practiceMs: 1000, questions: 2 },
  },
  keyErrors: { z: 3 },
  lessonEvidence: {
    double: {
      openedAt: 100,
      practiceSessions: 2,
      practiceAttempts: 30,
      lastPracticeAt: 200,
      bestAccuracy: 0.95,
    },
  },
  confusions: recordConfusion(recordConfusion({}, "k", "m", "shape1"), "x", "z", "sound1"),
});

describe("exportBackup", () => {
  it("产出确定性 JSON(同状态同时间戳 → 字节一致)", () => {
    expect(exportBackup(sampleData(), 1700)).toBe(exportBackup(sampleData(), 1700));
  });

  it("包含 kind/version/settings/progress/daily/keyErrors,不含数据集", () => {
    const backup = JSON.parse(exportBackup(sampleData(), 1700));
    expect(backup).toMatchObject({
      kind: BACKUP_KIND,
      version: BACKUP_VERSION,
      createdAt: 1700,
    });
    expect(backup.settings.lastMode).toBe("sentence");
    expect(backup.progress["行:xk"].attempts).toBe(3);
    expect(backup.daily["2026-09-01"].questions).toBe(2);
    expect(backup.keyErrors).toEqual({ z: 3 });
    // 备份绝不包含规范数据集字段
    expect(backup.entries).toBeUndefined();
    expect(backup.words).toBeUndefined();
    expect(backup.doublePinyin).toBeUndefined();
  });
});

describe("importBackup", () => {
  it("导出 → 导入往返一致", () => {
    const restored = importBackup(exportBackup(sampleData(), 1700));
    expect(restored.settings).toEqual({
      theme: "dark",
      hintMode: "on-error",
      difficulty: "beginner",
      sessionLength: 50,
      lastMode: "sentence",
    });
    expect(restored.progress["行:xk"]).toMatchObject({
      attempts: 3,
      wrong: 1,
      avgLatencyMs: 900,
    });
    expect(restored.daily["2026-09-01"].practiceMs).toBe(1000);
    expect(restored.keyErrors).toEqual({ z: 3 });
    expect(restored.lessonEvidence).toEqual({
      double: {
        openedAt: 100,
        practiceSessions: 2,
        practiceAttempts: 30,
        lastPracticeAt: 200,
        bestAccuracy: 0.95,
      },
    });
  });

  it("lessonEvidence 为可选增量字段:缺省导出补 {},旧备份导入得 {}", () => {
    // 未提供 lessonEvidence 的导出(宿主旧状态)补空对象,保持确定性。
    const exported = JSON.parse(exportBackup({ ...sampleData(), lessonEvidence: undefined }, 1700));
    expect(exported.lessonEvidence).toEqual({});

    // 旧版本 2 备份(无该字段)导入 → {}。
    const legacy = JSON.parse(exportBackup(sampleData(), 1700));
    delete legacy.lessonEvidence;
    const restored = importBackup(JSON.stringify(legacy));
    expect(restored.lessonEvidence).toEqual({});
  });

  it("confusions 往返保真;旧备份缺省 → {}(版本 2 可选增量字段)", () => {
    const restored = importBackup(exportBackup(sampleData(), 1700));
    expect(restored.confusions).toEqual(sampleData().confusions);

    const legacy = JSON.parse(exportBackup(sampleData(), 1700));
    delete legacy.confusions;
    expect(importBackup(JSON.stringify(legacy)).confusions).toEqual({});

    const exported = JSON.parse(
      exportBackup({ ...sampleData(), confusions: undefined }, 1700),
    );
    expect(exported.confusions).toEqual({});
  });

  it("confusions 损坏条目单独丢弃,不拒绝整份备份", () => {
    const backup = JSON.parse(exportBackup(sampleData(), 1700));
    backup.confusions["garbage"] = { expected: "K", actual: "mm", position: "middle", count: 1 };
    backup.confusions["negative"] = { expected: "k", actual: "w", position: "sound1", count: -2 };
    const restored = importBackup(JSON.stringify(backup));
    expect(Object.keys(restored.confusions)).toEqual(["k>m@shape1", "x>z@sound1"]);
  });

  it("拒绝超过上限的备份(可操作的错误提示)", () => {
    const backup = exportBackup(sampleData(), 1700);
    expect(backup.length).toBeLessThan(MAX_BACKUP_JSON_CHARS);
    const oversized = " ".repeat(MAX_BACKUP_JSON_CHARS + 1);
    expect(() => importBackup(oversized)).toThrow(/过大/);
    expect(() => importBackup(backup)).not.toThrow();
  });

  it("容量不变量:重度规模(1.2 万条真实结构进度)导出 < 上限且可完整重新导入", () => {
    // 旧 2 MiB 上限在约 4.8k 条即被突破且导出无守卫(导出成功/导入被拒)。
    // 现锚定:12,000 条(60% 含复习对象,EWMA 混合 null)紧凑导出仍远低于
    // 16 MiB,且导入后逐字段保真。
    const progress: Record<string, import("./progress").ItemProgress> = {};
    for (let i = 0; i < 12_000; i += 1) {
      progress[`字${i % 7000}:code${i}`] = {
        attempts: 12, correct: 10, wrong: 2, streak: 3, mastery: 55 + (i % 40),
        lastSeenAt: 1_788_710_400_000, avgLatencyMs: 1400,
        accuracyEwma: i % 3 === 0 ? null : 0.85,
        review: i % 5 < 3 ? {
          version: 1, level: i % 6, dueAt: 1_788_710_400_000,
          lastAttemptAt: 1_788_710_400_000,
          lastIndependentAt: i % 2 ? 1_788_710_400_000 : null,
          lastResult: "recalled", questionMs: 3000, wrongKeyEvents: 0,
          usedAnswerHint: false, routeUsed: "primary", lapses: i % 2,
        } : null,
      };
    }
    const backup = exportBackup({ ...sampleData(), progress }, 1700);
    expect(backup.length).toBeGreaterThan(2 * 1024 * 1024); // 旧上限必被突破
    expect(backup.length).toBeLessThan(MAX_BACKUP_JSON_CHARS);
    const restored = importBackup(backup);
    expect(Object.keys(restored.progress)).toHaveLength(12_000);
    // i=0:EWMA null(i%3===0)且含复习对象;i=2:EWMA 0.85;i=8:无复习对象(8%5≥3)。
    expect(restored.progress["字0:code0"]!).toMatchObject({ attempts: 12, accuracyEwma: null });
    expect(restored.progress["字0:code0"]!.review).toMatchObject({ level: 0, lastResult: "recalled" });
    expect(restored.progress["字2:code2"]!.accuracyEwma).toBeCloseTo(0.85);
    expect(restored.progress["字8:code8"]!.review).toBeNull();
  });

  it("导出守卫:超出上限的数据集导出直接失败,不产出无法重导入的备份", () => {
    // ~60k 条含复习对象的条目 ≈ >16 MiB(紧凑),导出必须拒绝。
    const progress: Record<string, import("./progress").ItemProgress> = {};
    for (let i = 0; i < 60_000; i += 1) {
      progress[`字${i}:code${i}`] = {
        attempts: 12, correct: 10, wrong: 2, streak: 3, mastery: 60,
        lastSeenAt: 1_788_710_400_000, avgLatencyMs: 1400, accuracyEwma: 0.85,
        review: {
          version: 1, level: 2, dueAt: 1_788_710_400_000,
          lastAttemptAt: 1_788_710_400_000, lastIndependentAt: 1_788_710_400_000,
          lastResult: "recalled", questionMs: 3000, wrongKeyEvents: 0,
          usedAnswerHint: false, routeUsed: "primary", lapses: 1,
        },
      };
    }
    expect(() => exportBackup({ ...sampleData(), progress }, 1700))
      .toThrow(/备份过大/);
  });

  it("前向兼容边界:未知新增字段被当前与旧版本一致忽略(只读已知字段)", () => {
    const backup = JSON.parse(exportBackup(sampleData(), 1700));
    backup.progress["行:xk"].futureField = { anything: true };
    backup.futureSection = "unknown";
    const restored = importBackup(JSON.stringify(backup));
    expect(restored.progress["行:xk"].attempts).toBe(3);
  });

  it("拒绝非法 lessonEvidence 结构", () => {
    const backup = JSON.parse(exportBackup(sampleData(), 1700));
    expect(() =>
      importBackup(JSON.stringify({ ...backup, lessonEvidence: "nope" })),
    ).toThrow(/lessonEvidence 结构无效/);
    expect(() =>
      importBackup(
        JSON.stringify({
          ...backup,
          lessonEvidence: { double: { practiceSessions: -1 } },
        }),
      ),
    ).toThrow(/应为非负整数/);
    expect(() =>
      importBackup(
        JSON.stringify({
          ...backup,
          lessonEvidence: { double: { bestAccuracy: 1.5 } },
        }),
      ),
    ).toThrow(/bestAccuracy 应在 0..1/);
  });

  it("拒绝损坏 JSON 与未知结构", () => {
    expect(() => importBackup("{not json")).toThrow(BackupError);
    expect(() => importBackup("null")).toThrow(/结构无效/);
    expect(() => importBackup(JSON.stringify({ kind: "other" }))).toThrow(/不是训练器备份/);
  });

  it("拒绝错误版本(含旧版本)", () => {
    const backup = JSON.parse(exportBackup(sampleData(), 1700));
    expect(() =>
      importBackup(JSON.stringify({ ...backup, version: 1 })),
    ).toThrow(/版本应为 2/);
  });

  it("拒绝非法进度与键位统计", () => {
    const backup = JSON.parse(exportBackup(sampleData(), 1700));
    expect(() =>
      importBackup(
        JSON.stringify({
          ...backup,
          progress: { "行:xk": { attempts: -1 } },
        }),
      ),
    ).toThrow(/非法数值/);
    expect(() =>
      importBackup(JSON.stringify({ ...backup, keyErrors: { Z: 1 } })),
    ).toThrow(/键非法/);
  });

  it("拒绝缺失 settings 字段", () => {
    const backup = JSON.parse(exportBackup(sampleData(), 1700));
    delete (backup.settings as Record<string, unknown>).theme;
    expect(() => importBackup(JSON.stringify(backup))).toThrow(/settings\.theme/);
  });
});

import { BackupError } from "./backup";

describe("跨天复习备份迁移", () => {
  it("V2 历史高掌握度保留,但不能伪造独立回忆证据", () => {
    const legacy = JSON.parse(exportBackup(sampleData(), 1700));
    legacy.version = 2;
    legacy.progress["行:xk"].mastery = 100;
    delete legacy.progress["行:xk"].review;
    const restored = importBackup(JSON.stringify(legacy));
    expect(restored.sourceVersion).toBe(2);
    expect(restored.progress["行:xk"]).toMatchObject({
      attempts: 3, correct: 2, wrong: 1, mastery: 100, review: null,
    });
  });

  it("V3 跨端导入保留到期、路线和提示证据,损坏复习状态不能静默导入", () => {
    const progress = applyQuestionProgress(emptyProgress(), {
      outcome: "imperfect", routeUsed: "alternate", independentRecall: false,
      usedAnswerHint: true, wrongKeyEvents: 2, questionMs: 9000, chars: 1, now: 1700,
    });
    const data = { ...sampleData(), progress: { "行:xk": progress } };
    const json = exportBackup(data, 1800);
    const restored = importBackup(json);
    expect(restored.sourceVersion).toBe(3);
    expect(restored.progress["行:xk"].review).toEqual({
      version: 1, level: 0, dueAt: 601700, lastAttemptAt: 1700,
      lastIndependentAt: null, lastResult: "hinted", questionMs: 9000,
      wrongKeyEvents: 2, usedAnswerHint: true, routeUsed: "alternate", lapses: 1,
    });
    const corrupt = JSON.parse(json);
    corrupt.progress["行:xk"].review.dueAt = -1;
    expect(() => importBackup(JSON.stringify(corrupt))).toThrow(BackupError);
  });
});
