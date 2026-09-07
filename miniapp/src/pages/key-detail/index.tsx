/**
 * 键详情页:单个字母键的全部规范数据映射——双拼声母/韵母、零声母
 * 规则、一级简码、形码首/次形例字(含完整编码)与本机误按次数。
 * 没有真实数据的区块如实省略,绝不编造。
 */

import { useMemo } from "react";
import { View, Text } from "@tarojs/components";
import {
  buildShapeKeyStats,
} from "@xhup/trainer-core";
import { dataset, trainerIndex } from "../../lib/dataset";
import { useAppState } from "../../lib/store";
import { useI18n } from "../../lib/i18n";
import { pageParam } from "../../lib/navigation";

export default function KeyDetail() {
  const state = useAppState();
  const { t } = useI18n();
  const language = state.settings.language;

  const key = useMemo(() => {
    const raw = pageParam("key");
    return raw && /^[a-z]$/.test(raw) ? raw : null;
  }, []);

  const derived = useMemo(() => {
    if (!key) return null;
    const initials = dataset.doublePinyin.initials
      .filter((mapping) => mapping.key === key)
      .map((mapping) => mapping.initial);
    const finals = dataset.doublePinyin.finals
      .filter((mapping) => mapping.key === key)
      .map((mapping) => mapping.final);
    const zero = dataset.doublePinyin.zeroInitials.filter(
      (mapping) => mapping.code[0] === key || mapping.code[1] === key,
    );
    const level1 = dataset.level1Shortcuts.filter(
      (shortcut) => shortcut.key === key,
    );
    const shapeStat =
      buildShapeKeyStats(dataset.entries).find((stat) => stat.key === key) ?? null;
    // 含该键的全码例字(教学演示:完整编码来自分片)。
    const examples = trainerIndex.dataset.entries
      .filter((entry) => entry.code.includes(key))
      .slice(0, 8);
    const myErrors = state.keyErrors[key] ?? 0;
    return { initials, finals, zero, level1, shapeStat, examples, myErrors };
  }, [key, state.keyErrors]);

  if (!key || !derived) {
    return (
      <View className="page">
        <View className="card">
          <Text className="title">{t("common.unknownError")}</Text>
          <Text className="subtitle">{t("keyDetail.noneMapped")}</Text>
        </View>
      </View>
    );
  }

  return (
    <View className="page">
      <View className="card key-detail-head">
        <Text className="key-detail-key">{key.toUpperCase()}</Text>
        <Text className="subtitle">{t("keyDetail.subtitle")}</Text>
      </View>

      <View className="card">
        <Text className="card-heading">{t("keyDetail.tabDouble")}</Text>
        <Text className="body-text paragraph">
          {t("reference.initial")}:{derived.initials.length > 0 ? derived.initials.join("、") : t("keyDetail.noneMapped")}
        </Text>
        <Text className="body-text paragraph">
          {t("reference.final")}:{derived.finals.length > 0 ? derived.finals.join("、") : t("keyDetail.noneMapped")}
        </Text>
        {derived.finals.length > 1 && (
          <Text className="subtitle">
            {t("keyDetail.finalList", { finals: derived.finals.join("、") })}
          </Text>
        )}
      </View>

      {derived.level1.length > 0 && (
        <View className="card">
          <Text className="card-heading">{t("reference.kindLevel1")}</Text>
          {derived.level1.map((shortcut) => (
            <Text key={shortcut.key} className="body-text paragraph">
              {key.toUpperCase()} = {shortcut.char}
            </Text>
          ))}
        </View>
      )}

      {derived.zero.length > 0 && (
        <View className="card">
          <Text className="card-heading">{t("reference.zeroInitials")}</Text>
          {derived.zero.map((mapping) => (
            <View className="zero-row" key={mapping.syllable}>
              <Text className="zero-syllable">{mapping.syllable}</Text>
              <Text className="zero-code">{mapping.code}</Text>
            </View>
          ))}
        </View>
      )}

      {(derived.shapeStat?.firstSamples.length ?? 0) + (derived.shapeStat?.secondSamples.length ?? 0) > 0 && (
        <View className="card">
          <Text className="card-heading">{t("keyDetail.tabShape")}</Text>
          <Text className="subtitle">{t("keyDetail.shapeNote")}</Text>
          {(derived.shapeStat?.firstSamples.length ?? 0) > 0 && (
            <Text className="body-text paragraph">
              {t("learn.firstShapeSamples", { key: key.toUpperCase() })}
              {derived.shapeStat!.firstSamples
                .map((sample) => `${sample.char}(${sample.code})`)
                .join(" ")}
            </Text>
          )}
          {(derived.shapeStat?.secondSamples.length ?? 0) > 0 && (
            <Text className="body-text paragraph">
              {t("learn.secondShapeSamples", { key: key.toUpperCase() })}
              {derived.shapeStat!.secondSamples
                .map((sample) => `${sample.char}(${sample.code})`)
                .join(" ")}
            </Text>
          )}
        </View>
      )}

      {derived.examples.length > 0 && (
        <View className="card">
          <Text className="card-heading">{t("keyDetail.tabExamples")}</Text>
          {derived.examples.map((entry) => (
            <View className="weak-row" key={`${entry.char}:${entry.code}`}>
              <Text className="weak-target">{entry.char}</Text>
              <Text className="weak-code">{entry.code}</Text>
              <Text className="weak-meta">
                {entry.toneReading ?? entry.readings.join(" / ")}
              </Text>
            </View>
          ))}
        </View>
      )}

      <View className="card">
        <Text className="card-heading">{t("keyDetail.tabMine")}</Text>
        <Text className="body-text">
          {derived.myErrors > 0
            ? t("keyDetail.myErrors", { n: derived.myErrors })
            : t("keyDetail.myErrorsNone")}
        </Text>
        <Text className="subtitle">{t("keyDetail.errorsNote")}</Text>
        <Text className="subtitle">
          {language === "en" ? "Data: canonical shard." : "数据:规范分片(高频子集)。"}
        </Text>
      </View>
    </View>
  );
}
