/**
 * 训练进度备份:确定性 JSON 导出 / 校验导入(纯逻辑;本地文件)。
 *
 * 备份只包含用户进度与偏好,绝不包含规范数据集(数据集由 Rust 重新
 * 生成);时间戳由调用方注入以保持导出确定性。导入按版本迁移:仅接受
 * 当前版本 2;损坏 / 未知结构抛 {@link BackupError},调用方展示原因。
 */

import type { ItemProgress } from "@/lib/progress";
import type { DailyStats } from "@/stores/trainer-store";
import type { Difficulty } from "@/lib/trainer-index";
import type { HintMode, PracticeMode, SessionLength } from "@/features/practice/types";

export const BACKUP_KIND = "xhup-flow-trainer-backup";
export const BACKUP_VERSION = 2;

/** 备份携带的用户偏好(与 store 的偏好字段一致)。 */
export type BackupSettings = {
  theme: string;
  hintMode: HintMode;
  difficulty: Difficulty;
  sessionLength: SessionLength;
  lastMode: PracticeMode;
};

/** 训练进度备份文档。 */
export type TrainerBackup = {
  kind: typeof BACKUP_KIND;
  version: typeof BACKUP_VERSION;
  /** 导出时间戳(调用方注入;仅记录,不参与校验)。 */
  createdAt: number;
  settings: BackupSettings;
  /** 稀疏条目进度。 */
  progress: Record<string, ItemProgress>;
  /** 按日统计。 */
  daily: Record<string, DailyStats>;
  /** 键位累积错误(V2 新增)。 */
  keyErrors: Record<string, number>;
};

/** 备份错误:信息面向用户。 */
export class BackupError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "BackupError";
  }
}

/** 导出备份为格式化 JSON(确定性:相同状态 + 相同时间戳 → 字节一致)。 */
export function exportBackup(
  data: {
    theme: string;
    hintMode: HintMode;
    difficulty: Difficulty;
    sessionLength: SessionLength;
    lastMode: PracticeMode;
    progress: Record<string, ItemProgress>;
    daily: Record<string, DailyStats>;
    keyErrors: Record<string, number>;
  },
  now: number,
): string {
  const backup: TrainerBackup = {
    kind: BACKUP_KIND,
    version: BACKUP_VERSION,
    createdAt: now,
    settings: {
      theme: data.theme,
      hintMode: data.hintMode,
      difficulty: data.difficulty,
      sessionLength: data.sessionLength,
      lastMode: data.lastMode,
    },
    progress: data.progress,
    daily: data.daily,
    keyErrors: data.keyErrors,
  };
  return `${JSON.stringify(backup, null, 2)}\n`;
}

function fail(reason: string): never {
  throw new BackupError(reason);
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function validateProgress(value: unknown): Record<string, ItemProgress> {
  if (!isRecord(value)) fail("progress 结构无效");
  const progress: Record<string, ItemProgress> = {};
  for (const [id, raw] of Object.entries(value)) {
    if (!isRecord(raw)) fail(`进度条目 ${id} 结构无效`);
    progress[id] = {
      attempts: numberField(raw.attempts, id),
      correct: numberField(raw.correct, id),
      wrong: numberField(raw.wrong, id),
      streak: numberField(raw.streak, id),
      mastery: numberField(raw.mastery, id),
      lastSeenAt: typeof raw.lastSeenAt === "number" ? raw.lastSeenAt : null,
      avgLatencyMs:
        typeof raw.avgLatencyMs === "number" ? raw.avgLatencyMs : null,
    };
    // 空进度条目丢弃(备份只存见过的条目)。
    if (progress[id].attempts === 0) {
      delete progress[id];
    }
  }
  return progress;
}

function numberField(value: unknown, id: string): number {
  if (typeof value !== "number" || !Number.isFinite(value) || value < 0) {
    fail(`进度条目 ${id} 含非法数值字段`);
  }
  return value;
}

function validateDaily(value: unknown): Record<string, DailyStats> {
  if (!isRecord(value)) fail("daily 结构无效");
  const daily: Record<string, DailyStats> = {};
  for (const [dateKey, raw] of Object.entries(value)) {
    if (!/^\d{4}-\d{2}-\d{2}$/.test(dateKey)) fail(`daily 日期键非法:${dateKey}`);
    if (!isRecord(raw)) fail(`daily[${dateKey}] 结构无效`);
    daily[dateKey] = {
      practiceMs: nonNegative(raw.practiceMs, dateKey),
      questions: nonNegative(raw.questions, dateKey),
      keystrokes: nonNegative(raw.keystrokes, dateKey),
      wrongKeyEvents: nonNegative(raw.wrongKeyEvents, dateKey),
      bestStreak: nonNegative(raw.bestStreak, dateKey),
      chars: nonNegative(raw.chars, dateKey),
      corrections: nonNegative(raw.corrections, dateKey),
    };
  }
  return daily;
}

function nonNegative(value: unknown, at: string): number {
  if (typeof value !== "number" || !Number.isFinite(value) || value < 0) {
    fail(`daily[${at}] 含非法数值字段`);
  }
  return value;
}

function validateKeyErrors(value: unknown): Record<string, number> {
  if (!isRecord(value)) fail("keyErrors 结构无效");
  const keyErrors: Record<string, number> = {};
  for (const [key, count] of Object.entries(value)) {
    if (!/^[a-z]$/.test(key)) fail(`keyErrors 键非法:${key}`);
    if (typeof count !== "number" || !Number.isFinite(count) || count < 0) {
      fail(`keyErrors[${key}] 应为非负数`);
    }
    keyErrors[key] = count;
  }
  return keyErrors;
}

/**
 * 校验并导入备份;返回可直接并入 store 的数据。
 * 只接受版本 2(旧版本无备份格式,不存在迁移路径)。
 */
export function importBackup(json: string): {
  settings: BackupSettings;
  progress: Record<string, ItemProgress>;
  daily: Record<string, DailyStats>;
  keyErrors: Record<string, number>;
} {
  let parsed: unknown;
  try {
    parsed = JSON.parse(json);
  } catch (cause) {
    fail(`备份不是合法 JSON:${String(cause)}`);
  }
  if (!isRecord(parsed)) fail("备份结构无效");
  if (parsed.kind !== BACKUP_KIND) fail("这不是训练器备份文件");
  if (parsed.version !== BACKUP_VERSION) {
    fail(`备份版本应为 ${BACKUP_VERSION}(实际 ${String(parsed.version)})`);
  }
  if (typeof parsed.createdAt !== "number") fail("备份缺少 createdAt");
  if (!isRecord(parsed.settings)) fail("备份缺少 settings");
  const { settings } = parsed;
  for (const field of ["theme", "hintMode", "difficulty", "lastMode"] as const) {
    if (typeof settings[field] !== "string") {
      fail(`settings.${field} 应为字符串`);
    }
  }
  if (
    typeof settings.sessionLength !== "number" ||
    !Number.isInteger(settings.sessionLength) ||
    settings.sessionLength < 0
  ) {
    fail("settings.sessionLength 应为非负整数");
  }
  return {
    settings: {
      theme: settings.theme as string,
      hintMode: settings.hintMode as HintMode,
      difficulty: settings.difficulty as Difficulty,
      sessionLength: settings.sessionLength as SessionLength,
      lastMode: settings.lastMode as PracticeMode,
    },
    progress: validateProgress(parsed.progress),
    daily: validateDaily(parsed.daily),
    keyErrors: validateKeyErrors(parsed.keyErrors),
  };
}
