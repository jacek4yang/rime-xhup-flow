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
import type { TrainerEntry } from "@/lib/trainer-data";
import type { Difficulty } from "@/lib/trainer-index";
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
  entries?: TrainerEntry[];
};

const MODES: PracticeMode[] = ["double", "sound-shape", "full", "mixed"];
const DIFFICULTIES: Difficulty[] = ["beginner", "daily", "full"];

export function PracticeSetupView({
  presetMode,
  reviewEntries,
  onPresetConsumed,
  onExitToToday,
}: {
  presetMode: PracticeMode | null;
  reviewEntries: TrainerEntry[] | null;
  onPresetConsumed: () => void;
  onExitToToday: () => void;
}) {
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

  return (
    <div className="flex flex-col gap-4">
      <header>
        <h1 className="text-xl font-semibold">开始练习</h1>
        <p className="mt-1 text-sm text-muted-foreground">
          选择模式后直接开始连续打字,全程无需鼠标。
        </p>
      </header>

      <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
        {MODES.map((candidate) => (
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
                  {candidate === "mixed" ? "2/3/4 键" : `${candidate === "double" ? 2 : candidate === "sound-shape" ? 3 : 4} 键`}
                </span>
              </CardTitle>
              <CardDescription>{MODE_DESCRIPTIONS[candidate]}</CardDescription>
            </CardHeader>
          </Card>
        ))}
      </div>

      <Card>
        <CardHeader>
          <CardTitle>练习设置</CardTitle>
        </CardHeader>
        <CardContent className="flex flex-col gap-4">
          <OptionRow label="难度">
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
          <OptionRow label="题数">
            {SESSION_LENGTH_OPTIONS.map((candidate) => (
              <OptionButton
                key={candidate}
                selected={sessionLength === candidate}
                onClick={() => setSessionLength(candidate)}
              >
                {candidate === 0 ? "无限" : candidate}
              </OptionButton>
            ))}
          </OptionRow>
          <Button size="lg" className="mt-2 w-full sm:w-auto" onClick={start}>
            <Play aria-hidden />
            开始练习
          </Button>
        </CardContent>
      </Card>
    </div>
  );
}
