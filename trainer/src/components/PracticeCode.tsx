import { motion } from "motion/react";
import { cn } from "@/lib/utils";
import { useI18n } from "@/lib/use-i18n";
import type { I18nKey } from "@xhup/trainer-core";
import type { QuestionOutcome } from "@xhup/trainer-core";

const SLOT_LABELS = [
  "practice.slotInitial",
  "practice.slotInitial",
  "practice.slotShape",
  "practice.slotShape",
] as const satisfies readonly I18nKey[];

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
  const { t } = useI18n();
  // 句子长码可达 40 键:缩小槽位并允许换行,任何码长都完整可见
  // (与小程序端 CodeSlots 同一套降级语义:收缩/换行,绝不截断)。
  const compact = code.length > 8;
  return (
    <div
      className="flex flex-wrap items-start justify-center gap-2 sm:gap-3"
      aria-label={t("practice.codeSlotsAria", {
        total: code.length,
        typed: typed.length,
      })}
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
                "flex items-center justify-center rounded-lg border-2 font-mono font-semibold transition-colors",
                compact
                  ? "size-8 text-base sm:size-10 sm:text-lg"
                  : "size-12 text-xl sm:size-14 sm:text-2xl",
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
              {index < SLOT_LABELS.length
                ? t(SLOT_LABELS[index])
                : t("practice.teachingSlotN", { n: index + 1 })}
            </span>
          </div>
        );
      })}
    </div>
  );
}
