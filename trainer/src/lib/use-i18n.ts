/**
 * i18n hook:从 store 读取语言偏好,返回翻译函数(语言切换时重渲染)。
 */

import { useTrainerStore } from "@/stores/trainer-store";
import { translate, type I18nKey, type Language } from "@/lib/i18n";

export function useI18n(): {
  t: (key: I18nKey, params?: Record<string, string | number>) => string;
  language: Language;
} {
  const language = useTrainerStore((state) => state.language);
  return {
    language,
    t: (key, params) => translate(language, key, params),
  };
}
