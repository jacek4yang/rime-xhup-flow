/**
 * 练习会话页:核心练习循环。大字目标 → 读音 → 编码槽位 → 教学键盘
 * → 实况统计;提示方式(HintMode)独立门控答案类信息,键帽参考
 * (KeyRefMode)只影响键帽标签;错误教学、暂停、退格与退出确认
 * 全部对齐桌面端 PracticeView 的语义,事件记账经共享核心纯函数落库。
 */

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { View, Text, ScrollView } from "@tarojs/components";
import Taro, { useDidHide } from "@tarojs/taro";
import {
  HINT_DELAY_MS,
  MODE_LABELS,
  MODE_POOL_ROTATION,
  activeCode,
  advance,
  backspace,
  buildPool,
  buildShapeKeyStats,
  createSession,
  expectedKey,
  finish,
  listWeakItems,
  pause,
  resume,
  retryCurrent,
  selectPool,
  typeKey,
  accuracy,
  cpm,
  formatDuration,
  formatPercent,
  kpm,
} from "@xhup/trainer-core";
import type {
  Difficulty,
  KeyRefMode,
  PracticeMode,
  QuestionPool,
  SessionState,
  StepResult,
  TrainingItem,
} from "@xhup/trainer-core";
import { trainerIndex } from "../../lib/dataset";
import { appStore, useAppState } from "../../lib/store";
import { buildKeyLabels } from "../../lib/keyboard-labels";
import { useI18n } from "../../lib/i18n";
import { haptic } from "../../lib/haptics-taro";
import { ROUTES, pageParam } from "../../lib/navigation";
import { setFinishedSession } from "../../lib/session-holder";
import { TeachingKeyboard } from "../../components/TeachingKeyboard";
import type { KeyboardRefMode, ShapeKeyRef } from "../../components/TeachingKeyboard";
import { CodeSlots } from "../../components/CodeSlots";
import { StatChip } from "../../components/StatChip";
import { TeachingCard, findEntry } from "../../components/TeachingCard";

/** 反馈展示时长(毫秒):触屏学习者需要读清正确编码,比桌面端稍长。 */
const FEEDBACK_MS = 1000;

/** 键帽参考的情境解析(与桌面端 resolvedRefMode 同表)。 */
function resolveRefMode(keyRefMode: KeyRefMode, mode: string): KeyboardRefMode {
  if (keyRefMode !== "contextual") return keyRefMode;
  switch (mode) {
    case "double":
    case "level1":
      return "double";
    case "sound-shape":
      return "shape";
    case "full":
    case "mixed":
    case "mixed-shortcut":
    case "mixed-all":
      return "both";
    case "fixed-word":
    case "two-key-word":
    case "zero-regression":
    case "fixed-first":
      return "double";
    case "sentence":
      return "none";
    default:
      return "double";
  }
}

export default function Session() {
  const state = useAppState();
  const { t } = useI18n();
  const [session, setSession] = useState<SessionState | null>(null);
  const sessionRef = useRef<SessionState | null>(null);
  const [exhausted, setExhausted] = useState(false);
  const weakItemsRef = useRef<TrainingItem[]>([]);
  const sessionMetaRef = useRef<{ mode: string; src: "normal" | "weak" }>({
    mode: "double",
    src: "normal",
  });

  // 键帽标签与形码参考:规范数据聚合,只建一次。
  const keyLabels = useMemo(() => buildKeyLabels(trainerIndex.dataset.doublePinyin), []);
  const shapeStats = useMemo(
    () => buildShapeKeyStats(trainerIndex.dataset.entries),
    [],
  );
  const shapeRef = useMemo<ShapeKeyRef>(() => {
    const map: ShapeKeyRef = new Map();
    for (const stat of shapeStats) {
      const first = stat.firstSamples[0]?.char;
      const second = stat.secondSamples[0]?.char;
      if (first ?? second) {
        map.set(stat.key, { first: first ?? "", second: second ?? "" });
      }
    }
    return map;
  }, [shapeStats]);

  // 统一提交:事件落库 + 状态推进 + 触感反馈。
  const apply = useCallback(
    (result: StepResult, previousLastWrong: string | null = null) => {
      for (const event of result.events) {
        if (event.type === "question-completed") {
          if (event.outcome === "imperfect") {
            weakItemsRef.current = [...weakItemsRef.current, event.item];
          }
          appStore.recordQuestionResult({
            id: event.item.id,
            outcome: event.outcome,
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
          appStore.addPracticeTime(event.practiceMs, Date.now());
        }
      }
      sessionRef.current = result.state;
      setSession(result.state);
      if (result.state.lastWrongKey && result.state.lastWrongKey !== previousLastWrong) {
        haptic.wrong();
      } else if (result.events.some((event) => event.type === "question-completed")) {
        haptic.success();
      } else {
        haptic.tap();
      }
    },
    [],
  );

  // 会话只建一次(页面实例内);池在开始时构建。
  useEffect(() => {
    if (sessionRef.current) return;
    const modeParam = pageParam("mode");
    const src = pageParam("src") === "weak" ? "weak" : "normal";
    const mode: PracticeMode =
      modeParam && modeParam in MODE_POOL_ROTATION
        ? (modeParam as PracticeMode)
        : state.settings.lastMode;
    sessionMetaRef.current = { mode, src };

    const settings = appStore.getState().settings;
    const pools = buildPools(src === "weak" ? null : mode, src, settings.difficulty);
    const created =
      pools.length === 0
        ? null
        : createSession(
            {
              mode: src === "weak" ? "mixed" : (mode as Parameters<typeof createSession>[0]["mode"]),
              targetLength: settings.sessionLength,
              pools,
            },
            new Map(Object.entries(appStore.getState().progress)),
            Math.random,
            Date.now(),
          );
    if (!created) {
      setExhausted(true);
      return;
    }
    sessionRef.current = created;
    setSession(created);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // 反馈后自动前进。
  useEffect(() => {
    if (!session || session.phase !== "feedback") return;
    const timer = setTimeout(() => {
      const current = sessionRef.current;
      if (current && current.phase === "feedback") {
        apply(advance(current, Math.random, Date.now()));
      }
    }, FEEDBACK_MS);
    return () => clearTimeout(timer);
  }, [session, apply]);

  // 实况计时:活跃阶段每秒刷新显示。
  const [, setTick] = useState(0);
  useEffect(() => {
    if (!session || (session.phase !== "question" && session.phase !== "feedback")) return;
    const timer = setInterval(() => setTick((n) => n + 1), 1000);
    return () => clearInterval(timer);
  }, [session?.phase]);

  // 切后台自动暂停:结清活跃时间,返回时停留在暂停浮层。
  // Taro 的 useDidHide 对应页面 onHide;微信在小程序整体切后台时也会
  // 触发当前页 onHide,因此无需再挂 Taro.onAppHide(也没有 offAppHide
  // 之类的反注册泄漏面)。后台时长不计入 KPM/activeMs:pause 已把
  // activeMs 结清到隐藏时刻,恢复(resume)重置 lastTickAt。
  useDidHide(() => {
    const current = sessionRef.current;
    if (current && current.phase === "question") {
      apply(pause(current, Date.now()), current.lastWrongKey);
    }
  });

  const handleLetter = useCallback(
    (key: string) => {
      const prev = sessionRef.current;
      if (!prev || prev.phase !== "question") return;
      apply(typeKey(prev, key, Math.random, Date.now()), prev.lastWrongKey);
    },
    [apply],
  );

  const handleBackspace = useCallback(() => {
    const prev = sessionRef.current;
    if (!prev || prev.phase !== "question") return;
    const next = backspace(prev);
    sessionRef.current = next;
    setSession(next);
    haptic.tap();
  }, []);

  const retryQuestion = useCallback(() => {
    const prev = sessionRef.current;
    if (!prev) return;
    const next = retryCurrent(prev);
    sessionRef.current = next;
    setSession(next);
  }, []);

  const finishToSummary = useCallback(() => {
    const current = sessionRef.current;
    if (!current) return;
    const result = finish(current, Date.now());
    for (const event of result.events) {
      if (event.type === "time-flushed") {
        appStore.addPracticeTime(event.practiceMs, Date.now());
      }
    }
    setFinishedSession({
      state: result.state,
      mode: sessionMetaRef.current.mode,
      src: sessionMetaRef.current.src,
      weakItems: weakItemsRef.current,
    });
    Taro.redirectTo({ url: ROUTES.summary });
  }, []);

  const confirmExit = useCallback(() => {
    const current = sessionRef.current;
    if (!current) return;
    Taro.showModal({
      title: t("common.exit"),
      content:
        current.questionsCompleted > 0
          ? "本场已有成绩,退出将计入小结。确认退出?"
          : "还没有完成任何题目,确认退出?",
      confirmText: t("common.exit"),
      cancelText: t("common.cancel"),
      success: (res) => {
        if (!res.confirm) return;
        if (current.questionsCompleted > 0) {
          finishToSummary();
        } else {
          Taro.navigateBack({
            fail: () => Taro.redirectTo({ url: ROUTES.practice }),
          });
        }
      },
    });
  }, [t, finishToSummary]);

  // 会话自然结束 → 小结(redirectTo 替换会话页,避免返回栈残留已结束的会话)。
  useEffect(() => {
    if (!session || session.phase !== "completed") return;
    setFinishedSession({
      state: session,
      mode: sessionMetaRef.current.mode,
      src: sessionMetaRef.current.src,
      weakItems: weakItemsRef.current,
    });
    Taro.redirectTo({ url: ROUTES.summary });
  }, [session?.phase]);

  if (exhausted) {
    return (
      <View className="page">
        <View className="card">
          <Text className="title">{t("practice.noItems")}</Text>
          <Text className="subtitle">{t("practice.emptyPool")}</Text>
        </View>
        <View className="button" onClick={() => Taro.navigateBack()}>
          <Text>{t("common.back")}</Text>
        </View>
      </View>
    );
  }
  if (!session || session.phase === "completed") {
    // 初始化中 / redirectTo 已发起:空视图避免闪烁。
    return <View />;
  }

  const displayedMs =
    session.phase === "paused"
      ? session.activeMs
      : session.activeMs + (Date.now() - session.lastTickAt);
  const sessionKpm = kpm(session.keystrokes, displayedMs);
  const sessionCpm = cpm(session.charsCompleted, displayedMs);
  const hintVisible =
    state.settings.hintMode === "always" ||
    (state.settings.hintMode === "on-delay" && displayedMs >= HINT_DELAY_MS) ||
    (state.settings.hintMode === "on-error" && session.hadError) ||
    session.phase === "feedback";
  const { targetLength } = session.config;
  const refMode = resolveRefMode(state.settings.keyRefMode, session.config.mode);
  const teachingVisible =
    state.settings.errorTeaching !== "quick" &&
    session.phase === "question" &&
    session.lastWrongKey !== null;

  return (
    // 定高滚动容器(app.css .session-scroll):短屏设备上键盘之上
    // 的内容放不下时页内滚动,统计芯片不会被完全顶出视口。
    <ScrollView scrollY className="session-scroll">
      <View className="session-page">
        <View className="session-header">
          <View className="session-header-btn" hoverClass="action-row-hover" onClick={confirmExit}>
            <Text className="session-header-text">‹ {t("common.exit")}</Text>
          </View>
          <Text className="session-mode">{t(MODE_LABELS[session.config.mode])}</Text>
          <Text className="session-progress">
            {targetLength > 0
              ? t("practice.progressOf", { done: session.questionsCompleted, total: targetLength })
              : t("practice.progressUnlimited", { n: session.questionsCompleted })}
          </Text>
          <View
            className="session-header-btn"
            hoverClass="action-row-hover"
            onClick={() => {
              const current = sessionRef.current;
              if (!current) return;
              if (current.phase === "paused") {
                const next = resume(current, Date.now());
                sessionRef.current = next;
                setSession(next);
              } else if (current.phase === "question") {
                apply(pause(current, Date.now()), current.lastWrongKey);
              }
            }}
          >
            <Text className="session-header-text">
              {session.phase === "paused" ? t("common.resume") : t("common.pause")}
            </Text>
          </View>
        </View>

        <View className="session-card">
          <Text className="session-target">{session.current.target}</Text>
          <Text className="session-reading">
            {session.current.kind === "sentence" && hintVisible
              ? (session.current.components ?? []).join(" · ")
              : session.current.readings.join(" / ")}
          </Text>
          {hintVisible && (
            <Text className="session-code-hint">{session.current.primaryCode}</Text>
          )}
          <CodeSlots
            code={activeCode(session)}
            typed={session.typed}
            lastWrongKey={session.lastWrongKey}
            outcome={session.phase === "feedback" ? session.lastOutcome : null}
          />
          {session.phase === "feedback" && (
            <Text
              className={`session-feedback ${
                session.lastOutcome === "perfect"
                  ? "session-feedback-ok"
                  : "session-feedback-warn"
              }`}
            >
              {session.lastOutcome === "perfect"
                ? "✓ 全对"
                : `正确编码:${session.current.primaryCode}`}
            </Text>
          )}

          {session.phase === "paused" && (
            <View className="pause-overlay">
              <Text className="pause-title">{t("common.paused")}</Text>
              <View
                className="button"
                onClick={() => {
                  const current = sessionRef.current;
                  if (!current) return;
                  const next = resume(current, Date.now());
                  sessionRef.current = next;
                  setSession(next);
                }}
              >
                <Text>{t("common.resume")}</Text>
              </View>
              <View className="button button-secondary" onClick={finishToSummary}>
                <Text>{t("common.finish")}</Text>
              </View>
            </View>
          )}

          {teachingVisible && session.lastWrongKey !== null && (
            <TeachingCard
              item={session.current}
              entry={findEntry(trainerIndex.dataset.entries, session.current.target)}
              wrongKey={session.lastWrongKey}
              position={session.typed.length}
              mode={state.settings.errorTeaching}
              shapeStats={shapeStats}
              onRetry={retryQuestion}
            />
          )}
        </View>

        <TeachingKeyboard
          labels={keyLabels}
          shapeRef={shapeRef}
          refMode={refMode}
          nextKey={hintVisible ? expectedKey(session) : null}
          wrongKey={session.lastWrongKey}
          onKeyPress={handleLetter}
          showBackspace
          onBackspace={handleBackspace}
        />

        <View className="stat-grid session-stats">
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
                value={sessionCpm === null ? "—" : Math.round(sessionCpm)}
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
        </View>
      </View>
    </ScrollView>
  );
}

/**
 * 构建题池:normal = 模式轮换池;weak = 错题条目按码长重组
 * (对齐桌面端 config.entries 路径)。池全空返回 []。
 */
function buildPools(
  mode: PracticeMode | null,
  src: "normal" | "weak",
  difficulty: Difficulty,
): QuestionPool[] {
  if (src === "weak") {
    const weak = listWeakItems(trainerIndex, appStore.getState().progress, 30);
    const items = weak.map((entry) => entry.item);
    if (items.length === 0) return [];
    const byLength = new Map<number, TrainingItem[]>();
    for (const item of items) {
      const list = byLength.get(item.codeLength) ?? [];
      list.push(item);
      byLength.set(item.codeLength, list);
    }
    return [...byLength.entries()].map(([length, group]) =>
      buildPool(`char-${length}`, group),
    );
  }
  return MODE_POOL_ROTATION[mode ?? "double"]
    .map((poolId) => buildPool(poolId, selectPool(trainerIndex, poolId, difficulty)))
    .filter((pool) => pool.items.length > 0);
}