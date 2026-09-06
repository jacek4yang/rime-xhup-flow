/**
 * 统计页:全部来自本机持久化状态(按日统计、稀疏进度、键位错误),
 * 数学复用共享核心 stats 纯函数,与桌面端 StatsView 同一语义:
 * - 今日:practiceMs / questions / keystrokes / accuracy / KPM / CPM /
 *   键字比(keystrokes / chars,除零回空)/ corrections;
 * - 近 14 天:纯 View/Text 等比条形(无图表依赖),高度按当日 practiceMs;
 * - 掌握度分布:与桌面端同一分桶公式(0-19/…/80-100,attempts=0 跳过),
 *   边界目前内联在桌面组件中,此处按同一口径复刻(stats-parity 测试锚定);
 * - 键位错误 Top 5。
 */

import { useMemo, useState } from "react";
import { View, Text } from "@tarojs/components";
import { useDidShow } from "@tarojs/taro";
import {
  accuracy,
  cpm,
  emptyDailyStats,
  formatDuration,
  formatPercent,
  kpm,
  localDateKey,
} from "@xhup/trainer-core";
import { trainerIndex } from "../../lib/dataset";
import { useAppState } from "../../lib/store";
import { useI18n } from "../../lib/i18n";
import { StatChip } from "../../components/StatChip";

const DAY_MS = 24 * 60 * 60 * 1000;
const TREND_DAYS = 14;

/** 与桌面端 StatsView 同一掌握度分桶公式(见 stats-parity 测试注释)。 */
function masteryBucket(mastery: number): number {
  return Math.min(4, Math.floor(mastery / 20));
}

export default function Stats() {
  const state = useAppState();
  const { t } = useI18n();
  const [, setTick] = useState(0);

  // useDidShow 刷新显示(今日/趋势随时间与练习变化)。
  useDidShow(() => setTick((n) => n + 1));

  const today = state.daily[localDateKey()] ?? emptyDailyStats();

  // 近 14 天(含无练习的占位日),按当日练习分钟数等比成条。
  const points = useMemo(() => {
    const now = new Date();
    const list: { dateKey: string; minutes: number }[] = [];
    for (let offset = TREND_DAYS - 1; offset >= 0; offset -= 1) {
      const dateKey = localDateKey(new Date(now.getTime() - offset * DAY_MS));
      const day = state.daily[dateKey];
      list.push({
        dateKey,
        minutes: Math.round((day?.practiceMs ?? 0) / 60000),
      });
    }
    return list;
  }, [state.daily]);
  const maxMinutes = Math.max(0, ...points.map((point) => point.minutes));

  // 掌握度分桶(与桌面端 StatsView 内联公式一致)。
  const masteryBuckets = useMemo(() => {
    const buckets = [0, 0, 0, 0, 0];
    for (const itemProgress of Object.values(state.progress)) {
      if (itemProgress.attempts === 0) continue;
      buckets[masteryBucket(itemProgress.mastery)] += 1;
    }
    return buckets;
  }, [state.progress]);
  const totalTrained = masteryBuckets.reduce((sum, count) => sum + count, 0);
  const notStarted = Math.max(0, trainerIndex.byId.size - totalTrained);

  const topErrors = useMemo(
    () =>
      Object.entries(state.keyErrors)
        .sort(([, a], [, b]) => b - a)
        .slice(0, 5),
    [state.keyErrors],
  );

  const totalDays = Object.keys(state.daily).length;
  const accuracyValue = accuracy(today.keystrokes, today.wrongKeyEvents);
  const kpmValue = kpm(today.keystrokes, today.practiceMs);
  const cpmValue = cpm(today.chars, today.practiceMs);
  const keysPerChar = today.chars > 0 ? today.keystrokes / today.chars : null;

  if (totalDays === 0) {
    return (
      <View className="page">
        <View className="card">
          <Text className="title">{t("stats.title")}</Text>
          <Text className="subtitle">{t("stats.noData")}</Text>
        </View>
      </View>
    );
  }

  return (
    <View className="page">
      <View className="card">
        <Text className="title">{t("stats.title")}</Text>
        <View className="stat-grid">
          <StatChip label={t("stats.streak")} value={streakDays(state.daily)} />
          <StatChip label={t("stats.totalDays")} value={totalDays} />
          <StatChip label={t("common.chars")} value={totalTrained} />
          <StatChip label={t("stats.notStarted")} value={notStarted} />
        </View>
      </View>

      <View className="card">
        <Text className="card-heading">{t("stats.today")}</Text>
        <View className="stat-grid">
          <StatChip
            label={t("dashboard.practiceTime")}
            value={formatDuration(today.practiceMs)}
          />
          <StatChip label={t("dashboard.completed")} value={today.questions} />
          <StatChip label={t("stats.keystrokes")} value={today.keystrokes} />
          <StatChip label={t("common.accuracy")} value={formatPercent(accuracyValue)} />
          <StatChip
            label={t("common.kpm")}
            value={kpmValue === null ? "—" : Math.round(kpmValue)}
          />
          <StatChip
            label={t("common.cpm")}
            value={cpmValue === null ? "—" : cpmValue.toFixed(1)}
          />
          <StatChip
            label={t("common.keysPerChar")}
            value={keysPerChar === null ? "—" : keysPerChar.toFixed(2)}
          />
          <StatChip label={t("stats.corrections")} value={today.corrections} />
        </View>
      </View>

      <View className="card">
        <Text className="card-heading">{t("stats.last14")}</Text>
        <Text className="subtitle">{t("stats.trendHint")}</Text>
        <View className="bar-chart" aria-hidden>
          {points.map((point) => (
            <View className="bar-col" key={point.dateKey}>
              <View
                className={
                  maxMinutes === 0 || point.minutes === 0
                    ? "bar-fill-empty"
                    : "bar-fill"
                }
                style={{
                  height:
                    maxMinutes === 0 || point.minutes === 0
                      ? "4rpx"
                      : `${Math.max(8, Math.round((point.minutes / maxMinutes) * 140))}rpx`,
                }}
              />
            </View>
          ))}
        </View>
        <View className="bar-labels" aria-hidden>
          <Text className="bar-label">{points[0]?.dateKey.slice(5)}</Text>
          <Text className="bar-label">
            {points[points.length - 1]?.dateKey.slice(5)}
          </Text>
        </View>
      </View>

      <View className="card">
        <Text className="card-heading">{t("stats.masteryDist")}</Text>
        <Text className="subtitle">
          0-19 / 20-39 / 40-59 / 60-79 / 80-100
        </Text>
        <View className="mastery-row" aria-hidden>
          {masteryBuckets.map((count, bucket) => (
            <View className="mastery-col" key={bucket}>
              <Text className="mastery-count">{count}</Text>
              <View
                className={count === 0 ? "bar-fill-empty" : "bar-fill"}
                style={{
                  height:
                    count === 0
                      ? "4rpx"
                      : `${Math.max(
                          8,
                          Math.round(
                            (count / Math.max(1, ...masteryBuckets)) * 120,
                          ),
                        )}rpx`,
                }}
              />
            </View>
          ))}
        </View>
        <View className="mastery-ranges" aria-hidden>
          {masteryBuckets.map((_, bucket) => (
            <Text className="bar-label" key={bucket}>
              {bucket * 20}-{bucket === 4 ? 100 : bucket * 20 + 19}
            </Text>
          ))}
        </View>
      </View>

      <View className="card">
        <Text className="card-heading">{t("stats.topErrors")}</Text>
        {topErrors.length === 0 ? (
          <Text className="subtitle">{t("stats.noErrors")}</Text>
        ) : (
          <View className="error-chip-row">
            {topErrors.map(([key, count]) => (
              <View className="error-chip" key={key}>
                <Text className="error-chip-key">{key.toUpperCase()}</Text>
                <Text className="error-chip-count">×{count}</Text>
              </View>
            ))}
          </View>
        )}
      </View>
    </View>
  );
}

/** 连续练习天数:从今天向过去回溯的每日记录数(与桌面端 dayStreak 同口径)。 */
function streakDays(daily: Record<string, unknown>): number {
  let streak = 0;
  const now = new Date();
  for (let offset = 0; offset < 365; offset += 1) {
    const date = new Date(now.getTime() - offset * DAY_MS);
    if (daily[localDateKey(date)] === undefined) break;
    streak += 1;
  }
  return streak;
}
