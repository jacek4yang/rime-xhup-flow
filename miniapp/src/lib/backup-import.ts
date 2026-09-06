/**
 * 备份导入流程(纯逻辑,存储与 UI 无关):解析 → 预览 → 确认 → 应用。
 *
 * 校验复用共享核心 importBackup(整份备份的版本 / 结构 / 字段校验,
 * 超过 MAX_BACKUP_JSON_CHARS 的输入在解析前直接拒绝);应用语义与
 * app-store 的 applyBackup 一致:整体替换 progress / daily / keyErrors
 * / confusions,偏好保留(备份里的 settings 字段在小程序端不生效:
 * 语言等偏好属设备本地,桌面独有字段如 theme 不适用于小程序)。
 * 备份中的 lessonEvidence 同样不落小程序状态(小程序 Schema 无该字段)。
 */

import {
  MAX_BACKUP_JSON_CHARS,
  importBackup,
} from "@xhup/trainer-core";
import type {
  ConfusionMap,
  DailyStats,
  ItemProgress,
} from "@xhup/trainer-core";

/** 应用到 store 的已校验备份数据(与 applyBackup 载荷一致)。 */
export type ParsedBackup = {
  progress: Record<string, ItemProgress>;
  daily: Record<string, DailyStats>;
  keyErrors: Record<string, number>;
  confusions: ConfusionMap;
};

/** 导入预览摘要:供确认对话框展示,绝不携带可执行内容。 */
export type ImportPreview = {
  version: number;
  progressCount: number;
  dailyDays: number;
  /** daily 日期键的字典序范围 [min, max];无按日数据时为 null。 */
  dateRange: [string, string] | null;
  keyErrorKeys: number;
};

export type ImportParseResult =
  | { ok: true; data: ParsedBackup; preview: ImportPreview }
  | { ok: false; kind: "tooLarge" | "invalid"; reason: string };

/**
 * 解析备份 JSON 文本并产出预览。
 * 不做任何副作用:成功返回可应用数据 + 预览摘要;失败返回可操作的
 * 类型化原因(tooLarge / invalid),调用方状态保持原样。
 */
export function parseBackupForImport(json: string): ImportParseResult {
  // 体积上限先于解析:超大输入几乎必然是选错文件,拒绝要可操作。
  if (json.length > MAX_BACKUP_JSON_CHARS) {
    return { ok: false, kind: "tooLarge", reason: "MAX_BACKUP_JSON_CHARS" };
  }
  try {
    const parsed = importBackup(json);
    const dates = Object.keys(parsed.daily).sort();
    return {
      ok: true,
      data: {
        progress: parsed.progress,
        daily: parsed.daily,
        keyErrors: parsed.keyErrors,
        confusions: parsed.confusions,
      },
      preview: {
        version: 2,
        progressCount: Object.keys(parsed.progress).length,
        dailyDays: dates.length,
        dateRange:
          dates.length > 0 ? [dates[0], dates[dates.length - 1]] : null,
        keyErrorKeys: Object.keys(parsed.keyErrors).length,
      },
    };
  } catch (cause) {
    return {
      ok: false,
      kind: "invalid",
      reason: cause instanceof Error ? cause.message : String(cause),
    };
  }
}
