/**
 * 页面 i18n 钩子:语言偏好来自应用状态,翻译表复用共享核心。
 * 课程正文为规范中文(canonical),不参与翻译;本钩子只覆盖界面文案。
 *
 * 小程序本地增量键(统计页 / 备份导入页新增文案)在 ui-i18n.ts 维护;
 * 本钩子的 `t` 经 translateUi 先查增量表再回落核心表。
 */

import { useMemo } from "react";
import { translateUi } from "./ui-i18n";
import type { UiKey } from "./ui-i18n";
import { useAppState } from "./store";

export type { UiKey };

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
