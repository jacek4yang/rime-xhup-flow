import { House, RotateCcw, Target } from "lucide-react";
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
import { accuracy, cpm, formatDuration, formatPercent, kpm } from "@/lib/stats";
import { useI18n } from "@/lib/use-i18n";
import type { WeakItem } from "@/lib/review";
import type { SessionState } from "./engine";

/** 会话结束后的内联小结:关键指标 + 最需要复习的几项 + 下一步动作。 */
export function SessionSummary({
  session,
  weakItems,
  onRestart,
  onPracticeWeak,
  onExitToToday,
}: {
  session: SessionState;
  weakItems: WeakItem[];
  onRestart: () => void;
  onPracticeWeak: () => void;
  onExitToToday: () => void;
}) {
  const { t, language } = useI18n();
  const sessionKpm = kpm(session.keystrokes, session.activeMs);
  const sessionCpm = cpm(session.charsCompleted, session.activeMs);
  return (
    <div className="mx-auto flex w-full max-w-2xl flex-col gap-4">
      <Card>
        <CardHeader>
          <CardTitle>{t("common.summaryTitle")}</CardTitle>
          <CardDescription>
            {t("practice.progressUnlimited", { n: session.questionsCompleted })} ·{" "}
            {session.config.targetLength > 0
              ? language === "zh"
                ? "已达目标"
                : "Target reached"
              : language === "zh"
                ? "手动结束"
                : "Ended manually"}
          </CardDescription>
        </CardHeader>
        <CardContent>
          <div className="grid grid-cols-2 gap-2 sm:grid-cols-4">
            <StatChip
              label={t("common.accuracy")}
              value={formatPercent(
                accuracy(session.keystrokes, session.wrongKeyEvents),
              )}
            />
            <StatChip
              label={t("common.kpm")}
              value={sessionKpm === null ? "—" : Math.round(sessionKpm)}
            />
            <StatChip label={t("common.elapsed")} value={formatDuration(session.activeMs)} />
            <StatChip label={t("common.chars")} value={session.charsCompleted} />
            {sessionCpm !== null && (
              <StatChip label={t("common.cpm")} value={Math.round(sessionCpm)} />
            )}
            <StatChip label={t("common.streak")} value={session.bestStreak} />
            <StatChip label={session.perfect >= session.imperfect ? "✓" : "✗"} value={`${session.perfect} / ${session.imperfect}`} />
            <StatChip label={t("common.keysPerChar")} value={session.charsCompleted === 0 ? "—" : (session.keystrokes / session.charsCompleted).toFixed(1)} />
          </div>
        </CardContent>
      </Card>

      {weakItems.length > 0 && (
        <Card>
          <CardHeader>
            <CardTitle>最需要复习</CardTitle>
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
                <span className="ml-auto flex items-center gap-2">
                  <Badge variant={progress.mastery < 40 ? "destructive" : "outline"}>
                    掌握 {progress.mastery}
                  </Badge>
                  <span className="text-xs text-muted-foreground">
                    错 {progress.wrong}
                  </span>
                </span>
              </div>
            ))}
          </CardContent>
        </Card>
      )}

      <div className="flex flex-col gap-2 sm:flex-row">
        <Button size="lg" onClick={onRestart}>
          <RotateCcw aria-hidden />
          再来一组
        </Button>
        {weakItems.length > 0 && (
          <Button size="lg" variant="secondary" onClick={onPracticeWeak}>
            <Target aria-hidden />
            练习错题
          </Button>
        )}
        <Button size="lg" variant="outline" onClick={onExitToToday}>
          <House aria-hidden />
          回到今日
        </Button>
      </div>
    </div>
  );
}
