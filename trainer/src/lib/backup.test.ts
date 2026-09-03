import { describe, expect, it } from "vitest";
import {
  BACKUP_KIND,
  BACKUP_VERSION,
  exportBackup,
  importBackup,
} from "./backup";
import { emptyProgress } from "./progress";
import { emptyDailyStats } from "@/stores/trainer-store";
import type { ItemProgress } from "./progress";

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
