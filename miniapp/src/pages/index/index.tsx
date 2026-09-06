/**
 * 今日(首页):今日状态概览 + 推荐入口 + 学习/练习快捷动作。
 * 数据全部来自本机状态(按日统计、稀疏进度)与共享核心课程。
 */

import { View, Text } from "@tarojs/components";
import { useDidShow } from "@tarojs/taro";
import { useMemo, useState } from "react";
import {
  MODE_LABELS,
  accuracy,
  emptyDailyStats,
  formatDuration,
  formatPercent,
  localDateKey,
} from "@xhup/trainer-core";
import { dataset, trainerIndex } from "../../lib/dataset";
import { useAppState } from "../../lib/store";
import { useI18n } from "../../lib/i18n";
import { ROUTES, navigateTo, switchTab } from "../../lib/navigation";
import { StatChip } from "../../components/StatChip";

export default function Home() {
  const state = useAppState();
  const { t } = useI18n();
  const [, setTick] = useState(0);

  // useDidShow 刷新显示(从其他页返回时今日统计可能已变化)。
  useDidShow(() => setTick((n) => n + 1));

  const today = state.daily[localDateKey()] ?? emptyDailyStats();
  const totalAttempts = useMemo(
    () => Object.values(state.progress).reduce((sum, p) => sum + p.attempts, 0),
    [state.progress],
  );
  const weakCount = useMemo(
    () => Object.values(state.progress).filter((p) => p.wrong > 0).length,
    [state.progress],
  );
  const entryCount =
    trainerIndex.frequencySorted["char-2"].length +
    trainerIndex.frequencySorted["char-3"].length +
    trainerIndex.frequencySorted["char-4"].length;

  return (
    <View className="page">
      <View className="card">
        <Text className="title">{t("app.name")} · {t("nav.today")}</Text>
        <Text className="subtitle">{t("dashboard.tagline")}</Text>
      </View>

      <View className="card">
        <Text className="card-heading">{t("nav.today")}</Text>
        {today.questions === 0 ? (
          <Text className="subtitle">{t("dashboard.todayEmpty")}</Text>
        ) : (
          <View className="stat-grid">
            <StatChip label={t("dashboard.completed")} value={today.questions} />
            <StatChip label={t("dashboard.practiceTime")} value={formatDuration(today.practiceMs)} />
            <StatChip
              label={t("common.accuracy")}
              value={formatPercent(accuracy(today.keystrokes, today.wrongKeyEvents))}
            />
            <StatChip label={t("dashboard.bestStreak")} value={today.bestStreak} />
          </View>
        )}
        {totalAttempts > 0 && (
          <Text className="subtitle">
            {t("common.attempts", { n: totalAttempts })}
          </Text>
        )}
      </View>

      <View className="card">
        <Text className="card-heading">{t("dashboard.recommendedPath")}</Text>
        <View
          className="action-row"
          hoverClass="action-row-hover"
          onClick={() => navigateTo(ROUTES.practiceSetup(state.settings.lastMode))}
        >
          <View className="action-main">
            <Text className="action-title">{t("practice.start")}</Text>
            <Text className="subtitle">{t(MODE_LABELS[state.settings.lastMode])}</Text>
          </View>
          <Text className="action-arrow">›</Text>
        </View>
        <View
          className="action-row"
          hoverClass="action-row-hover"
          onClick={() => switchTab(ROUTES.learn)}
        >
          <View className="action-main">
            <Text className="action-title">{t("nav.learn")}</Text>
            <Text className="subtitle">{t("dashboard.learnCta")}</Text>
          </View>
          <Text className="action-arrow">›</Text>
        </View>
        {weakCount > 0 && (
          <View
            className="action-row"
            hoverClass="action-row-hover"
            onClick={() => navigateTo(ROUTES.mistakes)}
          >
            <View className="action-main">
              <Text className="action-title">{t("nav.review")}</Text>
              <Text className="subtitle">
                {t("dashboard.needsReview")} · {t("common.wrongShort", { n: weakCount })}
              </Text>
            </View>
            <Text className="action-arrow">›</Text>
          </View>
        )}
        <View
          className="action-row"
          hoverClass="action-row-hover"
          onClick={() => navigateTo(ROUTES.stats)}
        >
          <View className="action-main">
            <Text className="action-title">{t("nav.stats")}</Text>
            <Text className="subtitle">{t("stats.entryHint")}</Text>
          </View>
          <Text className="action-arrow">›</Text>
        </View>
      </View>

      <View className="card">
        <Text className="card-heading">{t("nav.reference")}</Text>
        <View className="stat-grid">
          <StatChip label={t("reference.kindChar")} value={entryCount} />
          <StatChip label={t("reference.kindWord")} value={dataset.words.length} />
          <StatChip label={t("reference.kindLevel1")} value={dataset.level1Shortcuts.length} />
          <StatChip label={t("reference.kindSentence")} value={dataset.sentences.length} />
        </View>
        <Text className="subtitle">
          {t("settings.dataSchema")} v{dataset.schemaVersion} · {t("settings.dataVersion")} {dataset.packageVersion}
        </Text>
        <View
          className="action-row"
          hoverClass="action-row-hover"
          onClick={() => navigateTo(ROUTES.keyboard)}
        >
          <View className="action-main">
            <Text className="action-title">{t("nav.reference")}</Text>
            <Text className="subtitle">{t("reference.subtitle")}</Text>
          </View>
          <Text className="action-arrow">›</Text>
        </View>
      </View>
    </View>
  );
}
