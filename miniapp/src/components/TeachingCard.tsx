/**
 * 错误教学卡片(Taro 版 ErrorTeachingCard):答错后暂停讲解,展示
 * 读音(含声调)、码结构与出错位置,然后重试同一题。
 *
 * 只展示可验证事实:带调读音来自数据分片(Unihan 教育子集,可缺失,
 * 如实回退);形键分布来自规范全码数据;绝不编造字根释义或声调。
 */

import { View, Text } from "@tarojs/components";
import type {
  ErrorTeachingMode,
  I18nKey,
  ShapeKeyStat,
  TrainerEntry,
  TrainingItem,
} from "@xhup/trainer-core";
import { useI18n } from "../lib/i18n";

/** 音码键位标签(单字条目;复用码位格的既有槽位文案)。 */
const CODE_PART_KEYS: readonly [I18nKey, I18nKey, I18nKey, I18nKey] = [
  "practice.slotInitial",
  "practice.slotInitial",
  "practice.slotShape",
  "practice.slotShape",
];

export function TeachingCard({
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
  const slotLabel =
    isCharSlot && position < CODE_PART_KEYS.length
      ? t(CODE_PART_KEYS[position])
      : t("practice.teachingSlotN", { n: position + 1 });

  // 同首形高频例字(规范数据聚合;detailed 模式展示)。
  const sameShapeSamples =
    mode === "detailed" && isCharSlot && position === 2
      ? (shapeStats.find((stat) => stat.key === expected)?.firstSamples ?? [])
      : [];

  return (
    <View className="teaching-overlay">
      <Text className="teaching-title">{t("practice.teachingTitle")}</Text>

      <View className="teaching-head">
        <Text className="teaching-target">{item.target}</Text>
        {entry?.toneReading ? (
          <Text className="teaching-tone">{entry.toneReading}</Text>
        ) : item.readings.length > 0 ? (
          <Text className="teaching-reading">{item.readings.join(" / ")}</Text>
        ) : null}
      </View>
      {entry && entry.readings.length > 1 && (
        <Text className="teaching-note">
          {t("practice.teachingOtherReadings", {
            readings: entry.readings.join(" / "),
          })}
        </Text>
      )}

      <View className="teaching-code">
        <View className="teaching-code-row">
          {[...item.primaryCode].map((char, index) => (
            <Text
              key={index}
              className={`teaching-code-char ${index === position ? "teaching-code-wrong" : ""}`}
            >
              {char.toUpperCase()}
            </Text>
          ))}
        </View>
        <Text className="teaching-note">
          {slotLabel}:{t("practice.teachingDivergence", {
            expected: expected.toUpperCase(),
            actual: wrongKey.toUpperCase(),
          })}
        </Text>
      </View>

      {mode === "detailed" && isCharSlot && position >= 2 && (
        <Text className="teaching-note">
          {t("practice.teachingShapeHint", {
            first: item.primaryCode[2]?.toUpperCase() ?? "—",
            second: item.primaryCode[3]?.toUpperCase() ?? "—",
          })}
        </Text>
      )}
      {sameShapeSamples.length > 0 && (
        <Text className="teaching-note">
          {t("practice.teachingSameShape", {
            chars: sameShapeSamples.map((sample) => sample.char).join(" "),
          })}
        </Text>
      )}

      <View className="button teaching-retry" onClick={onRetry}>
        <Text>{t("practice.teachingRetry")}</Text>
      </View>
    </View>
  );
}

/** 复用入口:按练习条目查询规范数据。 */
export function findEntry(
  entries: readonly TrainerEntry[],
  target: string,
): TrainerEntry | null {
  return entries.find((entry) => entry.char === target) ?? null;
}
