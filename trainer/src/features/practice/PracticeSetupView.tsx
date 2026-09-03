import { useState } from "react";
import { Play } from "lucide-react";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { OptionButton, OptionRow } from "@/components/OptionGroup";
import { cn } from "@/lib/utils";
import type { TrainingItem } from "@/lib/trainer-index";
import type { Difficulty } from "@/lib/trainer-index";
import { useI18n } from "@/lib/use-i18n";
import { useTrainerStore } from "@/stores/trainer-store";
import { PracticeView } from "./PracticeView";
import {
  DIFFICULTY_LABELS,
  MODE_DESCRIPTIONS,
  MODE_LABELS,
  SESSION_LENGTH_OPTIONS,
  type PracticeMode,
} from "./types";

export type PracticeConfig = {
  mode: PracticeMode;
  difficulty: Difficulty;
  targetLength: number;
  /** 复习会话的指定条目;为空则按模式+难度选题。 */
  entries?: TrainingItem[];
};

const MODES: PracticeMode[] = [
  "double",
  "sound-shape",
  "full",
  "mixed",
  "level1",
  "two-key-word",
  "zero-regression",
  "fixed-first",
  "fixed-word",
  "sentence",
  "mixed-shortcut",
  "mixed-all",
];
const DIFFICULTIES: Difficulty[] = ["beginner", "daily", "full"];

/** 模式 → 设置分组(组句/综合不需要难度截断语境,仍复用统一设置)。 */
function modeGroupKey(mode: PracticeMode): "chars" | "shortcuts" | "words" | "sentences" | "mixed" {
  switch (mode) {
    case "double":
    case "sound-shape":
    case "full":
    case "mixed":
      return "chars";
    case "level1":
    case "two-key-word":
    case "zero-regression":
    case "fixed-first":
    case "mixed-shortcut":
      return "shortcuts";
    case "fixed-word":
      return "words";
    case "sentence":
      return "sentences";
    default:
      return "mixed";
  }
}

/** 各分组的展示顺序。 */
const GROUP_ORDER = ["chars", "shortcuts", "words", "sentences", "mixed"] as const;

/** 模式右上角键数徽标。 */
function modeKeyBadge(mode: PracticeMode): string {
  switch (mode) {
    case "double":
      return "2";
    case "sound-shape":
      return "3";
    case "full":
      return "4";
    case "level1":
      return "1";
    case "two-key-word":
      return "2";
    case "fixed-word":
      return "4/6/8";
    case "sentence":
      return "≥16";
    default:
      return "—";
  }
}

export function PracticeSetupView({
  presetMode,
  reviewEntries,
  onPresetConsumed,
  onExitToToday,
}: {
  presetMode: PracticeMode | null;
  reviewEntries: TrainingItem[] | null;
  onPresetConsumed: () => void;
  onExitToToday: () => void;
}) {
  const { t, language } = useI18n();
  const lastMode = useTrainerStore((state) => state.lastMode);
  const difficulty = useTrainerStore((state) => state.difficulty);
  const sessionLength = useTrainerStore((state) => state.sessionLength);
  const setLastMode = useTrainerStore((state) => state.setLastMode);
  const setDifficulty = useTrainerStore((state) => state.setDifficulty);
  const setSessionLength = useTrainerStore((state) => state.setSessionLength);

  const [mode, setMode] = useState<PracticeMode>(presetMode ?? lastMode);
  const [active, setActive] = useState<{ config: PracticeConfig; seed: number } | null>(
    () =>
      reviewEntries
        ? {
            config: {
              mode: "mixed",
              difficulty,
              targetLength: sessionLength,
              entries: reviewEntries,
            },
            seed: 0,
          }
        : null,
  );

  if (active) {
    return (
      <PracticeView
        key={active.seed}
        config={active.config}
        onExit={() => {
          setActive(null);
          onPresetConsumed();
        }}
        onRestart={() =>
          setActive((current) =>
            current ? { config: current.config, seed: current.seed + 1 } : null,
          )
        }
        onPracticeEntries={(entries) =>
          setActive((current) =>
            current
              ? {
                  config: { ...current.config, mode: "mixed", entries },
                  seed: current.seed + 1,
                }
              : null,
          )
        }
        onExitToToday={onExitToToday}
      />
    );
  }

  const start = () => {
    setLastMode(mode);
    setActive({
      config: { mode, difficulty, targetLength: sessionLength },
      seed: 0,
    });
  };

  const grouped = new Map<string, PracticeMode[]>();
  for (const candidate of MODES) {
    const group = modeGroupKey(candidate);
    grouped.set(group, [...(grouped.get(group) ?? []), candidate]);
  }

  return (
    <div className="flex flex-col gap-4">
      <header>
        <h1 className="text-xl font-semibold">{t("practice.start")}</h1>
        <p className="mt-1 text-sm text-muted-foreground">
          {language === "zh"
            ? "选择模式后直接开始连续打字,全程无需鼠标。"
            : "Pick a mode and type continuously — no mouse needed."}
        </p>
      </header>

      {GROUP_ORDER.map((group) => {
        const candidates = grouped.get(group) ?? [];
        if (candidates.length === 0) return null;
        return (
          <section key={group} aria-label={t(`practice.group.${group}`)}>
            <h2 className="mb-2 px-1 text-sm font-medium text-muted-foreground">
              {t(`practice.group.${group}`)}
            </h2>
            <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
              {candidates.map((candidate) => (
                <Card
                  key={candidate}
                  role="button"
                  tabIndex={0}
                  aria-pressed={mode === candidate}
                  onClick={() => setMode(candidate)}
                  onKeyDown={(event) => {
                    if (event.key === "Enter" || event.key === " ") {
                      event.preventDefault();
                      setMode(candidate);
                    }
                  }}
                  className={cn(
                    "cursor-pointer transition-colors hover:border-primary/50 focus-visible:outline-2 focus-visible:outline-ring",
                    mode === candidate && "border-primary bg-primary/5",
                  )}
                >
                  <CardHeader>
                    <CardTitle className="flex items-center justify-between">
                      {MODE_LABELS[candidate]}
                      <span className="text-xs font-normal text-muted-foreground">
                        {modeKeyBadge(candidate)}
                      </span>
                    </CardTitle>
                    <CardDescription>{MODE_DESCRIPTIONS[candidate]}</CardDescription>
                  </CardHeader>
                </Card>
              ))}
            </div>
          </section>
        );
      })}

      <Card>
        <CardHeader>
          <CardTitle>{t("practice.mode")}</CardTitle>
        </CardHeader>
        <CardContent className="flex flex-col gap-4">
          <OptionRow label={t("practice.difficulty")}>
            {DIFFICULTIES.map((candidate) => (
              <OptionButton
                key={candidate}
                selected={difficulty === candidate}
                onClick={() => setDifficulty(candidate)}
              >
                {DIFFICULTY_LABELS[candidate]}
              </OptionButton>
            ))}
          </OptionRow>
          <OptionRow label={t("practice.length")}>
            {SESSION_LENGTH_OPTIONS.map((candidate) => (
              <OptionButton
                key={candidate}
                selected={sessionLength === candidate}
                onClick={() => setSessionLength(candidate)}
              >
                {candidate === 0
                  ? language === "zh"
                    ? "无限"
                    : "∞"
                  : candidate}
              </OptionButton>
            ))}
          </OptionRow>
          <Button size="lg" className="mt-2 w-full sm:w-auto" onClick={start}>
            <Play aria-hidden />
            {t("practice.start")}
          </Button>
        </CardContent>
      </Card>
    </div>
  );
}
