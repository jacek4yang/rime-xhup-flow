import { useMemo } from "react";
import { cn } from "@/lib/utils";
import { useI18n } from "@/lib/use-i18n";
import type { DoublePinyinReference } from "@/lib/trainer-data";

const KEY_ROWS = ["qwertyuiop", "asdfghjkl", "zxcvbnm"] as const;

/** 经典 QWERTY 错位排布:中间行与底行向内缩进。 */
const ROW_INDENTS = ["px-0", "px-[4.5%]", "px-[9%]"] as const;

/** 键帽参考内容模式(情境模式由调用方按练习模式解析后再传入)。 */
export type KeyboardRefMode = "none" | "double" | "shape" | "both";

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

/** 形码参考:键 → {首形代表字, 次形代表字}(调用方从规范数据聚合)。 */
export type ShapeKeyRef = Map<string, { first: string; second: string }>;

/**
 * 键帽缩写:韵母多于一个时只显示第一个 + "+",完整对照在键详情面板。
 * 规则化缩写,绝不使用省略号截断(教育内容不可不完整)。
 */
export function compactFinals(finals: readonly string[]): string {
  if (finals.length === 0) return "";
  return finals.length > 1 ? `${finals[0]}+` : finals[0];
}

export function compactInitials(initials: readonly string[]): string {
  return initials.join(" ");
}

/**
 * QWERTY 屏显键盘:大键帽(上参考/字母/下参考)、三行错位、无省略号。
 *
 * - 练习模式传入 onKeyPress(点按 = 输入);
 * - 参考模式传入 onKeyInfo(点按 = 打开键知识面板,如键位页)。
 * - 「下一期待键」高亮由调用方按提示策略门控,本组件不自行判断。
 */
export function OnScreenKeyboard({
  reference,
  nextKey = null,
  wrongKey = null,
  onKeyPress,
  onKeyInfo,
  refMode = "none",
  shapeRef,
  compact = false,
}: {
  reference: DoublePinyinReference;
  nextKey?: string | null;
  wrongKey?: string | null;
  onKeyPress?: (key: string) => void;
  onKeyInfo?: (key: string) => void;
  refMode?: KeyboardRefMode;
  shapeRef?: ShapeKeyRef;
  compact?: boolean;
}) {
  const { t } = useI18n();
  const labels = useMemo(() => buildKeyLabels(reference), [reference]);
  const interactive = Boolean(onKeyPress ?? onKeyInfo);

  return (
    <div className="flex flex-col items-center gap-1.5 sm:gap-2" aria-label={t("common.keyboard")}>
      {KEY_ROWS.map((row, rowIndex) => (
        <div
          key={row}
          className={cn("flex w-full justify-center gap-1 sm:gap-1.5", ROW_INDENTS[rowIndex])}
        >
          {[...row].map((key) => {
            const label = labels.get(key);
            const shape = shapeRef?.get(key);
            const showDouble = refMode === "double" || refMode === "both";
            const showShape = refMode === "shape" || refMode === "both";
            const topLeft = showDouble ? compactInitials(label?.initials ?? []) : "";
            const bottomLeft = showDouble ? compactFinals(label?.finals ?? []) : "";
            const topRight = showShape ? (shape?.first ?? "") : "";
            const bottomRight = showShape ? (shape?.second ?? "") : "";
            const isNext = nextKey === key;
            const isWrong = wrongKey === key;
            const className = cn(
              // 固定几何:字号与高度不随内容变化,杜绝键宽漂移与截断。
              "relative flex min-h-11 min-w-0 max-w-12 flex-1 select-none items-center justify-center rounded-lg border font-mono shadow-sm transition-all sm:max-w-14",
              compact ? "h-11 sm:h-12" : "h-14 sm:h-16",
              isNext
                ? "border-primary bg-primary/15 text-primary ring-2 ring-primary/40"
                : isWrong
                  ? "border-destructive bg-destructive/15 text-destructive"
                  : "border-border bg-card text-foreground",
              interactive &&
                "cursor-pointer hover:bg-accent active:scale-[0.96] active:bg-accent/80 focus-visible:outline-2 focus-visible:outline-ring",
            );
            const content = (
              <>
                {topLeft && (
                  <span className="absolute left-1 top-0.5 max-w-[calc(100%-0.75rem)] text-left font-sans text-[9px] leading-none text-muted-foreground">
                    {topLeft}
                  </span>
                )}
                {topRight && (
                  <span className="absolute right-1 top-0.5 font-sans text-[9px] leading-none text-muted-foreground">
                    {topRight}
                  </span>
                )}
                <span
                  className={cn(
                    "font-semibold uppercase leading-none",
                    compact ? "text-base" : refMode === "none" ? "text-xl" : "text-lg",
                  )}
                >
                  {key}
                </span>
                {bottomLeft && (
                  <span className="absolute bottom-0.5 left-1 max-w-[calc(100%-1.5rem)] text-left font-sans text-[9px] leading-tight text-muted-foreground">
                    {bottomLeft}
                  </span>
                )}
                {bottomRight && (
                  <span className="absolute bottom-0.5 right-1 font-sans text-[9px] leading-none text-muted-foreground">
                    {bottomRight}
                  </span>
                )}
              </>
            );
            const ariaLabel = [
              t("practice.keyAriaPlain", { key }),
              topLeft && `${t("reference.initial")} ${topLeft}`,
              bottomLeft && `${t("reference.final")} ${bottomLeft}`,
              (topRight || bottomRight) &&
                t("learn.shapeRefLabel", { chars: [topRight, bottomRight].filter(Boolean).join("/") }),
            ]
              .filter(Boolean)
              .join(", ");
            const handleClick = onKeyPress ?? onKeyInfo;
            if (!handleClick) {
              return (
                <div key={key} className={className} aria-label={ariaLabel}>
                  {content}
                </div>
              );
            }
            return (
              <button
                key={key}
                type="button"
                tabIndex={-1}
                // 下一期待键标记:供测试观测(提示策略门控后才会出现)。
                data-next={isNext ? "true" : undefined}
                data-key={key}
                aria-label={ariaLabel}
                className={className}
                onClick={() => handleClick(key)}
              >
                {content}
              </button>
            );
          })}
        </div>
      ))}
    </div>
  );
}
