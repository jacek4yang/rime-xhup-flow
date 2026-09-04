/**
 * 键位知识面板:点选任一字母键,分页查看双拼映射、形码统计、
 * 例字与个人误按记录。
 *
 * 数据全部来自规范训练数据与本机练习统计;不编造任何字根含义——
 * 形码分页展示的是「哪些字用了这个形键」,而非「这个键代表什么部首」。
 */

import { useMemo, useState } from "react";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogTitle,
} from "@/components/ui/dialog";
import { cn } from "@/lib/utils";
import { useI18n } from "@/lib/use-i18n";
import type { I18nKey } from "@/lib/i18n";
import { useTrainerIndex } from "@/lib/trainer-context";
import { useTrainerStore } from "@/stores/trainer-store";
import { buildKeyLabels, compactFinals, compactInitials } from "@/components/OnScreenKeyboard";
import { buildShapeKeyStats } from "@/features/learn/shape-explorer";

type KeyDetailTab = "double" | "shape" | "examples" | "mine";

const TABS: readonly { id: KeyDetailTab; label: I18nKey }[] = [
  { id: "double", label: "keyDetail.tabDouble" },
  { id: "shape", label: "keyDetail.tabShape" },
  { id: "examples", label: "keyDetail.tabExamples" },
  { id: "mine", label: "keyDetail.tabMine" },
];

export function KeyDetailDialog({
  keyChar,
  open,
  onOpenChange,
}: {
  keyChar: string;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const { t } = useI18n();
  const index = useTrainerIndex();
  const keyErrors = useTrainerStore((state) => state.keyErrors);
  const [tab, setTab] = useState<KeyDetailTab>("double");

  const labels = useMemo(
    () => buildKeyLabels(index.dataset.doublePinyin).get(keyChar),
    [index.dataset.doublePinyin, keyChar],
  );

  const shapeStat = useMemo(() => {
    const stats = buildShapeKeyStats(index.dataset.entries);
    return stats.find((stat) => stat.key === keyChar) ?? null;
  }, [index.dataset.entries, keyChar]);

  // 例字:编码中含此键的条目(全码/词码),按频率取前 12。
  const examples = useMemo(() => {
    const matches: { target: string; code: string }[] = [];
    for (const item of index.byId.values()) {
      if (item.primaryCode.includes(keyChar)) {
        matches.push({ target: item.target, code: item.primaryCode });
        if (matches.length >= 12) break;
      }
    }
    return matches;
  }, [index, keyChar]);

  const myErrorCount = keyErrors[keyChar] ?? 0;

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent aria-describedby="key-detail-description">
        <DialogTitle>
          {t("keyDetail.title", { key: keyChar.toUpperCase() })}
        </DialogTitle>
        <DialogDescription id="key-detail-description">
          {t("keyDetail.subtitle")}
        </DialogDescription>

        <div className="mt-3 flex gap-1.5" role="tablist" aria-label={t("keyDetail.tabsAria")}>
          {TABS.map((candidate) => (
            <button
              key={candidate.id}
              type="button"
              role="tab"
              aria-selected={tab === candidate.id}
              onClick={() => setTab(candidate.id)}
              className={cn(
                "min-h-9 flex-1 rounded-lg border px-2 text-xs font-medium transition-colors focus-visible:outline-2 focus-visible:outline-ring",
                tab === candidate.id
                  ? "border-primary bg-primary/10 text-primary"
                  : "border-border text-muted-foreground hover:bg-accent",
              )}
            >
              {t(candidate.label)}
            </button>
          ))}
        </div>

        <div className="mt-3 min-h-32 text-sm">
          {tab === "double" && (
            <div className="flex flex-col gap-2">
              <Row
                label={t("reference.initial")}
                value={labels?.initials.length ? compactInitials(labels.initials) : t("keyDetail.noneMapped")}
              />
              <Row
                label={t("reference.final")}
                value={labels?.finals.length ? compactFinals(labels.finals) : t("keyDetail.noneMapped")}
              />
              {labels && labels.finals.length > 1 && (
                <p className="text-xs text-muted-foreground">
                  {t("keyDetail.finalList", { finals: labels.finals.join(" / ") })}
                </p>
              )}
            </div>
          )}

          {tab === "shape" && (
            <div className="flex flex-col gap-2">
              <Row
                label={t("keyDetail.asFirstShape")}
                value={shapeStat ? t("learn.shapeCharCount", { n: shapeStat.firstCount }) : t("keyDetail.noneMapped")}
              />
              {shapeStat && shapeStat.firstSamples.length > 0 && (
                <SampleGrid samples={shapeStat.firstSamples} />
              )}
              <Row
                label={t("keyDetail.asSecondShape")}
                value={shapeStat ? t("learn.shapeCharCount", { n: shapeStat.secondCount }) : t("keyDetail.noneMapped")}
              />
              {shapeStat && shapeStat.secondSamples.length > 0 && (
                <SampleGrid samples={shapeStat.secondSamples} />
              )}
              <p className="text-xs text-muted-foreground">{t("keyDetail.shapeNote")}</p>
            </div>
          )}

          {tab === "examples" && (
            <div className="flex flex-col gap-2">
              {examples.length === 0 ? (
                <p className="text-sm text-muted-foreground">{t("keyDetail.noExamples")}</p>
              ) : (
                <ul className="grid grid-cols-3 gap-1.5 sm:grid-cols-4">
                  {examples.map((example) => (
                    <li
                      key={example.code + example.target}
                      className="flex flex-col items-center rounded bg-muted/60 px-1 py-1.5"
                    >
                      <span className="text-base">{example.target}</span>
                      <span className="font-mono text-[10px] text-muted-foreground">
                        {example.code}
                      </span>
                    </li>
                  ))}
                </ul>
              )}
            </div>
          )}

          {tab === "mine" && (
            <div className="flex flex-col gap-2">
              <p className="text-sm">
                {myErrorCount > 0
                  ? t("keyDetail.myErrors", { n: myErrorCount })
                  : t("keyDetail.myErrorsNone")}
              </p>
              <p className="text-xs text-muted-foreground">{t("keyDetail.errorsNote")}</p>
            </div>
          )}
        </div>
      </DialogContent>
    </Dialog>
  );
}

function Row({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex items-baseline justify-between gap-2 rounded-lg border border-border px-3 py-2">
      <span className="text-xs text-muted-foreground">{label}</span>
      <span className="font-mono text-sm font-semibold">{value}</span>
    </div>
  );
}

function SampleGrid({
  samples,
}: {
  samples: readonly { char: string; code: string }[];
}) {
  return (
    <ul className="grid grid-cols-4 gap-1.5 sm:grid-cols-6">
      {samples.map((sample) => (
        <li
          key={sample.code + sample.char}
          className="flex flex-col items-center rounded bg-muted/60 px-1 py-1"
        >
          <span className="text-sm">{sample.char}</span>
          <span className="font-mono text-[10px] text-muted-foreground">{sample.code}</span>
        </li>
      ))}
    </ul>
  );
}
