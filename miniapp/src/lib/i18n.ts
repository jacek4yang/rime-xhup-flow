/**
 * 页面 i18n 钩子:语言偏好来自应用状态,翻译表复用共享核心。
 * 课程正文为规范中文(canonical),不参与翻译;本钩子只覆盖界面文案。
 *
 * 小程序本地增量字典:统计页与备份导入页的新增文案不进共享核心
 * i18n 表(共享表由多端共用,另行统一迁移),在此以 zh 为基准、
 * en 强制键对齐的方式维护;`t` 先查本地增量表,再回落核心表。
 */

import { useMemo } from "react";
import { translate as coreTranslate } from "@xhup/trainer-core";
import type { I18nKey, Language } from "@xhup/trainer-core";
import { useAppState } from "./store";

/** 基准增量字典(zh):仅小程序使用的界面文案。 */
const zhExtra = {
  // 统计页(stats.*)
  "stats.today": "今日概览",
  "stats.keystrokes": "按键数",
  "stats.corrections": "退格修正",
  "stats.notStarted": "未开始",
  "stats.topErrors": "最常按错的键(前 5)",
  "stats.noErrors": "暂无键位错误记录。",
  "stats.entryHint": "今日概览、近 14 天趋势与掌握度分布",
  "stats.trendHint": "条形高度与当日练习时长等比;无练习日显示占位细条。",
  // 备份导入页(import.*)
  "import.title": "导入备份",
  "import.fileHint":
    "推荐方式:在电脑端导出备份 JSON,用微信发送到「文件传输助手」,再点下方按钮选择该文件。",
  "import.pickFile": "从聊天记录选择文件",
  "import.pasteLabel": "或手动粘贴备份 JSON",
  "import.pastePlaceholder": "把训练器导出的备份 JSON 粘贴到这里",
  "import.clipboardNote":
    "注意:手机与电脑的剪贴板通常不同步,跨设备导入请优先使用「从聊天记录选择文件」。",
  "import.parse": "解析并预览",
  "import.previewVersion": "备份版本",
  "import.previewProgress": "进度条目",
  "import.previewDays": "按日统计天数",
  "import.previewRange": "日期范围",
  "import.previewKeyErrors": "键位错误键数",
  "import.confirmTitle": "确认导入备份?",
  "import.confirmBody":
    "将整体替换当前进度、按日统计、键位错误与混淆聚合;此操作不可撤销。",
  "import.apply": "导入并替换",
  "import.applied": "备份已导入",
  "import.errorTooLarge":
    "备份文件过大(超过 2 MB 上限),请确认选择的是训练器导出的 JSON 备份",
  "import.errorInvalid": "备份无效:{reason}",
  "import.readFailed": "读取文件失败:{reason}",
} as const;

/** 增量键集合(zh 为基准)。 */
export type UiExtraKey = keyof typeof zhExtra;

/** 英文增量字典:键必须与 zh 完全一致(类型约束 + 测试兜底)。 */
const enExtra: Record<UiExtraKey, string> = {
  "stats.today": "Today at a glance",
  "stats.keystrokes": "Keystrokes",
  "stats.corrections": "Corrections",
  "stats.notStarted": "Not started",
  "stats.topErrors": "Most mistyped keys (top 5)",
  "stats.noErrors": "No key mistakes recorded yet.",
  "stats.entryHint": "Today, 14-day trend and mastery distribution",
  "stats.trendHint":
    "Bar heights scale with that day's practice time; days without practice show a placeholder sliver.",
  "import.title": "Import backup",
  "import.fileHint":
    "Recommended: export the backup JSON on desktop, send it to yourself in a WeChat chat (e.g. File Transfer), then pick that file below.",
  "import.pickFile": "Pick file from a chat",
  "import.pasteLabel": "Or paste the backup JSON",
  "import.pastePlaceholder": "Paste the trainer's backup JSON here",
  "import.clipboardNote":
    "Note: phone and desktop clipboards usually do not sync — prefer picking a chat file for cross-device import.",
  "import.parse": "Parse and preview",
  "import.previewVersion": "Backup version",
  "import.previewProgress": "Progress entries",
  "import.previewDays": "Days with stats",
  "import.previewRange": "Date range",
  "import.previewKeyErrors": "Keys with errors",
  "import.confirmTitle": "Import this backup?",
  "import.confirmBody":
    "This replaces your current progress, daily stats, key errors and confusions. It cannot be undone.",
  "import.apply": "Import and replace",
  "import.applied": "Backup imported",
  "import.errorTooLarge":
    "Backup is too large (over the 2 MB limit) — make sure you picked the trainer's JSON backup",
  "import.errorInvalid": "Invalid backup: {reason}",
  "import.readFailed": "Failed to read file: {reason}",
};

/** 页面可用键全集 = 核心键 + 本地增量键。 */
export type UiKey = I18nKey | UiExtraKey;

const EXTRA_DICTIONARIES: Record<Language, Record<UiExtraKey, string>> = {
  zh: zhExtra,
  en: enExtra,
};

/** 插值:{n} 占位,与核心 translate 同一约定。 */
function interpolate(
  template: string,
  params: Record<string, string | number>,
): string {
  return Object.entries(params).reduce(
    (text, [name, value]) => text.split(`{${name}}`).join(String(value)),
    template,
  );
}

/**
 * 翻译(纯函数):先查本地增量表,再回落核心表。
 */
export function translateUi(
  language: Language,
  key: UiKey,
  params?: Record<string, string | number>,
): string {
  if ((key as string) in zhExtra) {
    return interpolate(EXTRA_DICTIONARIES[language][key as UiExtraKey], params ?? {});
  }
  return coreTranslate(language, key as I18nKey, params);
}

/** 测试辅助:导出增量字典键集合(校验 zh/en 对齐)。 */
export function uiDictionaryKeys(): { zh: Set<string>; en: Set<string> } {
  return {
    zh: new Set(Object.keys(zhExtra)),
    en: new Set(Object.keys(enExtra)),
  };
}

export function useI18n(): {
  t: (key: UiKey, params?: Record<string, string | number>) => string;
  language: "zh" | "en";
} {
  const language = useAppState().settings.language;
  return useMemo(
    () => ({
      t: (key, params) => translateUi(language, key, params),
      language,
    }),
    [language],
  );
}
