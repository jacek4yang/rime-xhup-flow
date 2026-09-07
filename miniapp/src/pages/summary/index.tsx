/**
 * 练习小结页:本场准确率 / 键数 / KPM / CPM / 连对 + 本场错题清单
 * + 重开/回首页。会话状态经 session-holder 进程内传递;缺失时回退首页。
 */

import { useState } from "react";
import { View, Text } from "@tarojs/components";
import Taro, { useDidShow } from "@tarojs/taro";
import {
  MODE_LABELS,
  accuracy,
  cpm,
  formatDuration,
  formatPercent,
  kpm,
} from "@xhup/trainer-core";
import { getFinishedSession, clearFinishedSession } from "../../lib/session-holder";
import { useAppState } from "../../lib/store";
import { useI18n } from "../../lib/i18n";
import { ROUTES, navigateToOrRedirect } from "../../lib/navigation";
import { StatChip } from "../../components/StatChip";

export default function Summary() {
  const state = useAppState();
  const { t } = useI18n();
  // useDidShow 保证从其他页返回时仍能读到 holder(redirectTo 时序)。
  const [, setTick] = useState(0);
  useDidShow(() => setTick((n) => n + 1));

  const finished = getFinishedSession();
  if (!finished) {
    return (
      <View className="page">
        <View className="card">
          <Text className="title">{t("common.summaryTitle")}</Text>
          <Text className="subtitle">没有找到刚结束的练习。</Text>
        </View>
        <View
          className="button"
          onClick={() =>
            Taro.switchTab({ url: ROUTES.home, fail: () => navigateToOrRedirect(ROUTES.home) })
          }
        >
          <Text>{t("summary.backToToday")}</Text>
        </View>
      </View>
    );
  }

  const { state: session, weakItems } = finished;
  const sessionKpm = kpm(session.keystrokes, session.activeMs);
  const sessionCpm = cpm(session.charsCompleted, session.activeMs);

  const restart = () => {
    clearFinishedSession();
    Taro.redirectTo({
      url: `${ROUTES.session(finished.mode, finished.src)}`,
      fail: () => navigateToOrRedirect(ROUTES.session(finished.mode, finished.src)),
    });
  };

  const backHome = () => {
    clearFinishedSession();
    Taro.switchTab({ url: ROUTES.home, fail: () => navigateToOrRedirect(ROUTES.home) });
  };

  return (
    <View className="page">
      <View className="card">
        <Text className="title">{t("common.summaryTitle")}</Text>
        <Text className="subtitle">
          {t(MODE_LABELS[session.config.mode])} ·{" "}
          {session.config.targetLength > 0
            ? t("summary.targetReached")
            : t("summary.endedManually")}
        </Text>
        <View className="stat-grid">
          <StatChip
            label={t("common.accuracy")}
            value={formatPercent(accuracy(session.keystrokes, session.wrongKeyEvents))}
          />
          <StatChip label={t("common.kpm")} value={sessionKpm === null ? "—" : Math.round(sessionKpm)} />
          <StatChip label={t("common.elapsed")} value={formatDuration(session.activeMs)} />
          <StatChip label={t("common.chars")} value={session.charsCompleted} />
          {sessionCpm !== null && (
            <StatChip label={t("common.cpm")} value={Math.round(sessionCpm)} />
          )}
          <StatChip label={t("common.streak")} value={session.bestStreak} />
          <StatChip label="✓ / ✗" value={`${session.perfect} / ${session.imperfect}`} />
          <StatChip
            label={t("common.keysPerChar")}
            value={
              session.charsCompleted === 0
                ? "—"
                : (session.keystrokes / session.charsCompleted).toFixed(1)
            }
          />
        </View>
      </View>

      {weakItems.length > 0 && (
        <View className="card">
          <Text className="card-heading">{t("summary.weak")}</Text>
          {weakItems.slice(0, 10).map((item) => {
            const progress = state.progress[item.id];
            return (
              <View className="weak-row" key={item.id}>
                <Text className="weak-target">{item.target}</Text>
                <Text className="weak-code">{item.primaryCode}</Text>
                {progress && (
                  <Text className="weak-meta">
                    {t("common.mastery", { n: progress.mastery })} ·{" "}
                    {t("common.wrongShort", { n: progress.wrong })}
                  </Text>
                )}
              </View>
            );
          })}
          <View
            className="button button-secondary"
            onClick={() => {
              clearFinishedSession();
              navigateToOrRedirect(ROUTES.mistakes);
            }}
          >
            <Text>{t("nav.review")}</Text>
          </View>
        </View>
      )}

      <View className="button" onClick={restart}>
        <Text>{t("common.restart")}</Text>
      </View>
      <View className="button button-secondary" onClick={backHome}>
        <Text>{t("summary.backToToday")}</Text>
      </View>
    </View>
  );
}
