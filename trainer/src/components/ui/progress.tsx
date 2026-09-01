import type { HTMLAttributes } from "react";
import { cn } from "@/lib/utils";

export type ProgressProps = HTMLAttributes<HTMLDivElement> & {
  /** 0..1;超出范围会被收敛。 */
  value: number;
};

export function Progress({ className, value, ...props }: ProgressProps) {
  const percent = Math.min(100, Math.max(0, value * 100));
  return (
    <div
      role="progressbar"
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={Math.round(percent)}
      className={cn(
        "h-2 w-full overflow-hidden rounded-full bg-secondary",
        className,
      )}
      {...props}
    >
      <div
        className="h-full rounded-full bg-primary transition-[width] duration-200"
        style={{ width: `${percent}%` }}
      />
    </div>
  );
}
