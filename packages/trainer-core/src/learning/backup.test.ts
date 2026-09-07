import { describe, expect, it } from "vitest";
import {
  BACKUP_KIND,
  BACKUP_VERSION,
  MAX_BACKUP_JSON_CHARS,
  exportBackup,
  importBackup,
} from "./backup";
import { emptyProgress } from "./progress";
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

  it("拒绝超过 2 MB 的备份(可操作的错误提示)", () => {
    const backup = exportBackup(sampleData(), 1700);
    expect(backup.length).toBeLessThan(MAX_BACKUP_JSON_CHARS);
    const oversized = " ".repeat(MAX_BACKUP_JSON_CHARS + 1);
    expect(() => importBackup(oversized)).toThrow(/过大/);
    expect(() => importBackup(backup)).not.toThrow();
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
