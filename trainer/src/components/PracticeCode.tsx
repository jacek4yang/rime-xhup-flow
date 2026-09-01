import { motion } from "motion/react";
import { cn } from "@/lib/utils";
import type { QuestionOutcome } from "@/features/practice/types";

const SLOT_LABELS = ["音", "音", "形", "形"] as const;

/**
 * 编码槽位:2 码 [音][音],3 码 [音][音][形],4 码全码。
 * 已接受的键逐格填入;当前格高亮;错键时当前格抖动提示(不填入错键)。
 */
export function PracticeCode({
  code,
  typed,
  lastWrongKey,
  outcome,
}: {
  code: string;
  typed: string;
  lastWrongKey: string | null;
  outcome: QuestionOutcome | null;
}) {
  return (
    <div
      className="flex items-start justify-center gap-2 sm:gap-3"
      aria-label={`编码 ${code.length} 键,已输入 ${typed.length} 键`}
    >
      {[...code].map((_, index) => {
        const filled = index < typed.length;
        const isCurrent = index === typed.length && outcome === null;
        const showError = isCurrent && lastWrongKey !== null;
        return (
          <div key={index} className="flex flex-col items-center gap-1.5">
            <motion.div
              animate={
                showError
                  ? { x: [0, -6, 6, -4, 4, 0] }
                  : outcome === "perfect"
                    ? { scale: [1, 1.06, 1] }
                    : { x: 0, scale: 1 }
              }
              transition={{ duration: 0.2 }}
              className={cn(
                "flex size-12 items-center justify-center rounded-lg border-2 font-mono text-xl font-semibold transition-colors sm:size-14 sm:text-2xl",
                filled
                  ? outcome === null
                    ? "border-primary/60 bg-primary/10 text-primary"
                    : outcome === "perfect"
                      ? "border-success/60 bg-success/10 text-success"
                      : "border-warning/60 bg-warning/10 text-warning"
                  : isCurrent
                    ? "border-primary bg-background text-foreground"
                    : "border-border bg-muted/50 text-muted-foreground",
              )}
            >
              {filled ? typed[index] : ""}
            </motion.div>
            <span className="text-xs text-muted-foreground">
              {SLOT_LABELS[index]}
            </span>
          </div>
        );
      })}
    </div>
  );
}
