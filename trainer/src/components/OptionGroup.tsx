import type { ReactNode } from "react";
import { cn } from "@/lib/utils";

/** 设置项行:左侧标签 + 右侧选项组(响应式堆叠)。 */
export function OptionRow({
  label,
  children,
}: {
  label: string;
  children: ReactNode;
}) {
  return (
    <div className="flex flex-col gap-2 sm:flex-row sm:items-center sm:gap-4">
      <span className="w-24 shrink-0 text-sm text-muted-foreground">{label}</span>
      <div className="flex flex-wrap gap-2">{children}</div>
    </div>
  );
}

export function OptionButton({
  selected,
  onClick,
  children,
}: {
  selected: boolean;
  onClick: () => void;
  children: ReactNode;
}) {
  return (
    <button
      type="button"
      aria-pressed={selected}
      onClick={onClick}
      className={cn(
        "min-h-11 rounded-md border px-4 text-sm font-medium transition-colors focus-visible:outline-2 focus-visible:outline-ring",
        selected
          ? "border-primary bg-primary/10 text-primary"
          : "border-border bg-background text-muted-foreground hover:bg-accent",
      )}
    >
      {children}
    </button>
  );
}
