/**
 * 小程序验证套件:证明共享核心 + 生成数据分片在小程序语境下完整工作。
 *
 * 覆盖:#41 验证清单 —— 导入真实共享域函数、加载真实规范数据分片、
 * 渲染一节真实课程所需的数据、跑一场 5 题练习并校验真实 XHUP 编码、
 * 经 StorageAdapter 契约持久化一条进度记录。
 */

import { describe, expect, it } from "vitest";
import {
  advance,
  buildPool,
  buildTrainerIndex,
  createSession,
  emptyProgress,
  LEARN_CHAPTERS,
  selectPool,
  typeKey,
  validateTrainerDataset,
  type ItemProgress,
  type StorageAdapter,
} from "@xhup/trainer-core";
import shard from "./data/generated/dataset.json";

describe("miniapp 数据分片", () => {
  it("通过共享核心的完整运行时校验", () => {
    const dataset = validateTrainerDataset(shard);
    expect(dataset.schemaVersion).toBe(4);
    expect(dataset.level1Shortcuts.length).toBe(26);
    expect(dataset.entries.length).toBeGreaterThanOrEqual(26);
  });

  it("与桌面端语义契约一致:编码只含小写字母且长度匹配 kind", () => {
    const dataset = validateTrainerDataset(shard);
    for (const entry of dataset.entries) {
      expect(entry.code).toMatch(/^[a-z]+$/);
      expect(entry.code.length).toBe(entry.length);
      expect(entry.char.length).toBe(1);
    }
    for (const word of dataset.words) {
      expect(word.code).toMatch(/^[a-z]+$/);
      expect(word.charCount).toBe(word.word.length);
    }
  });
});

describe("miniapp 课程与练习(共享核心)", () => {
  it("提供可渲染的真实课程章节", () => {
    expect(LEARN_CHAPTERS.length).toBe(9);
    const first = LEARN_CHAPTERS[0];
    expect(first.title.length).toBeGreaterThan(0);
    expect(first.sections.length).toBeGreaterThan(0);
  });

  it("用真实编码跑完 5 题练习并经 StorageAdapter 持久化进度", () => {
    const dataset = validateTrainerDataset(shard);
    const index = buildTrainerIndex(dataset);
    const items = selectPool(index, "char-2", "daily");
    expect(items.length).toBeGreaterThan(0);

    // 内存实现 StorageAdapter(与真机 Taro 实现同一契约)。
    const memory = new Map<string, string>();
    const storage = {
      get: (key) => memory.get(key) ?? null,
      set: (key, value) => void memory.set(key, value),
      remove: (key) => void memory.delete(key),
    } satisfies StorageAdapter;
    const PROGRESS_KEY = "xhup.miniapp.progress.v1";
    const load = (): Record<string, ItemProgress> => {
      const raw = storage.get(PROGRESS_KEY);
      return raw ? (JSON.parse(raw) as Record<string, ItemProgress>) : {};
    };
    const save = (p: Record<string, ItemProgress>) =>
      storage.set(PROGRESS_KEY, JSON.stringify(p));

    let seed = 20260906;
    const rng = () => {
      seed = (seed * 1103515245 + 12345) % 2147483648;
      return seed / 2147483648;
    };

    let state = createSession(
      { mode: "double", targetLength: 5, pools: [buildPool("char-2", items)] },
      new Map(),
      rng,
      1000,
    );
    expect(state).not.toBeNull();
    state = state!;

    let keystrokeBudget = 500;
    let questionsCompleted = 0;
    while (state.phase !== "completed" && keystrokeBudget-- > 0) {
      if (state.phase === "feedback") {
        // 题目完成:按桌面端同一语义把进度写入 StorageAdapter。
        const progress = load();
        const id = state.current.id;
        const current = progress[id] ?? emptyProgress();
        progress[id] =
          state.lastOutcome === "perfect"
            ? { ...current, attempts: current.attempts + 1, correct: current.correct + 1 }
            : { ...current, attempts: current.attempts + 1, wrong: current.wrong + 1 };
        save(progress);
        questionsCompleted = state.questionsCompleted;
        state = advance(state, rng, 2000).state;
        continue;
      }
      const expected: string = state.current.primaryCode[state.typed.length];
      expect(expected).toBeDefined();
      // 用真实编码逐键输入:任何键错误都会被判错(引擎负责校验)。
      state = typeKey(state, expected, rng, 1500).state;
    }

    expect(questionsCompleted).toBe(5);
    expect(state.phase).toBe("completed");
    const persisted = load();
    expect(Object.keys(persisted).length).toBeGreaterThanOrEqual(1);
    for (const progress of Object.values(persisted)) {
      expect(progress.attempts).toBe(1);
    }
  });

  it("按错的键会被引擎判错(真实校验,不是回放)", () => {
    const dataset = validateTrainerDataset(shard);
    const index = buildTrainerIndex(dataset);
    const items = selectPool(index, "char-2", "daily");
    let seed = 1;
    const rng = () => {
      seed = (seed * 1103515245 + 12345) % 2147483648;
      return seed / 2147483648;
    };
    let state = createSession(
      { mode: "double", targetLength: 1, pools: [buildPool("char-2", items)] },
      new Map(),
      rng,
      1000,
    )!;
    const wrongKey = state.current.primaryCode[0] === "a" ? "b" : "a";
    state = typeKey(state, wrongKey, rng, 1200).state;
    expect(state.hadError).toBe(true);
    expect(state.wrongKeyEvents).toBeGreaterThan(0);
  });
});
