/**
 * 学习中心:小鹤音形从入门到精通的结构化课程。
 *
 * 内容(canonical 中文)在 `content.ts`;本组件负责章节导航、内容渲染、
 * 形码探索器与「去练习」入口。练习入口复用全局练习引擎的既有模式,
 * 不新建第二套训练逻辑。
 */

import { useMemo, useState } from "react";
import { ArrowLeft, ArrowRight, Play } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { cn } from "@/lib/utils";
import type { I18nKey } from "@/lib/i18n";
import { useI18n } from "@/lib/use-i18n";
import { useTrainerIndex } from "@/lib/trainer-context";
import type { PracticeMode } from "@/features/practice/types";
import { LEARN_CHAPTERS, LEVEL_LABELS, type LearnSection } from "./content";
import {
  buildShapeKeyStats,
  topShapeKeys,
  type ShapeKeyStat,
} from "./shape-explorer";

export function LearnView({
  onStartPractice,
}: {
  onStartPractice: (mode: PracticeMode) => void;
}) {
  const { t } = useI18n();
  const index = useTrainerIndex();
  const [chapterId, setChapterId] = useState<string>(LEARN_CHAPTERS[0].id);
  const chapterIndex = LEARN_CHAPTERS.findIndex((chapter) => chapter.id === chapterId);
  const chapter = LEARN_CHAPTERS[chapterIndex] ?? LEARN_CHAPTERS[0];

  // 全码单字数据只聚合一次。
  const shapeStats = useMemo(
    () => buildShapeKeyStats(index.dataset.entries),
    [index.dataset.entries],
  );

  return (
    <div className="mx-auto flex w-full max-w-5xl flex-col gap-4 lg:flex-row lg:gap-6">
      <nav
        aria-label={t("nav.learn")}
        className="flex gap-2 overflow-x-auto lg:w-64 lg:flex-col lg:overflow-visible"
      >
        {LEARN_CHAPTERS.map((item, itemIndex) => (
          <button
            key={item.id}
            type="button"
            onClick={() => setChapterId(item.id)}
            aria-current={item.id === chapter.id ? "page" : undefined}
            className={cn(
              "flex min-h-11 shrink-0 flex-col items-start gap-0.5 rounded-lg px-3 py-2 text-start transition-colors focus-visible:outline-2 focus-visible:outline-ring",
              item.id === chapter.id
                ? "bg-primary/10 text-primary"
                : "text-muted-foreground hover:bg-accent hover:text-accent-foreground",
            )}
          >
            <span className="text-sm font-medium">
              {itemIndex + 1}. {item.title}
            </span>
            <span className="hidden text-xs text-muted-foreground lg:block">
              {LEVEL_LABELS[item.level]} · {item.summary}
            </span>
          </button>
        ))}
      </nav>

      <Card className="min-w-0 flex-1">
        <CardContent className="flex flex-col gap-5">
          <header className="flex flex-col gap-1.5">
            <div className="flex items-center gap-2">
              <Badge variant="secondary">{LEVEL_LABELS[chapter.level]}</Badge>
              <span className="text-xs text-muted-foreground">
                {t("learn.chapterOf", {
                  current: chapterIndex + 1,
                  total: LEARN_CHAPTERS.length,
                })}
              </span>
            </div>
            <h2 className="text-xl font-semibold tracking-tight">{chapter.title}</h2>
            <p className="text-sm text-muted-foreground">{chapter.summary}</p>
          </header>

          {chapter.sections.map((section, sectionIndex) => (
            <SectionView
              key={sectionIndex}
              section={section}
              t={t}
              shapeStats={shapeStats}
              onStartPractice={onStartPractice}
            />
          ))}

          <footer className="flex items-center justify-between border-t border-border pt-4">
            <Button
              variant="ghost"
              className="min-h-11"
              disabled={chapterIndex === 0}
              onClick={() => setChapterId(LEARN_CHAPTERS[chapterIndex - 1].id)}
            >
              <ArrowLeft aria-hidden />
              {t("learn.prevChapter")}
            </Button>
            <Button
              variant="ghost"
              className="min-h-11"
              disabled={chapterIndex === LEARN_CHAPTERS.length - 1}
              onClick={() => setChapterId(LEARN_CHAPTERS[chapterIndex + 1].id)}
            >
              {t("learn.nextChapter")}
              <ArrowRight aria-hidden />
            </Button>
          </footer>
        </CardContent>
      </Card>
    </div>
  );
}

function SectionView({
  section,
  t,
  shapeStats,
  onStartPractice,
}: {
  section: LearnSection;
  t: (key: I18nKey, params?: Record<string, string | number>) => string;
  shapeStats: ShapeKeyStat[];
  onStartPractice: (mode: PracticeMode) => void;
}) {
  switch (section.kind) {
    case "text":
      return (
        <section className="flex flex-col gap-2">
          {section.heading && (
            <h3 className="text-sm font-semibold">{section.heading}</h3>
          )}
          {section.paragraphs.map((paragraph, paragraphIndex) => (
            <p key={paragraphIndex} className="text-sm leading-relaxed">
              {paragraph}
            </p>
          ))}
        </section>
      );
    case "list":
      return (
        <section className="flex flex-col gap-2">
          <h3 className="text-sm font-semibold">{section.heading}</h3>
          {section.ordered ? (
            <ol className="list-decimal space-y-1.5 pl-5 text-sm leading-relaxed">
              {section.items.map((item, itemIndex) => (
                <li key={itemIndex}>{item}</li>
              ))}
            </ol>
          ) : (
            <ul className="list-disc space-y-1.5 pl-5 text-sm leading-relaxed">
              {section.items.map((item, itemIndex) => (
                <li key={itemIndex}>{item}</li>
              ))}
            </ul>
          )}
        </section>
      );
    case "practice":
      return (
        <section className="flex flex-col gap-2 rounded-lg bg-muted/50 p-4">
          <h3 className="text-sm font-semibold">{section.heading}</h3>
          <Button
            className="min-h-11 w-fit"
            onClick={() => onStartPractice(section.mode)}
          >
            <Play aria-hidden />
            {t(section.label)}
          </Button>
        </section>
      );
    case "shape-explorer":
      return (
        <section className="flex flex-col gap-3">
          <h3 className="text-sm font-semibold">{section.heading}</h3>
          <ShapeExplorer stats={shapeStats} />
        </section>
      );
  }
}

/** 形码探索器:点选形键,查看以它为首形/次形的高频例字(规范数据聚合)。 */
function ShapeExplorer({ stats }: { stats: ShapeKeyStat[] }) {
  const { t } = useI18n();
  const top = useMemo(() => topShapeKeys(stats), [stats]);
  const [selectedKey, setSelectedKey] = useState<string>(top[0]?.key ?? "");
  const selected = stats.find((stat) => stat.key === selectedKey) ?? top[0];

  if (!selected) return null;

  return (
    <div className="flex flex-col gap-3">
      <div className="flex flex-wrap gap-1.5" role="tablist" aria-label={t("learn.shapeKeysAria")}>
        {top.map((stat) => (
          <button
            key={stat.key}
            type="button"
            role="tab"
            aria-selected={stat.key === selected.key}
            onClick={() => setSelectedKey(stat.key)}
            className={cn(
              "flex min-h-11 min-w-11 flex-col items-center justify-center rounded-lg border px-2 font-mono text-sm font-semibold transition-colors focus-visible:outline-2 focus-visible:outline-ring",
              stat.key === selected.key
                ? "border-primary bg-primary/10 text-primary"
                : "border-border bg-card text-foreground hover:bg-accent",
            )}
          >
            {stat.key.toUpperCase()}
            <span className="text-[10px] font-normal text-muted-foreground">
              {t("learn.shapeCharCount", { n: stat.firstCount + stat.secondCount })}
            </span>
          </button>
        ))}
      </div>

      <div className="grid gap-3 sm:grid-cols-2">
        <SampleList
          title={t("learn.firstShapeSamples", { key: selected.key.toUpperCase() })}
          countLabel={t("learn.shapeCharCount", { n: selected.firstCount })}
          samples={selected.firstSamples}
        />
        <SampleList
          title={t("learn.secondShapeSamples", { key: selected.key.toUpperCase() })}
          countLabel={t("learn.shapeCharCount", { n: selected.secondCount })}
          samples={selected.secondSamples}
        />
      </div>
      <p className="text-xs text-muted-foreground">{t("learn.shapeExplorerNote")}</p>
    </div>
  );
}

function SampleList({
  title,
  countLabel,
  samples,
}: {
  title: string;
  countLabel: string;
  samples: readonly { char: string; code: string }[];
}) {
  return (
    <div className="rounded-lg border border-border p-3">
      <p className="mb-2 flex items-baseline justify-between text-xs font-medium">
        <span>{title}</span>
        <span className="text-muted-foreground">{countLabel}</span>
      </p>
      <ul className="grid grid-cols-4 gap-1.5 font-mono text-xs sm:grid-cols-4">
        {samples.map((sample) => (
          <li
            key={sample.code + sample.char}
            className="flex flex-col items-center rounded bg-muted/60 px-1 py-1"
          >
            <span className="text-base">{sample.char}</span>
            <span className="text-[10px] text-muted-foreground">{sample.code}</span>
          </li>
        ))}
      </ul>
    </div>
  );
}
