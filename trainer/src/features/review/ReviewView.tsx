import { useMemo, useState } from "react";
import { Play } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import {
  Card,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Progress } from "@/components/ui/progress";
import { cn } from "@/lib/utils";
import { itemAccuracy, listWeakItems } from "@/lib/review";
import { formatPercent } from "@/lib/stats";
import { useTrainerIndex } from "@/lib/trainer-context";
import { useTrainerStore } from "@/stores/trainer-store";
import type { TrainingItem } from "@/lib/trainer-index";

type ModeFilter = "all" | 2 | 3 | 4;

const FILTERS: { key: ModeFilter; label: string }[] = [
  { key: "all", label: "全部" },
  { key: 2, label: "双拼" },
  { key: 3, label: "音形" },
  { key: 4, label: "全码" },
];

const LENGTH_LABELS: Record<number, string> = {
  2: "双拼",
  3: "音形",
  4: "全码",
};

export function ReviewView({
  onPracticeEntries,
}: {
  onPracticeEntries: (entries: TrainingItem[]) => void;
}) {
  const index = useTrainerIndex();
  const progress = useTrainerStore((state) => state.progress);
  const [filter, setFilter] = useState<ModeFilter>("all");

  const weakItems = useMemo(() => listWeakItems(index, progress), [index, progress]);
  const filtered =
    filter === "all"
      ? weakItems
      : weakItems.filter((item) => item.item.codeLength === filter);

  return (
    <div className="flex flex-col gap-4">
      <header className="flex flex-wrap items-center gap-3">
        <div>
          <h1 className="text-xl font-semibold">错题</h1>
          <p className="mt-1 text-sm text-muted-foreground">
            按掌握度排序,从最薄弱的开始复习。
          </p>
        </div>
        {filtered.length > 0 && (
          <Button
            className="ml-auto"
            onClick={() => onPracticeEntries(filtered.map((item) => item.item))}
          >
            <Play aria-hidden />
            练这些({filtered.length})
          </Button>
        )}
      </header>

      <div className="flex gap-2">
        {FILTERS.map((candidate) => (
          <button
            key={candidate.key}
            type="button"
            aria-pressed={filter === candidate.key}
            onClick={() => setFilter(candidate.key)}
            className={cn(
              "min-h-11 rounded-md border px-4 text-sm font-medium transition-colors focus-visible:outline-2 focus-visible:outline-ring",
              filter === candidate.key
                ? "border-primary bg-primary/10 text-primary"
                : "border-border text-muted-foreground hover:bg-accent",
            )}
          >
            {candidate.label}
          </button>
        ))}
      </div>

      {filtered.length === 0 ? (
        <Card>
          <CardHeader>
            <CardTitle>暂无错题</CardTitle>
            <CardDescription>
              {weakItems.length === 0
                ? "还没有练习记录。完成几组练习后,答错的字会出现在这里。"
                : "当前筛选下没有错题,换个模式看看。"}
            </CardDescription>
          </CardHeader>
        </Card>
      ) : (
        <div className="flex flex-col gap-2">
          {filtered.map(({ item, progress: itemProgress }) => (
            <Card key={item.id} className="px-4 py-3">
              <div className="flex items-center gap-3">
                <span className="text-3xl font-medium">{item.target}</span>
                <div className="flex flex-col">
                  <span className="font-mono text-sm">{item.primaryCode}</span>
                  <span className="text-xs text-muted-foreground">
                    {LENGTH_LABELS[item.codeLength] ?? `${item.codeLength} 键`} · 练{" "}
                    {itemProgress.attempts} 次 · 错 {itemProgress.wrong} 次
                  </span>
                </div>
                <div className="ml-auto flex w-28 flex-col items-end gap-1 sm:w-36">
                  <div className="flex items-center gap-2 text-xs text-muted-foreground">
                    <Badge variant={itemProgress.mastery < 40 ? "destructive" : "outline"}>
                      掌握 {itemProgress.mastery}
                    </Badge>
                    <span>{formatPercent(itemAccuracy(itemProgress))}</span>
                  </div>
                  <Progress value={itemProgress.mastery / 100} className="h-1.5" />
                </div>
              </div>
            </Card>
          ))}
        </div>
      )}
    </div>
  );
}
