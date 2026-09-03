/**
 * 练习主界面:大字目标字 → 读音/提示 → 码位格 → 键盘 → 实况统计。
 *
 * 输入走一个覆盖练习区的透明 input(支持实体键盘与手机原生键盘),
 * 点按屏幕键盘是同一条输入路径;Escape 暂停,答对自动前进,无需鼠标。
 */

import { useEffect, useMemo, useReducer, useRef, useState } from "react";
import { AnimatePresence, motion } from "motion/react";
import { ChevronLeft, Pause } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { Progress } from "@/components/ui/progress";
import { StatChip } from "@/components/StatChip";
import { PracticeCode } from "@/components/PracticeCode";
import { OnScreenKeyboard } from "@/components/OnScreenKeyboard";
import type { TrainingItem } from "@/lib/trainer-index";
import { selectPool } from "@/lib/trainer-index";
import { useTrainerIndex } from "@/lib/trainer-context";
import { accuracy, formatDuration, formatPercent, kpm, cpm } from "@/lib/stats";
import { listWeakItems } from "@/lib/review";
import { useI18n } from "@/lib/use-i18n";
import { useTrainerStore } from "@/stores/trainer-store";
import {
  activeCode,
  advance,
  backspace,
  createSession,
  expectedKey,
  finish,
  pause,
  resume,
  typeKey,
  type SessionEvent,
  type SessionState,
  type StepResult,
} from "./engine";
import { buildPool, type QuestionPool } from "./scheduler";
import { SessionSummary } from "./SessionSummary";
import type { PracticeConfig } from "./PracticeSetupView";
import {
  DIFFICULTY_LABELS,
  HINT_DELAY_MS,
  MODE_LABELS,
  MODE_POOL_ROTATION,
} from "./types";

const FEEDBACK_MS = 150;

function dispatchEvents(events: SessionEvent[]): void {
  for (const event of events) {
    if (event.type === "question-completed") {
      useTrainerStore.getState().recordQuestionResult({
        id: event.item.id,
        outcome: event.outcome,
        routeUsed: event.routeUsed,
        keystrokes: event.keystrokes,
        wrongKeyEvents: event.wrongKeyEvents,
        wrongKeys: event.wrongKeys,
        chars: event.item.charCount,
        corrections: event.corrections,
        practiceMs: event.practiceMs,
        bestStreak: event.bestStreak,
        now: Date.now(),
      });
    } else if (event.type === "time-flushed") {
      useTrainerStore.getState().addPracticeTime(event.practiceMs, Date.now());
    }
  }
}

export function PracticeView({
  config,
  onExit,
  onRestart,
  onPracticeEntries,
  onExitToToday,
}: {
  config: PracticeConfig;
  onExit: () => void;
  onRestart: () => void;
  onPracticeEntries: (entries: TrainingItem[]) => void;
  onExitToToday: () => void;
}) {
  const index = useTrainerIndex();
  const { t } = useI18n();
  const hintMode = useTrainerStore((state) => state.hintMode);
  const storeProgress = useTrainerStore((state) => state.progress);
  const inputRef = useRef<HTMLInputElement>(null);

  // 题池只在会话开始时构建一次;出题不再全量 filter/sort。
  const pools = useMemo<QuestionPool[]>(() => {
    if (config.entries) {
      const byLength = new Map<number, TrainingItem[]>();
      for (const item of config.entries) {
        const list = byLength.get(item.codeLength) ?? [];
        list.push(item);
        byLength.set(item.codeLength, list);
      }
      return [...byLength.entries()].map(([length, items]) =>
        buildPool(`char-${length}`, items),
      );
    }
    return MODE_POOL_ROTATION[config.mode]
      .map((poolId) => buildPool(poolId, selectPool(index, poolId, config.difficulty)))
      .filter((pool) => pool.items.length > 0);
  }, [config, index]);

  const [session, setSession] = useState<SessionState | null>(() =>
    createSession(
      {
        mode: config.entries ? "mixed" : config.mode,
        targetLength: config.targetLength,
        pools,
      },
      new Map(Object.entries(useTrainerStore.getState().progress)),
      Math.random,
      Date.now(),
    ),
  );

  const commit = (result: StepResult) => {
    dispatchEvents(result.events);
    setSession(result.state);
  };

  // 答对/答错补全后的短暂反馈,然后自动前进。
  useEffect(() => {
    if (!session || session.phase !== "feedback") return;
    const current = session;
    const timer = setTimeout(
      () => commit(advance(current, Math.random, Date.now())),
      FEEDBACK_MS,
    );
    return () => clearTimeout(timer);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [session]);

  // 进入出题阶段时确保输入焦点(手机点按练习区也会聚焦)。
  useEffect(() => {
    if (session?.phase === "question") inputRef.current?.focus();
  }, [session?.phase, session?.current]);

  // 实况计时:活跃阶段每秒刷新一次显示。
  const [, tick] = useReducer((count: number) => count + 1, 0);
  useEffect(() => {
    if (!session || (session.phase !== "question" && session.phase !== "feedback")) {
      return;
    }
    const timer = setInterval(tick, 1000);
    return () => clearInterval(timer);
  }, [session?.phase, session]);

  const weakItems = useMemo(
    () => listWeakItems(index, storeProgress, 5),
    [index, storeProgress],
  );

  if (!session) {
    return (
      <Card className="mx-auto max-w-md p-6 text-center">
        <p className="font-medium">{t("practice.noItems")}</p>
        <p className="mt-1 text-sm text-muted-foreground">
          {t("practice.emptyPool")}
        </p>
        <Button className="mt-4" onClick={onExit}>
          返回
        </Button>
      </Card>
    );
  }

  if (session.phase === "completed") {
    return (
      <SessionSummary
        session={session}
        weakItems={weakItems}
        onRestart={onRestart}
        onPracticeWeak={() =>
          onPracticeEntries(weakItems.map((weak) => weak.item))
        }
        onExitToToday={onExitToToday}
      />
    );
  }

  const handleLetter = (key: string) => {
    commit(typeKey(session, key, Math.random, Date.now()));
    inputRef.current?.focus();
  };

  const handleKeyDown = (event: React.KeyboardEvent<HTMLInputElement>) => {
    if (event.nativeEvent.isComposing) return;
    if (event.ctrlKey || event.altKey || event.metaKey) return;
    const key = event.key;
    if (/^[a-zA-Z]$/.test(key)) {
      event.preventDefault();
      handleLetter(key.toLowerCase());
    } else if (key === "Backspace") {
      event.preventDefault();
      setSession(backspace(session));
    } else if (key === "Escape") {
      event.preventDefault();
      if (session.phase === "paused") {
        setSession(resume(session, Date.now()));
      } else {
        commit(pause(session, Date.now()));
      }
    }
  };

  const handleExit = () => {
    dispatchEvents(finish(session, Date.now()).events);
    onExit();
  };

  const displayedMs =
    session.phase === "paused"
      ? session.activeMs
      : session.activeMs + (Date.now() - session.lastTickAt);
  const sessionKpm = kpm(session.keystrokes, displayedMs);
  const sessionCpm = cpm(session.charsCompleted, displayedMs);
  const activeNow =
    session.phase === "paused"
      ? session.activeMs
      : session.activeMs + (Date.now() - session.lastTickAt);
  const codeHintVisible =
    hintMode === "always" ||
    (hintMode === "on-delay" && activeNow >= HINT_DELAY_MS) ||
    (hintMode === "on-error" && session.hadError) ||
    session.phase === "feedback";
  const { targetLength } = session.config;

  return (
    <div className="mx-auto flex w-full max-w-2xl flex-col gap-3 sm:gap-4">
      <header className="flex items-center gap-2">
        <Button variant="ghost" size="sm" onClick={handleExit}>
          <ChevronLeft aria-hidden />
          {t("common.exit")}
        </Button>
        <Badge variant="secondary">
          {config.entries ? t("practice.reviewBadge") : MODE_LABELS[session.config.mode]}
        </Badge>
        <Badge variant="outline">{DIFFICULTY_LABELS[config.difficulty]}</Badge>
        <div className="ml-auto flex items-center gap-2">
          <span className="text-sm tabular-nums text-muted-foreground">
            {targetLength > 0
              ? `${session.questionsCompleted} / ${targetLength}`
              : `已练 ${session.questionsCompleted} 题`}
          </span>
          <Button
            variant="ghost"
            size="icon"
            aria-label={t("common.pause")}
            onClick={() => commit(pause(session, Date.now()))}
          >
            <Pause aria-hidden />
          </Button>
        </div>
      </header>
      {targetLength > 0 && (
        <Progress value={session.questionsCompleted / targetLength} />
      )}

      <Card
        className="relative flex flex-col items-center gap-4 px-4 py-6 sm:gap-5 sm:py-8"
        onClick={() => inputRef.current?.focus()}
      >
        <input
          ref={inputRef}
          value=""
          onChange={() => {}}
          onKeyDown={handleKeyDown}
          autoCapitalize="none"
          autoComplete="off"
          autoCorrect="off"
          spellCheck={false}
          aria-label="编码输入区"
          className="absolute inset-0 h-full w-full cursor-text opacity-0"
        />
        <AnimatePresence mode="wait">
          <motion.div
            key={session.current.id}
            initial={{ opacity: 0, y: 8 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0 }}
            transition={{ duration: 0.15 }}
            className="flex flex-col items-center gap-3 sm:gap-4"
          >
            <div
              className={
                session.current.charCount > 4
                  ? "text-3xl font-medium leading-snug tracking-tight sm:text-4xl"
                  : session.current.charCount > 1
                    ? "text-5xl font-medium leading-none tracking-tight sm:text-6xl"
                    : "text-7xl font-medium leading-none tracking-tight sm:text-8xl"
              }
            >
              {session.current.target}
            </div>
            <div className="font-mono text-sm text-muted-foreground sm:text-base">
              {session.current.kind === "sentence" && codeHintVisible
                ? session.current.components?.join(" · ")
                : session.current.readings.join(" / ")}
            </div>
            <div
              aria-live="polite"
              className="flex h-6 items-center font-mono text-base tracking-[0.3em] text-primary"
            >
              {codeHintVisible ? session.current.primaryCode : ""}
            </div>
            <PracticeCode
              code={activeCode(session)}
              typed={session.typed}
              lastWrongKey={session.lastWrongKey}
              outcome={session.phase === "feedback" ? session.lastOutcome : null}
            />
          </motion.div>
        </AnimatePresence>

        {session.phase === "paused" && (
          <div className="absolute inset-0 z-10 flex flex-col items-center justify-center gap-4 rounded-xl bg-card/95 backdrop-blur-sm">
            <p className="text-lg font-semibold">{t("common.paused")}</p>
            <div className="flex gap-2">
              <Button onClick={() => setSession(resume(session, Date.now()))}>
                {t("common.resume")}
              </Button>
              <Button
                variant="outline"
                onClick={() => commit(finish(session, Date.now()))}
              >
                {t("common.finish")}
              </Button>
            </div>
          </div>
        )}
      </Card>

      <OnScreenKeyboard
        reference={index.dataset.doublePinyin}
        nextKey={expectedKey(session)}
        wrongKey={session.lastWrongKey}
        onKeyPress={handleLetter}
        compact
      />

      <div className="grid grid-cols-4 gap-2">
        <StatChip label={t("common.streak")} value={session.currentStreak} />
        <StatChip
          label={t("common.accuracy")}
          value={formatPercent(accuracy(session.keystrokes, session.wrongKeyEvents))}
        />
        <StatChip label={t("common.elapsed")} value={formatDuration(displayedMs)} />
        <StatChip
          label={t("common.kpm")}
          value={sessionKpm === null ? "—" : Math.round(sessionKpm)}
        />
        {session.charsCompleted > 0 && (
          <>
            <StatChip label={t("common.chars")} value={session.charsCompleted} />
            <StatChip
              label={t("common.cpm")}
              value={
                sessionCpm === null ? "—" : Math.round(sessionCpm)
              }
            />
            <StatChip
              label={t("common.keysPerChar")}
              value={
                session.charsCompleted === 0
                  ? "—"
                  : (session.keystrokes / session.charsCompleted).toFixed(1)
              }
            />
          </>
        )}
      </div>
    </div>
  );
}
