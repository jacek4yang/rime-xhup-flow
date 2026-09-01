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
import { accuracy, formatDuration, formatPercent, kpm } from "@/lib/stats";
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
  const sessionKpm = kpm(session.keystrokes, session.activeMs);
  return (
    <div className="mx-auto flex w-full max-w-2xl flex-col gap-4">
      <Card>
        <CardHeader>
          <CardTitle>本次练习</CardTitle>
          <CardDescription>
            完成 {session.questionsCompleted} 题 ·{" "}
            {session.config.targetLength > 0 ? "已达目标" : "手动结束"}
          </CardDescription>
        </CardHeader>
        <CardContent>
          <div className="grid grid-cols-2 gap-2 sm:grid-cols-4">
            <StatChip
              label="准确率"
              value={formatPercent(
                accuracy(session.keystrokes, session.wrongKeyEvents),
              )}
            />
            <StatChip
              label="KPM"
              value={sessionKpm === null ? "—" : Math.round(sessionKpm)}
            />
            <StatChip label="用时" value={formatDuration(session.activeMs)} />
            <StatChip label="完成题数" value={session.questionsCompleted} />
            <StatChip label="完美" value={session.perfect} />
            <StatChip label="有误" value={session.imperfect} />
            <StatChip label="最佳连对" value={session.bestStreak} />
            <StatChip label="按键" value={session.keystrokes} />
          </div>
        </CardContent>
      </Card>

      {weakItems.length > 0 && (
        <Card>
          <CardHeader>
            <CardTitle>最需要复习</CardTitle>
          </CardHeader>
          <CardContent className="flex flex-col gap-2">
            {weakItems.map(({ entry, progress }) => (
              <div
                key={`${entry.char}:${entry.code}`}
                className="flex items-center gap-3 rounded-lg border border-border px-3 py-2"
              >
                <span className="text-2xl font-medium">{entry.char}</span>
                <span className="font-mono text-sm text-muted-foreground">
                  {entry.code}
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
