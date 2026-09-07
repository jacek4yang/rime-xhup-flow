/**
 * 统计视图:全部本地真实数据(按日统计 + 稀疏进度),无遥测无服务端。
 *
 * 图表用纯 div 条形,不引入图表依赖;零历史状态如实显示空态。
 */

import { useMemo } from "react";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { localDateKey } from "@/lib/stats";
import { aggregateWeakness } from "@/lib/weakness";
import { useI18n } from "@/lib/use-i18n";
import { useTrainerIndex } from "@/lib/trainer-context";
import { useTrainerStore } from "@/stores/trainer-store";

const DAY_MS = 24 * 60 * 60 * 1000;
const TREND_DAYS = 14;

type DayPoint = {
  dateKey: string;
  minutes: number;
  kpm: number | null;
  cpm: number | null;
};

/** 近 N 天的按日数据点(含无练习的占位日,便于看趋势)。 */
function last14Days(
  daily: Record<
    string,
    {
      practiceMs: number;
      questions: number;
      keystrokes: number;
      chars: number;
    }
  >,
): DayPoint[] {
  const points: DayPoint[] = [];
  const today = new Date();
  for (let offset = TREND_DAYS - 1; offset >= 0; offset -= 1) {
    const date = new Date(today.getTime() - offset * DAY_MS);
    const dateKey = localDateKey(date);
    const stats = daily[dateKey];
    if (!stats || stats.questions === 0) {
      points.push({ dateKey, minutes: 0, kpm: null, cpm: null });
      continue;
    }
    points.push({
      dateKey,
      minutes: Math.round(stats.practiceMs / 60000),
      kpm:
        stats.practiceMs < 1000
          ? null
          : stats.keystrokes / (stats.practiceMs / 60000),
      cpm:
        stats.practiceMs < 1000 || stats.chars === 0
          ? null
          : stats.chars / (stats.practiceMs / 60000),
    });
  }
  return points;
}

function dayStreak(daily: Record<string, unknown>): number {
  let streak = 0;
  const today = new Date();
  for (let offset = 0; offset < 365; offset += 1) {
    const date = new Date(today.getTime() - offset * DAY_MS);
    if (daily[localDateKey(date)] === undefined) break;
    streak += 1;
  }
  return streak;
}

function TrendBars({
  points,
  label,
  valueOf,
}: {
  points: DayPoint[];
  label: string;
  valueOf: (point: DayPoint) => number | null;
}) {
  const values = points.map(valueOf);
  const max = Math.max(0, ...values.map((value) => value ?? 0));
  return (
    <div>
      <p className="mb-2 text-xs font-medium text-muted-foreground">{label}</p>
      <div className="flex h-20 items-end gap-1" aria-hidden>
        {points.map((point, index) => {
          const value = values[index] ?? 0;
          return (
            <div
              key={point.dateKey}
              className="flex-1 rounded-sm bg-primary/70"
              style={{
                height: `${max === 0 || value === 0 ? 2 : Math.max(6, (value / max) * 76)}px`,
              }}
              title={`${point.dateKey}: ${value === 0 ? "—" : value.toFixed(value < 10 ? 1 : 0)}`}
            />
          );
        })}
      </div>
      <div
        className="mt-1 flex justify-between text-[10px] text-muted-foreground"
        aria-hidden
      >
        <span>{points[0]?.dateKey.slice(5)}</span>
        <span>{points[points.length - 1]?.dateKey.slice(5)}</span>
      </div>
    </div>
  );
}

export function StatsView() {
  const index = useTrainerIndex();
  const { t } = useI18n();
  const daily = useTrainerStore((state) => state.daily);
  const progress = useTrainerStore((state) => state.progress);
  const keyErrors = useTrainerStore((state) => state.keyErrors);

  const points = useMemo(() => last14Days(daily), [daily]);
  const report = useMemo(
    () =>
      aggregateWeakness(index, new Map(Object.entries(progress)), keyErrors, 1),
    [index, progress, keyErrors],
  );

  const masteryBuckets = useMemo(() => {
    const buckets = [0, 0, 0, 0, 0]; // 0-19 / 20-39 / 40-59 / 60-79 / 80-100
    for (const itemProgress of Object.values(progress)) {
      if (itemProgress.attempts === 0) continue;
      buckets[Math.min(4, Math.floor(itemProgress.mastery / 20))] += 1;
    }
    return buckets;
  }, [progress]);

  const totalTrained = masteryBuckets.reduce((sum, count) => sum + count, 0);
  const streak = useMemo(() => dayStreak(daily), [daily]);
  const totalDays = Object.keys(daily).length;

  if (totalDays === 0) {
    return (
      <div className="flex flex-col gap-4">
        <h1 className="text-xl font-semibold">{t("stats.title")}</h1>
        <Card>
          <CardContent className="py-10 text-center text-sm text-muted-foreground">
            {t("stats.noData")}
          </CardContent>
        </Card>
      </div>
    );
  }

  const codeLengths = Object.entries(report.byCodeLength).sort(
    ([a], [b]) => Number(a) - Number(b),
  );

  return (
    <div className="flex flex-col gap-4">
      <h1 className="text-xl font-semibold">{t("stats.title")}</h1>

      <div className="grid grid-cols-2 gap-2 sm:grid-cols-3">
        <Card className="px-4 py-3">
          <p className="text-xs text-muted-foreground">{t("stats.streak")}</p>
          <p className="text-2xl font-semibold tabular-nums">{streak}</p>
        </Card>
        <Card className="px-4 py-3">
          <p className="text-xs text-muted-foreground">{t("stats.totalDays")}</p>
          <p className="text-2xl font-semibold tabular-nums">{totalDays}</p>
        </Card>
        <Card className="px-4 py-3">
          <p className="text-xs text-muted-foreground">{t("common.chars")}</p>
          <p className="text-2xl font-semibold tabular-nums">{totalTrained}</p>
        </Card>
      </div>

      <Card>
        <CardHeader>
          <CardTitle className="text-base">{t("stats.last14")}</CardTitle>
        </CardHeader>
        <CardContent>
          <TrendBars
            points={points}
            label={t("stats.last14")}
            valueOf={(point) => point.minutes}
          />
          <div className="mt-4 grid gap-4 sm:grid-cols-2">
            <TrendBars
              points={points}
              label={t("stats.kpmTrend")}
              valueOf={(point) => point.kpm}
            />
            <TrendBars
              points={points}
              label={t("stats.cpmTrend")}
              valueOf={(point) => point.cpm}
            />
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle className="text-base">{t("stats.masteryDist")}</CardTitle>
          <CardDescription>0-19 / 20-39 / 40-59 / 60-79 / 80-100</CardDescription>
        </CardHeader>
        <CardContent className="flex h-24 items-end gap-2">
          {masteryBuckets.map((count, bucket) => (
            <div key={bucket} className="flex flex-1 flex-col items-center gap-1">
              <span className="text-xs tabular-nums text-muted-foreground">
                {count}
              </span>
              <div
                className="w-full rounded-sm bg-primary/70"
                style={{
                  height: `${count === 0 ? 2 : Math.max(6, (count / Math.max(1, Math.max(...masteryBuckets))) * 64)}px`,
                }}
                aria-hidden
              />
              <span className="text-[10px] text-muted-foreground" aria-hidden>
                {bucket * 20}-{bucket === 4 ? 100 : bucket * 20 + 19}
              </span>
            </div>
          ))}
        </CardContent>
      </Card>

      {codeLengths.length > 0 && (
        <Card>
          <CardHeader>
            <CardTitle className="text-base">{t("stats.byCodeLength")}</CardTitle>
          </CardHeader>
          <CardContent className="flex flex-col gap-2">
            {codeLengths.map(([length, stat]) => (
              <div key={length} className="flex items-center gap-3 text-sm">
                <span className="w-16 font-mono text-muted-foreground">
                  {length} 键
                </span>
                <div className="h-2 flex-1 overflow-hidden rounded-full bg-muted">
                  <div
                    className="h-full rounded-full bg-destructive/60"
                    style={{ width: `${Math.min(100, stat.wrongRate * 100)}%` }}
                    aria-hidden
                  />
                </div>
                <span className="w-24 text-right tabular-nums text-muted-foreground">
                  {Math.round(stat.wrongRate * 100)}% · {stat.attempts}
                </span>
              </div>
            ))}
          </CardContent>
        </Card>
      )}

      {Object.keys(report.keyErrors).length > 0 && (
        <Card>
          <CardHeader>
            <CardTitle className="text-base">{t("stats.heatmap")}</CardTitle>
            <CardDescription>{t("stats.heatmapHint")}</CardDescription>
          </CardHeader>
          <CardContent className="flex flex-wrap gap-1.5">
            {Object.entries(report.keyErrors)
              .sort(([, a], [, b]) => b - a)
              .slice(0, 15)
              .map(([key, count]) => (
                <span
                  key={key}
                  className="flex size-10 items-center justify-center rounded-md border border-border font-mono text-sm"
                  style={{
                    background: `color-mix(in oklab, var(--destructive) ${Math.min(70, count * 8)}%, transparent)`,
                  }}
                >
                  {key}
                </span>
              ))}
          </CardContent>
        </Card>
      )}
    </div>
  );
}
