/**
 * 主题偏好与系统主题解析。
 *
 * 不引入 next-themes:一个小控制器即可。偏好持久化在 store 里,
 * "system" 时监听 prefers-color-scheme 变化并跟随。
 */

import type { I18nKey } from "@/lib/i18n";

export type ThemePreference = "system" | "light" | "dark";
export type ResolvedTheme = "light" | "dark";

export const DEFAULT_THEME: ThemePreference = "system";

export const THEME_LABELS: Record<ThemePreference, I18nKey> = {
  system: "settings.themeSystem",
  light: "settings.themeLight",
  dark: "settings.themeDark",
};

const MEDIA_QUERY = "(prefers-color-scheme: dark)";

export function systemPrefersDark(): boolean {
  return (
    typeof window !== "undefined" &&
    typeof window.matchMedia === "function" &&
    window.matchMedia(MEDIA_QUERY).matches
  );
}

export function resolveTheme(preference: ThemePreference): ResolvedTheme {
  if (preference === "system") return systemPrefersDark() ? "dark" : "light";
  return preference;
}

/** 把解析后的主题应用到根元素(class 策略,与 index.css 的 dark variant 对应)。 */
export function applyThemeToDocument(preference: ThemePreference): void {
  if (typeof document === "undefined") return;
  document.documentElement.classList.toggle(
    "dark",
    resolveTheme(preference) === "dark",
  );
}

/** 监听系统主题变化;返回取消订阅函数。 */
export function onSystemThemeChange(listener: () => void): () => void {
  if (typeof window === "undefined" || typeof window.matchMedia !== "function") {
    return () => {};
  }
  const media = window.matchMedia(MEDIA_QUERY);
  media.addEventListener("change", listener);
  return () => media.removeEventListener("change", listener);
}
