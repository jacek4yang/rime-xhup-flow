/**
 * 键位参考页:QWERTY 教学键盘完整视图 + 零声母规则表。
 * 双拼映射全部来自规范数据(doublePinyin);点任意键进入键详情。
 */

import { useMemo } from "react";
import { View, Text } from "@tarojs/components";
import { dataset } from "../../lib/dataset";
import { buildKeyLabels } from "../../lib/keyboard-labels";
import { useI18n } from "../../lib/i18n";
import { ROUTES, navigateTo } from "../../lib/navigation";
import { TeachingKeyboard } from "../../components/TeachingKeyboard";
import type { ShapeKeyRef } from "../../components/TeachingKeyboard";
import { buildShapeKeyStats } from "@xhup/trainer-core";

export default function KeyboardReference() {
  const { t } = useI18n();
  const labels = useMemo(() => buildKeyLabels(dataset.doublePinyin), []);

  // 形码代表字:仅当分片有全码数据时出现(没有就如实不显示)。
  const shapeRef = useMemo<ShapeKeyRef>(() => {
    const map: ShapeKeyRef = new Map();
    for (const stat of buildShapeKeyStats(dataset.entries)) {
      const first = stat.firstSamples[0]?.char;
      const second = stat.secondSamples[0]?.char;
      if (first ?? second) {
        map.set(stat.key, { first: first ?? "", second: second ?? "" });
      }
    }
    return map;
  }, []);

  return (
    <View className="page">
      <View className="card">
        <Text className="title">{t("nav.reference")}</Text>
        <Text className="subtitle">{t("reference.subtitle")}</Text>
        <Text className="subtitle">{t("reference.keyboardTapHint")}</Text>
      </View>

      <View className="card">
        <TeachingKeyboard
          labels={labels}
          shapeRef={shapeRef}
          refMode={shapeRef.size > 0 ? "both" : "double"}
          onKeyInfo={(key) => navigateTo(ROUTES.keyDetail(key))}
        />
      </View>

      <View className="card">
        <Text className="card-heading">{t("reference.zeroInitials")}</Text>
        <Text className="subtitle">{t("reference.zeroInitialsHint")}</Text>
        {dataset.doublePinyin.zeroInitials.map((mapping) => (
          <View className="zero-row" key={mapping.syllable}>
            <Text className="zero-syllable">{mapping.syllable}</Text>
            <Text className="zero-code">{mapping.code}</Text>
          </View>
        ))}
      </View>

      <View className="card">
        <Text className="card-heading">{t("reference.codeStructure")}</Text>
        <Text className="body-text paragraph">{t("reference.code2")}{t("reference.code2Desc")}</Text>
        <Text className="body-text paragraph">{t("reference.code3")}{t("reference.code3Desc")}</Text>
        <Text className="body-text paragraph">{t("reference.code4")}{t("reference.code4Desc")}</Text>
      </View>
    </View>
  );
}
