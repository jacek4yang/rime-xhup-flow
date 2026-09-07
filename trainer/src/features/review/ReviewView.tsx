import { useMemo, useState } from "react";
import { Play, RotateCcw } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogTitle,
} from "@/components/ui/dialog";
import { Progress } from "@/components/ui/progress";
import { cn } from "@/lib/utils";
import { itemAccuracy, listWeakItems } from "@xhup/trainer-core";
import { aggregateWeakness, keyHeatmap, topConfusions, positionLabel } from "@xhup/trainer-core";
import { confusionId } from "@xhup/trainer-core";
import { formatPercent } from "@xhup/trainer-core";
import { useI18n } from "@/lib/use-i18n";
import type { I18nKey } from "@xhup/trainer-core";
import { useTrainerIndex } from "@/lib/trainer-context";
import { useTrainerStore } from "@/stores/trainer-store";
import type { TrainingItem } from "@xhup/trainer-core";

type ModeFilter = "all" | 2 | 3 | 4;

const FILTERS: { key: ModeFilter; label: I18nKey }[] = [
  { key: "all", label: "review.all" },
  { key: 2, label: "practice.modeDouble" },
  { key: 3, label: "practice.modeSoundShape" },
  { key: 4, label: "practice.modeFull" },
];

const LENGTH_LABELS: Record<number, I18nKey> = {
  1: "review.lengthLevel1",
  2: "review.lengthDouble",
  3: "review.lengthSoundShape",
  4: "review.lengthFull",
  6: "review.lengthWord3",
  8: "review.lengthWord4",
};

/** 错误率 → 热力颜色(非唯一颜色反馈;同时有数字标签)。 */
function heatClass(count: number): string {
  if (count >= 10) return "bg-destructive/70 text-destructive-foreground";
  if (count >= 5) return "bg-destructive/45";
  if (count >= 2) return "bg-destructive/25";
  return "bg-destructive/10";
}

export function WeaknessCenter({
  onPracticeEntries,
}: {
  onPracticeEntries: (entries: TrainingItem[]) => void;
}) {
  const index = useTrainerIndex();
  const { t } = useI18n();
  const progress = useTrainerStore((state) => state.progress);
  const keyErrors = useTrainerStore((state) => state.keyErrors);
  const confusions = useTrainerStore((state) => state.confusions);
  const resetItemProgress = useTrainerStore((state) => state.resetItemProgress);
  const [filter, setFilter] = useState<ModeFilter>("all");
  const [selected, setSelected] = useState<ReadonlySet<string>>(new Set());
  const [confirmOpen, setConfirmOpen] = useState(false);

  const weakItems = useMemo(() => listWeakItems(index, progress), [index, progress]);
  const report = useMemo(
    () =>
      aggregateWeakness(
        index,
        new Map(Object.entries(progress)),
        keyErrors,
        50,
      ),
    [index, progress, keyErrors],
  );
  const heat = useMemo(() => keyHeatmap(keyErrors), [keyErrors]);
  // 键位混淆:按次数取前 8 对(本机训练结果;词/句位降级为 other:N)。
  const topConfusionList = useMemo(() => topConfusions(confusions, 8), [confusions]);
  const filtered =
    filter === "all"
      ? weakItems
      : weakItems.filter((item) => item.item.codeLength === filter);

  const toggleSelected = (id: string) => {
    setSelected((current) => {
      const next = new Set(current);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  const confirmReset = () => {
    resetItemProgress([...selected]);
    setSelected(new Set());
    setConfirmOpen(false);
  };

  return (
    <div className="flex flex-col gap-4">
      <header className="flex flex-wrap items-center gap-3">
        <div>
          <h1 className="text-xl font-semibold">{t("review.title")}</h1>
          <p className="mt-1 text-sm text-muted-foreground">{t("review.subtitle")}</p>
        </div>
        {filtered.length > 0 && (
          <Button
            className="ml-auto"
            onClick={() => onPracticeEntries(filtered.map((item) => item.item))}
          >
            <Play aria-hidden />
            {t("review.practiceThese", { n: filtered.length })}
          </Button>
        )}
      </header>

      <div className="flex flex-wrap gap-2">
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
            {t(candidate.label)}
          </button>
        ))}
        {selected.size > 0 && (
          <Button
            variant="outline"
            className="ml-auto"
            onClick={() => setConfirmOpen(true)}
          >
            <RotateCcw aria-hidden />
            {t("review.resetMastery")}({selected.size})
          </Button>
        )}
      </div>

      {filtered.length === 0 ? (
        <Card>
          <CardHeader>
            <CardTitle>{t("review.empty")}</CardTitle>
            <CardDescription>
              {weakItems.length === 0 ? t("review.emptyHint") : t("review.emptyFiltered")}
            </CardDescription>
          </CardHeader>
        </Card>
      ) : (
        <div className="flex flex-col gap-2">
          {filtered.map(({ item, progress: itemProgress }) => (
            <Card key={item.id} className="px-4 py-3">
              <div className="flex items-center gap-3">
                <label className="flex cursor-pointer items-center gap-3">
                  <input
                    type="checkbox"
                    checked={selected.has(item.id)}
                    onChange={() => toggleSelected(item.id)}
                    aria-label={`${item.target} ${t("review.resetMastery")}`}
                    className="size-4 accent-primary"
                  />
                  <span className="text-3xl font-medium">{item.target}</span>
                </label>
                <div className="flex flex-col">
                  <span className="font-mono text-sm">{item.primaryCode}</span>
                  <span className="text-xs text-muted-foreground">
                    {LENGTH_LABELS[item.codeLength]
                      ? t(LENGTH_LABELS[item.codeLength])
                      : item.codeLength}{" "}
                    · {t("common.attempts", { n: itemProgress.attempts })} ·{" "}
                    {t("common.wrongCount", { n: itemProgress.wrong })}
                  </span>
                </div>
                <div className="ml-auto flex w-28 flex-col items-end gap-1 sm:w-36">
                  <div className="flex items-center gap-2 text-xs text-muted-foreground">
                    <Badge
                      variant={itemProgress.mastery < 40 ? "destructive" : "outline"}
                    >
                      {t("common.mastery", { n: itemProgress.mastery })}
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

      {Object.keys(heat).length > 0 && (
        <Card>
          <CardHeader>
            <CardTitle>{t("stats.heatmap")}</CardTitle>
            <CardDescription>{t("stats.heatmapHint")}</CardDescription>
          </CardHeader>
          <CardContent className="flex flex-wrap gap-1.5">
            {Object.entries(heat)
              .sort(([, a], [, b]) => b - a)
              .slice(0, 12)
              .map(([key, count]) => (
                <span
                  key={key}
                  className={`flex size-10 items-center justify-center rounded-md font-mono text-sm font-medium ${heatClass(count)}`}
                >
                  {key}
                  <span className="sr-only">{count}</span>
                </span>
              ))}
          </CardContent>
        </Card>
      )}

      {topConfusionList.length > 0 && (
        <Card>
          <CardHeader>
            <CardTitle>{t("confusion.title")}</CardTitle>
            <CardDescription>{t("confusion.hint")}</CardDescription>
          </CardHeader>
          <CardContent className="flex flex-col gap-2">
            {topConfusionList.map((entry) => {
              const label = positionLabel(entry.position);
              return (
                <div
                  key={confusionId(entry.expected, entry.actual, entry.position)}
                  className="flex items-center gap-3 rounded-lg border border-border px-3 py-2"
                >
                  <span className="font-mono text-sm font-semibold">
                    {entry.expected.toUpperCase()} → {entry.actual.toUpperCase()}
                  </span>
                  <Badge variant="outline" className="text-xs">
                    {t(label.key, label.params)}
                  </Badge>
                  <span className="ml-auto text-xs tabular-nums text-muted-foreground">
                    ×{entry.count}
                  </span>
                </div>
              );
            })}
          </CardContent>
        </Card>
      )}

      {report.recentMistakes.length > 0 && (
        <Card>
          <CardHeader>
            <CardTitle>{t("review.title")}</CardTitle>
          </CardHeader>
          <CardContent className="flex flex-wrap gap-2">
            {report.recentMistakes.slice(0, 8).map((weak) => (
              <Badge key={weak.id} variant="outline" className="text-base">
                {weak.target}
              </Badge>
            ))}
          </CardContent>
        </Card>
      )}

      <Dialog open={confirmOpen} onOpenChange={setConfirmOpen}>
        <DialogContent>
          <DialogTitle>{t("review.resetMastery")}</DialogTitle>
          <DialogDescription>{t("review.resetConfirm")}</DialogDescription>
          <DialogFooter>
            <Button variant="outline" onClick={() => setConfirmOpen(false)}>
              {t("common.cancel")}
            </Button>
            <Button variant="destructive" onClick={confirmReset}>
              {t("review.resetMastery")}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}
