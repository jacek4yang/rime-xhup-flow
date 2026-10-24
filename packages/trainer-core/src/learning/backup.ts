/**
 * 训练进度备份:确定性 JSON 导出 / 校验导入(纯逻辑;本地文件)。
 *
 * 备份只包含用户进度与偏好,绝不包含规范数据集(数据集由 Rust 重新
 * 生成);时间戳由调用方注入以保持导出确定性。V3 保存跨天复习证据,
 * V2 记录迁移为未验证回忆;损坏 / 未知结构抛 {@link BackupError}。
 * 备份体积超过 {@link MAX_BACKUP_JSON_CHARS} 直接拒绝(可操作的错误提示),
 * 防止误选超大文件卡死解析;导出侧同样拒绝超出上限的结果 ——
 * 当前版本导出的合法备份必须能被当前版本重新导入(容量不变量)。
 */

import type { ItemProgress } from "./progress";
import { parseSpacedReview } from "./spaced-review";
import type { DailyStats } from "./daily-stats";
import type { ConfusionMap } from "./confusion";
import { sanitizeConfusionMap } from "./confusion";
import type { Difficulty } from "../data/trainer-index";
import type { HintMode, PracticeMode, SessionLength } from "../practice/types";
import type { LessonEvidence } from "../learning-path/lesson-state";

export const BACKUP_KIND = "xhup-flow-trainer-backup";
/**
 * 共享备份文档版本(V3:跨天复习证据)。与桌面 STORAGE_VERSION / 小程序
 * STORE_VERSION 是三套独立版本系统:备份文档是跨端交换格式,不随宿主
 * 本机存储的架构演进升版;字段级兼容策略见下方版本策略注释。
 */
export const BACKUP_VERSION = 3;

/**
 * 备份 JSON 字符长度上限(16 MiB,按 UTF-16 码元计)。
 * 实测锚定(2026-10-24,桌面 Node 26):真实结构的稀疏进度约 290 字节
 * /条(紧凑序列化,60% 含复习对象),重度用户 2 万条约 5.7 MB;43k 条
 * (≈ 全部单字+词条数据集)约 16.7 MB,JSON.parse 约 100 ms —— 对显式
 * 的用户导入动作完全可接受。旧 2 MiB 上限在约 4.8k 条(重度用户数月)
 * 即被突破,且导出侧无守卫,曾出现「导出成功但自家导入拒绝」的
 * 不变量违反;现两端一致拒绝。
 */
export const MAX_BACKUP_JSON_CHARS = 16 * 1024 * 1024;

/**
 * 版本策略(里程碑 44):`lessonEvidence` 作为「可选字段」加入版本 2。
 * 旧备份缺该字段 → 导入为 `{}`(进度零丢失);带该字段的备份被旧版本
 * 应用导入时,旧代码只读已知字段、自然忽略多余字段,不破坏前向兼容。
 * 字段为纯增量展示型证据,不参与任何评分正确性判定,因此无需升版。
 *
 * `confusions`(键位混淆聚合)同理:版本 2 的可选增量字段,缺失 → {},
 * 存在 → 逐条校验(损坏条目单独丢弃,绝不因个别条目拒绝整份备份)。
 */

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
  /** 章节学习证据(可选:旧备份缺省 → {};里程碑 44 增量字段)。 */
  lessonEvidence?: Record<string, LessonEvidence>;
  /** 键位混淆聚合(可选:旧备份缺省 → {};紧凑聚合,非原始日志)。 */
  confusions?: ConfusionMap;
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
    language?: string;
    theme: string;
    hintMode: HintMode;
    difficulty: Difficulty;
    sessionLength: SessionLength;
    lastMode: PracticeMode;
    progress: Record<string, ItemProgress>;
    daily: Record<string, DailyStats>;
    keyErrors: Record<string, number>;
    lessonEvidence?: Record<string, LessonEvidence>;
    confusions?: ConfusionMap;
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
    lessonEvidence: data.lessonEvidence ?? {},
    confusions: data.confusions ?? {},
  };
  // 紧凑序列化:机器消费的备份文档,冗余空白只损耗一半容量。
  // 导出守卫:绝不出产出当前版本无法重新导入的备份(容量不变量)。
  const json = `${JSON.stringify(backup)}\n`;
  if (json.length > MAX_BACKUP_JSON_CHARS) {
    fail(`导出的备份过大(${json.length} 字符,上限 ${MAX_BACKUP_JSON_CHARS});进度条目过多,请联系开发者`);
  }
  return json;
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
    const review = parseSpacedReview(raw.review);
    if (raw.review != null && review === null) fail(`进度条目 ${id} 的复习状态无效`);
    progress[id] = {
      attempts: numberField(raw.attempts, id),
      correct: numberField(raw.correct, id),
      wrong: numberField(raw.wrong, id),
      streak: numberField(raw.streak, id),
      mastery: numberField(raw.mastery, id),
      lastSeenAt: typeof raw.lastSeenAt === "number" ? raw.lastSeenAt : null,
      avgLatencyMs:
        typeof raw.avgLatencyMs === "number" ? raw.avgLatencyMs : null,
      // V4 起携带;旧备份缺省 → null(不伪造历史表现)。
      accuracyEwma:
        typeof raw.accuracyEwma === "number" &&
        Number.isFinite(raw.accuracyEwma) &&
        raw.accuracyEwma >= 0 &&
        raw.accuracyEwma <= 1
          ? raw.accuracyEwma
          : null,
      review,
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

/** 章节学习证据校验:字段可选;存在时逐章节校验(缺失/null 字段回默认)。 */
function validateLessonEvidence(value: unknown): Record<string, LessonEvidence> {
  if (value === undefined || value === null) return {};
  if (!isRecord(value)) fail("lessonEvidence 结构无效");
  const lessonEvidence: Record<string, LessonEvidence> = {};
  for (const [chapterId, raw] of Object.entries(value)) {
    if (!isRecord(raw)) fail(`lessonEvidence[${chapterId}] 结构无效`);
    const evidence: LessonEvidence = {
      openedAt: typeof raw.openedAt === "number" ? raw.openedAt : null,
      practiceSessions: nonNegativeInt(raw.practiceSessions ?? 0, `lessonEvidence[${chapterId}].practiceSessions`),
      practiceAttempts: nonNegativeInt(raw.practiceAttempts ?? 0, `lessonEvidence[${chapterId}].practiceAttempts`),
      lastPracticeAt: typeof raw.lastPracticeAt === "number" ? raw.lastPracticeAt : null,
      bestAccuracy:
        typeof raw.bestAccuracy === "number" && Number.isFinite(raw.bestAccuracy)
          ? raw.bestAccuracy
          : null,
    };
    if (
      evidence.bestAccuracy !== null &&
      (evidence.bestAccuracy < 0 || evidence.bestAccuracy > 1)
    ) {
      fail(`lessonEvidence[${chapterId}].bestAccuracy 应在 0..1`);
    }
    lessonEvidence[chapterId] = evidence;
  }
  return lessonEvidence;
}

function nonNegativeInt(value: unknown, at: string): number {
  if (typeof value !== "number" || !Number.isInteger(value) || value < 0) {
    fail(`${at} 应为非负整数`);
  }
  return value;
}

/**
 * 键位混淆校验(可选增量字段):缺失/null → {};存在时逐条校验,
 * 损坏条目单独丢弃(与 lessonEvidence 的宽松策略一致),聚合表走
 * sanitizeConfusionMap 的同一校验边界(上限、键格式、码位)。
 */
function validateConfusions(value: unknown): ConfusionMap {
  if (value === undefined || value === null) return {};
  return sanitizeConfusionMap(value);
}

/**
 * 校验并导入备份;返回可直接并入 store 的数据。
 * 接受 V2/V3;V2 缺失的复习证据保持 null,不把历史 mastery 当独立回忆。
 * lessonEvidence 与 confusions 仍按各自的字段校验策略处理。
 * 超过 {@link MAX_BACKUP_JSON_CHARS} 的输入直接拒绝(可操作的错误提示)。
 */
export function importBackup(json: string): {
  settings: BackupSettings;
  progress: Record<string, ItemProgress>;
  daily: Record<string, DailyStats>;
  keyErrors: Record<string, number>;
  lessonEvidence: Record<string, LessonEvidence>;
  confusions: ConfusionMap;
  sourceVersion: number;
} {
  if (json.length > MAX_BACKUP_JSON_CHARS) {
    fail(
      `备份文件过大(超过 ${Math.round(MAX_BACKUP_JSON_CHARS / 1024 / 1024)} MB 上限),请确认选择的是训练器导出的 JSON 备份`,
    );
  }
  let parsed: unknown;
  try {
    parsed = JSON.parse(json);
  } catch (cause) {
    fail(`备份不是合法 JSON:${String(cause)}`);
  }
  if (!isRecord(parsed)) fail("备份结构无效");
  if (parsed.kind !== BACKUP_KIND) fail("这不是训练器备份文件");
  if (parsed.version !== 2 && parsed.version !== BACKUP_VERSION) {
    fail(`备份版本应为 2 或 ${BACKUP_VERSION}(实际 ${String(parsed.version)})`);
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
    sourceVersion: parsed.version,
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
    lessonEvidence: validateLessonEvidence(parsed.lessonEvidence),
    confusions: validateConfusions(parsed.confusions),
  };
}
