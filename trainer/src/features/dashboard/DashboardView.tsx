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
import { useTrainerStore } from "@/stores/trainer-store";
import {
  MODE_DESCRIPTIONS,
  MODE_LABELS,
  type PracticeMode,
} from "@/features/practice/types";

const MODE_ORDER: PracticeMode[] = ["double", "sound-shape", "full", "mixed"];
const MODE_KEYS: Partial<Record<PracticeMode, string>> = {
  double: "2 键",
  "sound-shape": "3 键",
  full: "4 键",
  mixed: "2/3/4 键",
};

export function DashboardView({
  onStartPractice,
  onShowReview,
}: {
  onStartPractice: (mode: PracticeMode) => void;
  onShowReview: () => void;
}) {
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
          <CardTitle className="text-xl">小鹤音形训练</CardTitle>
          <CardDescription>
            从双拼到全码,每天几分钟,把编码练成肌肉记忆。
          </CardDescription>
        </CardHeader>
        <CardContent>
          <Button size="lg" onClick={() => onStartPractice(lastMode)}>
            <Play aria-hidden />
            开始练习
          </Button>
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
                {MODE_LABELS[mode]}
                <Badge variant="outline">{MODE_KEYS[mode]}</Badge>
              </CardTitle>
              <CardDescription className="text-xs">
                {MODE_DESCRIPTIONS[mode]}
              </CardDescription>
            </CardHeader>
          </Card>
        ))}
      </div>

      <Card>
        <CardHeader>
          <CardTitle>今日</CardTitle>
        </CardHeader>
        <CardContent>
          {today && today.questions > 0 ? (
            <div className="grid grid-cols-2 gap-2 sm:grid-cols-4">
              <StatChip label="练习时间" value={formatDuration(today.practiceMs)} />
              <StatChip label="完成题数" value={today.questions} />
              <StatChip label="准确率" value={formatPercent(todayAccuracy)} />
              <StatChip label="最佳连对" value={today.bestStreak} />
            </div>
          ) : (
            <p className="text-sm text-muted-foreground">
              今天还没有练习,从「双拼」开始第一组吧。
            </p>
          )}
        </CardContent>
      </Card>

      {weakItems.length > 0 && (
        <Card>
          <CardHeader className="flex-row items-center justify-between">
            <CardTitle>需要复习</CardTitle>
            <Button variant="ghost" size="sm" onClick={onShowReview}>
              查看全部
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
                    掌握 {progress.mastery}
                  </Badge>
                  错 {progress.wrong}
                </span>
              </div>
            ))}
          </CardContent>
        </Card>
      )}

      <Card>
        <CardHeader>
          <CardTitle>推荐路径</CardTitle>
        </CardHeader>
        <CardContent>
          <ol className="flex flex-col gap-2 text-sm text-muted-foreground sm:flex-row sm:flex-wrap sm:items-center sm:gap-3">
            {MODE_ORDER.map((mode, step) => (
              <li key={mode} className="flex items-center gap-2">
                <span className="flex size-6 items-center justify-center rounded-full bg-primary/10 text-xs font-semibold text-primary">
                  {step + 1}
                </span>
                {MODE_LABELS[mode]}({MODE_KEYS[mode]})
              </li>
            ))}
          </ol>
        </CardContent>
      </Card>
    </div>
  );
}
