/**
 * 章节证据充分性反例与规则(任务书 §3)。
 *
 * 反例(修复前成立):20 个已练条目中 19 个是 V3 迁移历史(accuracyEwma
 * 为 null,历史 mastery 高),仅 1 个条目有新证据 accuracyEwma = 1。
 * 旧逻辑对非 null EWMA 求均值 → 表现 = 1.0,叠加历史高掌握度与样本
 * 充分性门禁(20 个已练)→ 章节被标记 mastered。
 * 单个新样本不该验证整章表现:这正是 #240 要消除的「一次完美终身达标」
 * 在混合数据下的复发路径。
 */

import { describe, expect, it } from "vitest";
import {
  DEFAULT_LESSON_THRESHOLDS,
  chapterRelatedItems,
  chapterRelatedProgress,
  deriveAllLessonStates,
  deriveLessonState,
  type LessonEvidence,
} from "./lesson-state";
import { emptyLessonEvidence } from "./lesson-state";
import { emptyProgress, type ItemProgress } from "../learning/progress";
import { LEARN_CHAPTERS } from "../lessons/content";
import { makeIndex } from "../testing/fixtures";

const NOW = 1_000_000_000_000;

function evidence(overrides: Partial<LessonEvidence> = {}): LessonEvidence {
  return { ...emptyLessonEvidence(), ...overrides };
}

function progress(overrides: Partial<ItemProgress> = {}): ItemProgress {
  return { ...emptyProgress(), ...overrides };
}

/** 反例数据:19 个迁移历史条目 + 1 个新证据条目,历史掌握度全部很高。 */
function mixedChapterItems(newEvidenceCount: number, total = DEFAULT_LESSON_THRESHOLDS.minPracticedItems) {
  const items: ItemProgress[] = [];
  for (let index = 0; index < total; index += 1) {
    const hasNewEvidence = index < newEvidenceCount;
    items.push(progress({
      attempts: 10,
      correct: 10,
      mastery: 95,
      accuracyEwma: hasNewEvidence ? 1 : null,
      lastSeenAt: NOW,
    }));
  }
  return items;
}

describe("章节证据充分性(新证据不足不得声称 mastered)", () => {
  it("反例:19 个迁移历史 + 仅 1 个新证据条目 → 不得标记 mastered", () => {
    const state = deriveLessonState(
      evidence({
        openedAt: NOW, practiceSessions: 5, practiceAttempts: 200,
        lastPracticeAt: NOW, bestAccuracy: 1,
      }),
      mixedChapterItems(1),
      NOW,
    );
    expect(state).not.toBe("mastered");
  });

  it("新证据达到最小数量(默认 5)且表现达标 → mastered", () => {
    const state = deriveLessonState(
      evidence({
        openedAt: NOW, practiceSessions: 5, practiceAttempts: 200,
        lastPracticeAt: NOW, bestAccuracy: 1,
      }),
      mixedChapterItems(DEFAULT_LESSON_THRESHOLDS.minNewEvidenceItems),
      NOW,
    );
    expect(state).toBe("mastered");
  });

  it("纯旧数据(全部迁移历史,零新证据)保持既有回退路径:bestAccuracy 代理仍可 mastered", () => {
    const state = deriveLessonState(
      evidence({
        openedAt: NOW, practiceSessions: 5, practiceAttempts: 200,
        lastPracticeAt: NOW, bestAccuracy: 1,
      }),
      mixedChapterItems(0),
      NOW,
    );
    expect(state).toBe("mastered");
  });

  it("混合数据中已有少量新证据且表现差 → 不得因 bestAccuracy=1 回退达标", () => {
    const items = mixedChapterItems(2).map((item, index) =>
      index < 2 ? { ...item, accuracyEwma: 0.3 } : item);
    const state = deriveLessonState(
      evidence({
        openedAt: NOW, practiceSessions: 5, practiceAttempts: 200,
        lastPracticeAt: NOW, bestAccuracy: 1,
      }),
      items,
      NOW,
    );
    expect(state).not.toBe("mastered");
  });
});

describe("deriveAllLessonStates 反向遍历与正向路径等价(记忆化正确性)", () => {
  it("与 chapterRelatedItems + chapterRelatedProgress 逐字段的朴素重算完全一致", () => {
    const index = makeIndex();
    // 构造跨池的稀疏进度:覆盖 null/非 null EWMA、attempts=0、非关联条目。
    const progress: Record<string, ItemProgress> = {};
    let seeded = 0;
    for (const pool of Object.values(index.pools)) {
      for (const item of pool) {
        seeded += 1;
        if (seeded % 3 === 0) continue;
        progress[item.id] = {
          ...emptyProgress(),
          attempts: seeded % 2,
          correct: 1,
          mastery: (seeded * 7) % 100,
          accuracyEwma: seeded % 4 === 0 ? 0.5 + (seeded % 10) / 20 : null,
          lastSeenAt: NOW,
        };
      }
    }
    progress["not-in-any-pool"] = { ...emptyProgress(), attempts: 5, mastery: 90 };
    const lessonEvidence: Record<string, LessonEvidence> = {};
    for (const chapter of LEARN_CHAPTERS) {
      lessonEvidence[chapter.id] = evidence({
        openedAt: NOW - 1000, practiceSessions: 3, practiceAttempts: 120,
        lastPracticeAt: NOW, bestAccuracy: 0.9,
      });
    }
    const summaries = deriveAllLessonStates(index, progress, lessonEvidence, NOW);
    expect(summaries).toHaveLength(LEARN_CHAPTERS.length);
    for (const summary of summaries) {
      const naiveTotal = chapterRelatedItems(summary.chapter, index).length;
      const naivePracticed = chapterRelatedProgress(summary.chapter, index, progress);
      const naiveState = deriveLessonState(
        lessonEvidence[summary.chapter.id], naivePracticed, NOW,
      );
      expect(summary.relatedTotal).toBe(naiveTotal);
      expect(summary.practicedCount).toBe(naivePracticed.length);
      expect(summary.state).toBe(naiveState);
    }
    // 二次调用(命中缓存)结果不变。
    const again = deriveAllLessonStates(index, progress, lessonEvidence, NOW);
    expect(again).toEqual(summaries);
  });
});
