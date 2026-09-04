/**
 * 错误教学卡片:答错后暂停讲解,展示读音(含声调)、码结构与出错位置,
 * 然后重试同一题。
 *
 * 只展示可验证事实:带调读音来自 Unihan 教育子集(可缺失,如实回退);
 * 形键分布来自规范全码数据;绝不编造字根释义或声调。
 */

import { Play } from "lucide-react";
import { Button } from "@/components/ui/button";
import type { I18nKey } from "@/lib/i18n";
import { useI18n } from "@/lib/use-i18n";
import type { TrainerEntry } from "@/lib/trainer-data";
import type { TrainingItem } from "@/lib/trainer-index";
import type { ShapeKeyStat } from "@/features/learn/shape-explorer";
import type { ErrorTeachingMode } from "./types";

/** 音码键位标签(单字条目;复用码位格的既有槽位文案)。 */
const CODE_PART_KEYS: readonly I18nKey[] = [
  "practice.slotInitial",
  "practice.slotInitial",
  "practice.slotShape",
  "practice.slotShape",
];

export function ErrorTeachingCard({
  item,
  entry,
  wrongKey,
  position,
  mode,
  shapeStats,
  onRetry,
}: {
  item: TrainingItem;
  /** 单字条目的规范数据(词/句为 null)。 */
  entry: TrainerEntry | null;
  wrongKey: string;
  position: number;
  mode: ErrorTeachingMode;
  shapeStats: ShapeKeyStat[];
  onRetry: () => void;
}) {
  const { t } = useI18n();
  const expected = item.primaryCode[position];
  const isCharSlot = item.kind === "char" || item.kind === "level1";
  const slotLabel = isCharSlot && position < CODE_PART_KEYS.length
    ? t(CODE_PART_KEYS[position])
    : t("practice.teachingSlotN", { n: position + 1 });

  // 同首形高频例字(规范数据聚合;detailed 模式展示)。
  const sameShapeSamples =
    mode === "detailed" && isCharSlot && position === 2
      ? (shapeStats.find((stat) => stat.key === expected)?.firstSamples ?? [])
      : [];

  return (
    <div
      className="absolute inset-0 z-20 flex flex-col items-center justify-center gap-3 overflow-auto rounded-xl bg-card/97 p-5 backdrop-blur-sm"
      role="alertdialog"
      aria-label={t("practice.teachingTitle")}
    >
      <p className="text-sm font-medium text-destructive">{t("practice.teachingTitle")}</p>

      <div className="flex items-baseline gap-3">
        <span className="text-3xl font-semibold">{item.target}</span>
        {entry?.toneReading ? (
          <span className="font-mono text-lg text-primary">{entry.toneReading}</span>
        ) : item.readings.length > 0 ? (
          <span className="font-mono text-base text-muted-foreground">
            {item.readings.join(" / ")}
          </span>
        ) : null}
      </div>
      {entry && entry.readings.length > 1 && (
        <p className="text-xs text-muted-foreground">
          {t("practice.teachingOtherReadings", {
            readings: entry.readings.join(" / "),
          })}
        </p>
      )}

      <div className="flex flex-col items-center gap-1 rounded-lg border border-border px-4 py-3">
        <div className="flex gap-1 font-mono text-lg">
          {[...item.primaryCode].map((char, index) => (
            <span
              key={index}
              className={cn_slot(index === position)}
            >
              {char.toUpperCase()}
            </span>
          ))}
        </div>
        <p className="text-xs text-muted-foreground">
          {slotLabel}:{t("practice.teachingDivergence", { expected: expected.toUpperCase(), actual: wrongKey.toUpperCase() })}
        </p>
      </div>

      {mode === "detailed" && isCharSlot && position >= 2 && (
        <p className="max-w-xs text-center text-xs text-muted-foreground">
          {t("practice.teachingShapeHint", {
            first: item.primaryCode[2]?.toUpperCase() ?? "—",
            second: item.primaryCode[3]?.toUpperCase() ?? "—",
          })}
        </p>
      )}
      {sameShapeSamples.length > 0 && (
        <p className="max-w-xs text-center font-mono text-xs text-muted-foreground">
          {t("practice.teachingSameShape", {
            chars: sameShapeSamples.map((sample) => sample.char).join(" "),
          })}
        </p>
      )}

      <Button className="min-h-11" onClick={onRetry}>
        <Play aria-hidden />
        {t("practice.teachingRetry")}
      </Button>
    </div>
  );
}

function cn_slot(isWrongSlot: boolean): string {
  return [
    "flex size-9 items-center justify-center rounded border",
    isWrongSlot
      ? "border-destructive bg-destructive/10 text-destructive"
      : "border-border bg-muted/40 text-foreground",
  ].join(" ");
}

/** 复用入口:按练习条目查询规范数据(PracticeView 已建索引)。 */
export function findEntry(
  entries: readonly TrainerEntry[],
  target: string,
): TrainerEntry | null {
  return entries.find((entry) => entry.char === target) ?? null;
}
