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
import { itemId, type TrainerEntry } from "@/lib/trainer-data";
import { selectPool } from "@/lib/trainer-index";
import { useTrainerIndex } from "@/lib/trainer-context";
import { accuracy, formatDuration, formatPercent, kpm } from "@/lib/stats";
import { listWeakItems } from "@/lib/review";
import { useTrainerStore } from "@/stores/trainer-store";
import {
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
  MODE_LABELS,
  MODE_LENGTHS,
} from "./types";

const FEEDBACK_MS = 150;

function dispatchEvents(events: SessionEvent[]): void {
  for (const event of events) {
    if (event.type === "question-completed") {
      useTrainerStore.getState().recordQuestionResult({
        id: itemId(event.entry),
        outcome: event.outcome,
        keystrokes: event.keystrokes,
        wrongKeyEvents: event.wrongKeyEvents,
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
  onPracticeEntries: (entries: TrainerEntry[]) => void;
  onExitToToday: () => void;
}) {
  const index = useTrainerIndex();
  const hintMode = useTrainerStore((state) => state.hintMode);
  const storeProgress = useTrainerStore((state) => state.progress);
  const inputRef = useRef<HTMLInputElement>(null);

  // 题池只在会话开始时构建一次;出题不再全量 filter/sort。
  const pools = useMemo<QuestionPool[]>(() => {
    if (config.entries) {
      const byLength = new Map<number, TrainerEntry[]>();
      for (const entry of config.entries) {
        const list = byLength.get(entry.length) ?? [];
        list.push(entry);
        byLength.set(entry.length, list);
      }
      return [...byLength.entries()].map(([length, entries]) =>
        buildPool(length as 2 | 3 | 4, entries),
      );
    }
    return MODE_LENGTHS[config.mode].map((length) =>
      buildPool(length, selectPool(index, length, config.difficulty)),
    );
  }, [config, index]);

  const [session, setSession] = useState<SessionState | null>(() =>
    createSession(
      {
        mode: config.entries ? "mixed" : config.mode,
        difficulty: config.difficulty,
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
        <p className="font-medium">暂无可练习的条目</p>
        <p className="mt-1 text-sm text-muted-foreground">
          当前模式与难度组合下题池为空,请更换设置。
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
          onPracticeEntries(weakItems.map((item) => item.entry))
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
  const codeHintVisible =
    hintMode === "always" ||
    (hintMode === "on-error" && session.hadError) ||
    session.phase === "feedback";
  const { targetLength } = session.config;

  return (
    <div className="mx-auto flex w-full max-w-2xl flex-col gap-3 sm:gap-4">
      <header className="flex items-center gap-2">
        <Button variant="ghost" size="sm" onClick={handleExit}>
          <ChevronLeft aria-hidden />
          退出
        </Button>
        <Badge variant="secondary">
          {config.entries ? "复习" : MODE_LABELS[session.config.mode]}
        </Badge>
        <Badge variant="outline">
          {DIFFICULTY_LABELS[session.config.difficulty]}
        </Badge>
        <div className="ml-auto flex items-center gap-2">
          <span className="text-sm tabular-nums text-muted-foreground">
            {targetLength > 0
              ? `${session.questionsCompleted} / ${targetLength}`
              : `已练 ${session.questionsCompleted} 题`}
          </span>
          <Button
            variant="ghost"
            size="icon"
            aria-label="暂停"
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
            key={itemId(session.current)}
            initial={{ opacity: 0, y: 8 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0 }}
            transition={{ duration: 0.15 }}
            className="flex flex-col items-center gap-3 sm:gap-4"
          >
            <div className="text-7xl font-medium leading-none tracking-tight sm:text-8xl">
              {session.current.char}
            </div>
            <div className="font-mono text-sm text-muted-foreground sm:text-base">
              {session.current.readings.join(" / ")}
            </div>
            <div
              aria-live="polite"
              className="flex h-6 items-center font-mono text-base tracking-[0.3em] text-primary"
            >
              {codeHintVisible ? session.current.code : ""}
            </div>
            <PracticeCode
              code={session.current.code}
              typed={session.typed}
              lastWrongKey={session.lastWrongKey}
              outcome={session.phase === "feedback" ? session.lastOutcome : null}
            />
          </motion.div>
        </AnimatePresence>

        {session.phase === "paused" && (
          <div className="absolute inset-0 z-10 flex flex-col items-center justify-center gap-4 rounded-xl bg-card/95 backdrop-blur-sm">
            <p className="text-lg font-semibold">已暂停</p>
            <div className="flex gap-2">
              <Button onClick={() => setSession(resume(session, Date.now()))}>
                继续练习
              </Button>
              <Button
                variant="outline"
                onClick={() => commit(finish(session, Date.now()))}
              >
                结束本次
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
        <StatChip label="连对" value={session.currentStreak} />
        <StatChip
          label="准确率"
          value={formatPercent(accuracy(session.keystrokes, session.wrongKeyEvents))}
        />
        <StatChip label="用时" value={formatDuration(displayedMs)} />
        <StatChip
          label="KPM"
          value={sessionKpm === null ? "—" : Math.round(sessionKpm)}
        />
      </div>
    </div>
  );
}
