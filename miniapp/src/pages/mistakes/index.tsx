/**
 * 错题与弱点页:掌握度排序的薄弱条目清单(核心 listWeakItems)+
 * 键位热力(keyHeatmap)与维度聚合(aggregateWeakness)。
 * 「练这些」把薄弱条目直接送进会话页(src=weak)。
 */

import { useMemo, useState } from "react";
import { View, Text } from "@tarojs/components";
import { useDidShow } from "@tarojs/taro";
import {
  aggregateWeakness,
  itemAccuracy,
  keyHeatmap,
  listWeakItems,
} from "@xhup/trainer-core";
import type { WeakListEntry } from "@xhup/trainer-core";
import { trainerIndex } from "../../lib/dataset";
import { useAppState } from "../../lib/store";
import { useI18n } from "../../lib/i18n";
import { ROUTES, navigateToOrRedirect } from "../../lib/navigation";

export default function Mistakes() {
  const state = useAppState();
  const { t } = useI18n();
  const [, setTick] = useState(0);
  useDidShow(() => setTick((n) => n + 1));

  const weakItems = useMemo<WeakListEntry[]>(
    () => listWeakItems(trainerIndex, state.progress),
    [state.progress],
  );
  const report = useMemo(
    () =>
      aggregateWeakness(
        trainerIndex,
        new Map(Object.entries(state.progress)),
        state.keyErrors,
        20,
      ),
    [state.progress, state.keyErrors],
  );
  const heat = useMemo(() => keyHeatmap(state.keyErrors), [state.keyErrors]);
  const maxHeat = Math.max(1, ...Object.values(heat));

  const practiceWeak = () => {
    if (weakItems.length === 0) return;
    navigateToOrRedirect(ROUTES.session(state.settings.lastMode, "weak"));
  };

  return (
    <View className="page">
      <View className="card">
        <Text className="title">{t("review.title")}</Text>
        <Text className="subtitle">{t("review.subtitle")}</Text>
      </View>

      {weakItems.length === 0 ? (
        <View className="card">
          <Text className="card-heading">{t("review.empty")}</Text>
          <Text className="subtitle">{t("review.emptyHint")}</Text>
        </View>
      ) : (
        <>
          <View className="card">
            <View className="card-heading-row">
              <Text className="card-heading">{t("review.title")}</Text>
              <View className="button button-inline" onClick={practiceWeak}>
                <Text>{t("review.practiceThese", { n: weakItems.length })}</Text>
              </View>
            </View>
            {weakItems.slice(0, 50).map(({ item, progress }) => (
              <View className="weak-row" key={item.id}>
                <Text className="weak-target">{item.target}</Text>
                <Text className="weak-code">{item.primaryCode}</Text>
                <Text className="weak-meta">
                  {t("common.mastery", { n: progress.mastery })} ·{" "}
                  {t("common.wrongCount", { n: progress.wrong })} ·{" "}
                  {t("common.attempts", { n: progress.attempts })} ·{" "}
                  {progress.lastSeenAt === null
                    ? "—"
                    : formatAccuracy(itemAccuracy(progress))}
                </Text>
              </View>
            ))}
          </View>

          {Object.keys(heat).length > 0 && (
            <View className="card">
              <Text className="card-heading">{t("stats.heatmap")}</Text>
              <Text className="subtitle">{t("stats.heatmapHint")}</Text>
              <View className="heat-grid">
                {Object.entries(heat)
                  .sort(([a], [b]) => a.localeCompare(b))
                  .map(([key, count]) => (
                    <View
                      key={key}
                      className="heat-cell"
                      style={{
                        backgroundColor: `rgba(217, 48, 38, ${
                          0.15 + 0.75 * (count / maxHeat)
                        })`,
                      }}
                    >
                      <Text className="heat-key">{key.toUpperCase()}</Text>
                      <Text className="heat-count">{count}</Text>
                    </View>
                  ))}
              </View>
            </View>
          )}

          <View className="card">
            <Text className="card-heading">{t("stats.byCodeLength")}</Text>
            {Object.entries(report.byCodeLength).map(([length, stat]) => (
              <Text key={length} className="body-text paragraph">
                {length} 键:{stat.attempts} 次 · 错 {stat.wrong} 次 · 错误率{" "}
                {Math.round(stat.wrongRate * 100)}%
              </Text>
            ))}
          </View>
        </>
      )}
    </View>
  );
}

function formatAccuracy(ratio: number | null): string {
  if (ratio === null) return "—";
  return `${tPercent(ratio)} 准确`;
}

function tPercent(ratio: number): string {
  return `${Math.round(ratio * 100)}%`;
}
