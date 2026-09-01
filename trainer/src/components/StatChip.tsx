import type { ReactNode } from "react";
import { cn } from "@/lib/utils";

/** 小型统计数字块:标签 + 数值,用于今日统计与练习实况。 */
export function StatChip({
  label,
  value,
  className,
}: {
  label: ReactNode;
  value: ReactNode;
  className?: string;
}) {
  return (
    <div
      className={cn(
        "flex min-w-0 flex-col gap-0.5 rounded-lg border border-border bg-card px-3 py-2",
        className,
      )}
    >
      <span className="truncate text-xs text-muted-foreground">{label}</span>
      <span className="text-base font-semibold tabular-nums">{value}</span>
    </div>
  );
}
