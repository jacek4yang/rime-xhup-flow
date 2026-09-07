/**
 * 编码槽位(Taro 版 PracticeCode):已接受的键逐格填入,当前格高亮,
 * 错键时当前格标红(不填入错键)。槽位下标注 音/形 参考位置。
 */

import { View, Text } from "@tarojs/components";
import type { I18nKey, QuestionOutcome } from "@xhup/trainer-core";
import { useI18n } from "../lib/i18n";

const SLOT_LABELS: readonly I18nKey[] = [
  "practice.slotInitial",
  "practice.slotInitial",
  "practice.slotShape",
  "practice.slotShape",
];

export function CodeSlots({
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
  // 长码(词/句)自动缩小槽位,保证一行放得下。
  const sizeClass =
    code.length <= 4 ? "slot-lg" : code.length <= 8 ? "slot-md" : "slot-sm";
  return (
    <View className="slots">
      {[...code].map((_, index) => {
        const filled = index < typed.length;
        const isCurrent = index === typed.length && outcome === null;
        const showError = isCurrent && lastWrongKey !== null;
        const stateClass = !filled
          ? isCurrent
            ? showError
              ? "slot-current slot-error"
              : "slot-current"
            : "slot-todo"
          : outcome === null
            ? "slot-filled"
            : outcome === "perfect"
              ? "slot-perfect"
              : "slot-imperfect";
        return (
          <View className="slot-wrap" key={index}>
            <View className={`slot ${sizeClass} ${stateClass}`}>
              <Text className="slot-char">{filled ? typed[index] : ""}</Text>
            </View>
            <Text className="slot-label">
              {index < SLOT_LABELS.length
                ? t(SLOT_LABELS[index])
                : t("practice.teachingSlotN", { n: index + 1 })}
            </Text>
          </View>
        );
      })}
    </View>
  );
}
