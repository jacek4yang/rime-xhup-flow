/**
 * 页面 i18n 钩子:语言偏好来自应用状态,翻译表复用共享核心。
 * 课程正文为规范中文(canonical),不参与翻译;本钩子只覆盖界面文案。
 */

import { useMemo } from "react";
import { translate } from "@xhup/trainer-core";
import type { I18nKey } from "@xhup/trainer-core";
import { useAppState } from "./store";

export function useI18n(): {
  t: (key: I18nKey, params?: Record<string, string | number>) => string;
  language: "zh" | "en";
} {
  const language = useAppState().settings.language;
  return useMemo(
    () => ({
      t: (key, params) => translate(language, key, params),
      language,
    }),
    [language],
  );
}
