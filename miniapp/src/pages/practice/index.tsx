/**
 * 练习页(小程序验证实现):用共享核心引擎跑一场 5 题的双拼码练习,
 * 逐键校验真实 XHUP 编码,并把每题的条目进度持久化到本机存储。
 */

import { View, Text } from "@tarojs/components";
import { useCallback, useRef, useState } from "react";
import {
  advance,
  applyImperfect,
  applyPerfect,
  buildPool,
  createSession,
  emptyProgress,
  selectPool,
  typeKey,
  type ItemProgress,
  type SessionState,
} from "@xhup/trainer-core";
import { trainerIndex } from "../../lib/dataset";
import { loadProgress, saveProgress } from "../../lib/storage";

const TARGET_QUESTIONS = 5;
const KEY_ROWS = ["qwertyuiop", "asdfghjkl", "zxcvbnm"] as const;

// 确定性 RNG:验证实现要求可复现,不引入随机依赖。
function makeRng() {
  let seed = 20260906;
  return () => {
    seed = (seed * 1103515245 + 12345) % 2147483648;
    return seed / 2147483648;
  };
}

function startSession(progress: Record<string, ItemProgress>): SessionState {
  const items = selectPool(trainerIndex, "char-2", "daily");
  const pool = buildPool("char-2", items);
  const state = createSession(
    { mode: "double", targetLength: TARGET_QUESTIONS, pools: [pool] },
    new Map(Object.entries(progress).map(([id, p]) => [id, p])),
    makeRng(),
    Date.now(),
  );
  if (!state) throw new Error("char-2 池意外为空");
  return state;
}

function recordProgress(state: SessionState): void {
  const progress = loadProgress();
  const id = state.current.id;
  const current = progress[id] ?? emptyProgress();
  const next =
    state.lastOutcome === "perfect"
      ? applyPerfect(current, Date.now(), state.activeMs)
      : applyImperfect(current, Date.now());
  progress[id] = next;
  saveProgress(progress);
}

export default function Practice() {
  const [session, setSession] = useState<SessionState | null>(null);
  const [completed, setCompleted] = useState(false);
  const progressRef = useRef<Record<string, ItemProgress> | null>(null);

  const start = useCallback(() => {
    const progress = loadProgress();
    progressRef.current = progress;
    setSession(startSession(progress));
    setCompleted(false);
  }, []);

  const press = useCallback(
    (key: string) => {
      setSession((prev) => {
        if (!prev || prev.phase !== "question") return prev;
        const rng = makeRng();
        const { state } = typeKey(prev, key, rng, Date.now());
        if (state.phase === "feedback") {
          recordProgress(state);
        }
        return state;
      });
    },
    [],
  );

  const nextQuestion = useCallback(() => {
    setSession((prev) => {
      if (!prev) return prev;
      const { state } = advance(prev, makeRng(), Date.now());
      if (state.phase === "completed") setCompleted(true);
      return state;
    });
  }, []);

  if (!session) {
    return (
      <View className="page">
        <View className="card">
          <Text className="title">双拼码练习</Text>
          <Text className="subtitle">
            从高频单字池选 5 题,逐键校验真实编码;完成后每题进度写入本机。
          </Text>
        </View>
        <View className="card button" onClick={start}>
          <Text>开始</Text>
        </View>
      </View>
    );
  }

  if (completed || session.phase === "completed") {
    return (
      <View className="page">
        <View className="card">
          <Text className="title">本场完成</Text>
          <Text className="subtitle">
            {session.perfect} 题全对 / {session.imperfect} 题有误 ·
            共 {session.keystrokes} 键
          </Text>
          <Text className="status-ok">本机进度已更新</Text>
        </View>
        <View className="card button" onClick={start}>
          <Text>再来一场</Text>
        </View>
      </View>
    );
  }

  return (
    <View className="page">
      <View className="card">
        <Text className="subtitle">
          第 {session.questionsCompleted + 1} / {TARGET_QUESTIONS} 题 · 双拼码
        </Text>
        <Text className="target">{session.current.target}</Text>
        <Text className={`typed ${session.lastWrongKey ? "typed-wrong" : ""}`}>
          {session.typed || "…"}
        </Text>
        {session.phase === "feedback" && (
          <Text className={session.lastOutcome === "perfect" ? "status-ok" : "subtitle"}>
            {session.lastOutcome === "perfect"
              ? "全对!"
              : `正确编码:${session.current.primaryCode}`}
          </Text>
        )}
      </View>

      {session.phase === "feedback" ? (
        <View className="card button" onClick={nextQuestion}>
          <Text>下一题</Text>
        </View>
      ) : (
        <View className="card keyboard">
          {KEY_ROWS.map((row) => (
            <View className="kb-row" key={row}>
              {row.split("").map((key) => (
                <View
                  key={key}
                  className={`kb-key ${session.lastWrongKey === key ? "kb-key-pressed" : ""}`}
                  onClick={() => press(key)}
                >
                  <Text>{key.toUpperCase()}</Text>
                </View>
              ))}
            </View>
          ))}
        </View>
      )}
    </View>
  );
}
