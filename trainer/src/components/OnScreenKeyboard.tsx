import { useMemo } from "react";
import { cn } from "@/lib/utils";
import { useI18n } from "@/lib/use-i18n";
import type { DoublePinyinReference } from "@/lib/trainer-data";

const KEY_ROWS = ["qwertyuiop", "asdfghjkl", "zxcvbnm"] as const;

/** 经典 QWERTY 错位排布:中间行与底行向内缩进。 */
const ROW_INDENTS = ["px-0", "px-[4.5%]", "px-[9%]"] as const;

type KeyLabels = {
  initials: string[];
  finals: string[];
};

/** 从规范 JSON 的双拼参考构建 键 → {声母, 韵母} 标签;不做任何硬编码映射。 */
export function buildKeyLabels(
  reference: DoublePinyinReference,
): Map<string, KeyLabels> {
  const map = new Map<string, KeyLabels>();
  const ensure = (key: string): KeyLabels => {
    const existing = map.get(key);
    if (existing) return existing;
    const created: KeyLabels = { initials: [], finals: [] };
    map.set(key, created);
    return created;
  };
  for (const { initial, key } of reference.initials) {
    ensure(key).initials.push(initial);
  }
  for (const { final, key } of reference.finals) {
    ensure(key).finals.push(final);
  }
  return map;
}

/**
 * QWERTY 屏幕键盘。练习时高亮下一期待键、闪烁错键,按键可点按输入;
 * 键位参考页不传 onKeyPress,渲染为静态展示。
 */
export function OnScreenKeyboard({
  reference,
  nextKey = null,
  wrongKey = null,
  onKeyPress,
  compact = false,
}: {
  reference: DoublePinyinReference;
  nextKey?: string | null;
  wrongKey?: string | null;
  onKeyPress?: (key: string) => void;
  compact?: boolean;
}) {
  const { t } = useI18n();
  const labels = useMemo(() => buildKeyLabels(reference), [reference]);

  return (
    <div className="flex flex-col items-center gap-1.5 sm:gap-2" aria-label={t("common.keyboard")}>
      {KEY_ROWS.map((row, rowIndex) => (
        <div
          key={row}
          className={cn("flex w-full justify-center gap-1 sm:gap-1.5", ROW_INDENTS[rowIndex])}
        >
          {[...row].map((key) => {
            const label = labels.get(key);
            const hint = [label?.initials.join(" "), label?.finals.join(" ")]
              .filter(Boolean)
              .join(" ");
            const isNext = nextKey === key;
            const isWrong = wrongKey === key;
            const className = cn(
              "flex min-h-11 min-w-0 max-w-12 flex-1 select-none flex-col items-center justify-center rounded-lg border font-mono shadow-sm transition-all sm:max-w-14",
              compact ? "h-11 sm:h-12" : "h-14 sm:h-16",
              isNext
                ? "border-primary bg-primary/15 text-primary ring-2 ring-primary/40"
                : isWrong
                  ? "border-destructive bg-destructive/15 text-destructive"
                  : "border-border bg-card text-foreground",
              onKeyPress &&
                "cursor-pointer hover:bg-accent active:scale-[0.96] active:bg-accent/80 focus-visible:outline-2 focus-visible:outline-ring",
            );
            const content = (
              <>
                <span
                  className={cn(
                    "font-semibold uppercase leading-none",
                    compact ? "text-sm" : "text-base",
                  )}
                >
                  {key}
                </span>
                {hint && (
                  <span
                    className={cn(
                      "mt-0.5 max-w-full truncate px-0.5 font-sans leading-none text-muted-foreground",
                      compact ? "text-[9px]" : "text-[10px] sm:text-xs",
                      isNext && "text-primary/80",
                    )}
                  >
                    {hint}
                  </span>
                )}
              </>
            );
            return onKeyPress ? (
              <button
                key={key}
                type="button"
                tabIndex={-1}
                // 下一期待键标记:供测试与调试观测(不改变视觉语义)。
                data-next={isNext ? "" : undefined}
                aria-label={
                  hint
                    ? t("practice.keyAria", { key, hint })
                    : t("practice.keyAriaPlain", { key })
                }
                className={className}
                onClick={() => onKeyPress(key)}
              >
                {content}
              </button>
            ) : (
              <div key={key} className={className} aria-hidden={!hint}>
                {content}
              </div>
            );
          })}
        </div>
      ))}
    </div>
  );
}
