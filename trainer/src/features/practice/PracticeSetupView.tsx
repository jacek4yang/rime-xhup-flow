import { useEffect, useState } from "react";
import { ChevronLeft, Play } from "lucide-react";
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
import { registerBackHandler } from "@/lib/back-handler";
import { useI18n } from "@/lib/use-i18n";
import { useTrainerStore } from "@/stores/trainer-store";
import { PracticeView } from "./PracticeView";
import {
  DIFFICULTY_LABELS,
  HAPTICS_MODES,
  KEY_REF_MODES,
  MODE_DESCRIPTIONS,
  MODE_LABELS,
  SESSION_LENGTH_OPTIONS,
  type HapticsMode,
  type KeyRefMode,
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

/**
 * 练习入口:模式选择(PracticeHome)与参数设置(Setup)是两块独立屏幕;
 * 会话(PracticeView)整屏替换,不再同页下滚。返回键由各屏自行认领。
 */
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
  const { t } = useI18n();
  const lastMode = useTrainerStore((state) => state.lastMode);
  const difficulty = useTrainerStore((state) => state.difficulty);
  const sessionLength = useTrainerStore((state) => state.sessionLength);
  const setLastMode = useTrainerStore((state) => state.setLastMode);
  const setDifficulty = useTrainerStore((state) => state.setDifficulty);
  const setSessionLength = useTrainerStore((state) => state.setSessionLength);
  const keyRefMode = useTrainerStore((state) => state.keyRefMode);
  const setKeyRefMode = useTrainerStore((state) => state.setKeyRefMode);
  const keyHaptics = useTrainerStore((state) => state.keyHaptics);
  const setKeyHaptics = useTrainerStore((state) => state.setKeyHaptics);

  // 带预设模式(今日页模式卡/学习中心)直接进入设置屏;复习条目直接进入会话。
  const [screen, setScreen] = useState<"home" | "setup">(presetMode ? "setup" : "home");
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

  // 设置屏:返回键回到模式选择屏(会话屏由 PracticeView 自行认领;
  // 激活时不得清除子组件的注册)。
  useEffect(() => {
    if (active) return;
    if (screen !== "setup") {
      registerBackHandler(null);
      return;
    }
    registerBackHandler(() => {
      setScreen("home");
      return true;
    });
    return () => registerBackHandler(null);
  }, [screen, active]);

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

  if (screen === "setup") {
    return (
      <div className="mx-auto flex w-full max-w-2xl flex-col gap-4">
        <header className="flex items-center gap-1">
          <Button
            variant="ghost"
            size="icon"
            aria-label={t("common.back")}
            onClick={() => setScreen("home")}
          >
            <ChevronLeft aria-hidden />
          </Button>
          <div className="min-w-0">
            <h1 className="truncate text-xl font-semibold">
              {t(MODE_LABELS[mode])}
            </h1>
            <p className="truncate text-sm text-muted-foreground">
              {t(MODE_DESCRIPTIONS[mode])}
            </p>
          </div>
        </header>

        <Card>
          <CardContent className="flex flex-col gap-4 pt-4">
            <OptionRow label={t("practice.difficulty")}>
              {DIFFICULTIES.map((candidate) => (
                <OptionButton
                  key={candidate}
                  selected={difficulty === candidate}
                  onClick={() => setDifficulty(candidate)}
                >
                  {t(DIFFICULTY_LABELS[candidate])}
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
                  {candidate === 0 ? t("practice.lengthUnlimited") : candidate}
                </OptionButton>
              ))}
            </OptionRow>
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle className="text-base">{t("practice.keyboardTitle")}</CardTitle>
            <CardDescription>{t("practice.keyboardHint")}</CardDescription>
          </CardHeader>
          <CardContent className="flex flex-col gap-4">
            <OptionRow label={t("practice.keyRefMode")}>
              {KEY_REF_MODES.map((candidate) => (
                <OptionButton
                  key={candidate}
                  selected={keyRefMode === candidate}
                  onClick={() => setKeyRefMode(candidate as KeyRefMode)}
                >
                  {t(`practice.keyRef.${candidate}`)}
                </OptionButton>
              ))}
            </OptionRow>
            <OptionRow label={t("practice.haptics")}>
              {HAPTICS_MODES.map((candidate) => (
                <OptionButton
                  key={candidate}
                  selected={keyHaptics === candidate}
                  onClick={() => setKeyHaptics(candidate as HapticsMode)}
                >
                  {t(`practice.haptics.${candidate}`)}
                </OptionButton>
              ))}
            </OptionRow>
          </CardContent>
        </Card>

        {/* 吸底开始按钮:设置项再多也无需滚动寻找。 */}
        <div className="sticky bottom-[calc(env(safe-area-inset-bottom)+1rem)] z-10">
          <Button size="lg" className="w-full shadow-lg" onClick={start}>
            <Play aria-hidden />
            {t("practice.start")}
          </Button>
        </div>
      </div>
    );
  }

  const grouped = new Map<string, PracticeMode[]>();
  for (const candidate of MODES) {
    const group = modeGroupKey(candidate);
    grouped.set(group, [...(grouped.get(group) ?? []), candidate]);
  }

  return (
    <div className="flex flex-col gap-4">
      <header>
        <h1 className="text-xl font-semibold">{t("practice.chooseMode")}</h1>
        <p className="mt-1 text-sm text-muted-foreground">
          {t("practice.setupHint")}
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
                  onClick={() => {
                    setMode(candidate);
                    setScreen("setup");
                  }}
                  onKeyDown={(event) => {
                    if (event.key === "Enter" || event.key === " ") {
                      event.preventDefault();
                      setMode(candidate);
                      setScreen("setup");
                    }
                  }}
                  className={cn(
                    "cursor-pointer transition-colors hover:border-primary/50 focus-visible:outline-2 focus-visible:outline-ring",
                    mode === candidate && "border-primary bg-primary/5",
                  )}
                >
                  <CardHeader>
                    <CardTitle className="flex items-center justify-between">
                      {t(MODE_LABELS[candidate])}
                      <span className="text-xs font-normal text-muted-foreground">
                        {modeKeyBadge(candidate)}
                      </span>
                    </CardTitle>
                    <CardDescription>{t(MODE_DESCRIPTIONS[candidate])}</CardDescription>
                  </CardHeader>
                </Card>
              ))}
            </div>
          </section>
        );
      })}
    </div>
  );
}
