import { ArrowRight, Play } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { StatChip } from "@/components/StatChip";
import { accuracy, formatDuration, formatPercent, localDateKey } from "@/lib/stats";
import { listWeakItems } from "@/lib/review";
import { useTrainerIndex } from "@/lib/trainer-context";
import { useI18n } from "@/lib/use-i18n";
import { useTrainerStore } from "@/stores/trainer-store";
import type { I18nKey } from "@/lib/i18n";
import {
  MODE_DESCRIPTIONS,
  MODE_LABELS,
  type PracticeMode,
} from "@/features/practice/types";

const MODE_ORDER = ["double", "sound-shape", "full", "mixed"] as const;
const MODE_KEYS: Record<(typeof MODE_ORDER)[number], I18nKey> = {
  double: "dashboard.modeKeyDouble",
  "sound-shape": "dashboard.modeKeySoundShape",
  full: "dashboard.modeKeyFull",
  mixed: "dashboard.modeKeyMixed",
};

export function DashboardView({
  onStartPractice,
  onShowReview,
  onOpenLearn,
}: {
  onStartPractice: (mode: PracticeMode) => void;
  onShowReview: () => void;
  onOpenLearn?: () => void;
}) {
  const { t } = useI18n();
  const index = useTrainerIndex();
  const lastMode = useTrainerStore((state) => state.lastMode);
  const progress = useTrainerStore((state) => state.progress);
  const today = useTrainerStore((state) => state.daily[localDateKey()]);
  const weakItems = listWeakItems(index, progress, 5);
  const todayAccuracy = today
    ? accuracy(today.keystrokes, today.wrongKeyEvents)
    : null;

  return (
    <div className="flex flex-col gap-4">
      <Card className="bg-primary/5">
        <CardHeader>
          <CardTitle className="text-xl">{t("app.subtitle")}</CardTitle>
          <CardDescription>{t("dashboard.tagline")}</CardDescription>
        </CardHeader>
        <CardContent>
          <Button size="lg" onClick={() => onStartPractice(lastMode)}>
            <Play aria-hidden />
            {t("practice.start")}
          </Button>
          {onOpenLearn && (
            <Button
              size="lg"
              variant="ghost"
              className="ml-2"
              onClick={onOpenLearn}
            >
              {t("dashboard.learnCta")}
            </Button>
          )}
        </CardContent>
      </Card>

      <div className="grid grid-cols-2 gap-3 lg:grid-cols-4">
        {MODE_ORDER.map((mode) => (
          <Card
            key={mode}
            role="button"
            tabIndex={0}
            onClick={() => onStartPractice(mode)}
            onKeyDown={(event) => {
              if (event.key === "Enter" || event.key === " ") {
                event.preventDefault();
                onStartPractice(mode);
              }
            }}
            className="cursor-pointer transition-colors hover:border-primary/50 focus-visible:outline-2 focus-visible:outline-ring"
          >
            <CardHeader className="p-4">
              <CardTitle className="flex items-center justify-between text-base">
                {t(MODE_LABELS[mode])}
                <Badge variant="outline">{t(MODE_KEYS[mode])}</Badge>
              </CardTitle>
              <CardDescription className="text-xs">
                {t(MODE_DESCRIPTIONS[mode])}
              </CardDescription>
            </CardHeader>
          </Card>
        ))}
      </div>

      <Card>
        <CardHeader>
          <CardTitle>{t("nav.today")}</CardTitle>
        </CardHeader>
        <CardContent>
          {today && today.questions > 0 ? (
            <div className="grid grid-cols-2 gap-2 sm:grid-cols-4">
              <StatChip
                label={t("dashboard.practiceTime")}
                value={formatDuration(today.practiceMs)}
              />
              <StatChip label={t("dashboard.completed")} value={today.questions} />
              <StatChip
                label={t("common.accuracy")}
                value={formatPercent(todayAccuracy)}
              />
              <StatChip label={t("dashboard.bestStreak")} value={today.bestStreak} />
            </div>
          ) : (
            <p className="text-sm text-muted-foreground">
              {t("dashboard.todayEmpty")}
            </p>
          )}
        </CardContent>
      </Card>

      {weakItems.length > 0 && (
        <Card>
          <CardHeader className="flex-row items-center justify-between">
            <CardTitle>{t("dashboard.needsReview")}</CardTitle>
            <Button variant="ghost" size="sm" onClick={onShowReview}>
              {t("dashboard.viewAll")}
              <ArrowRight aria-hidden />
            </Button>
          </CardHeader>
          <CardContent className="flex flex-col gap-2">
            {weakItems.map(({ item, progress }) => (
              <div
                key={item.id}
                className="flex items-center gap-3 rounded-lg border border-border px-3 py-2"
              >
                <span className="text-2xl font-medium">{item.target}</span>
                <span className="font-mono text-sm text-muted-foreground">
                  {item.primaryCode}
                </span>
                <span className="ml-auto flex items-center gap-2 text-xs text-muted-foreground">
                  <Badge variant={progress.mastery < 40 ? "destructive" : "outline"}>
                    {t("common.mastery", { n: progress.mastery })}
                  </Badge>
                  {t("common.wrongShort", { n: progress.wrong })}
                </span>
              </div>
            ))}
          </CardContent>
        </Card>
      )}

      <Card>
        <CardHeader>
          <CardTitle>{t("dashboard.recommendedPath")}</CardTitle>
        </CardHeader>
        <CardContent>
          <ol className="flex flex-col gap-2 text-sm text-muted-foreground sm:flex-row sm:flex-wrap sm:items-center sm:gap-3">
            {MODE_ORDER.map((mode, step) => (
              <li key={mode} className="flex items-center gap-2">
                <span className="flex size-6 items-center justify-center rounded-full bg-primary/10 text-xs font-semibold text-primary">
                  {step + 1}
                </span>
                {t(MODE_LABELS[mode])}({t(MODE_KEYS[mode])})
              </li>
            ))}
          </ol>
        </CardContent>
      </Card>
    </div>
  );
}
