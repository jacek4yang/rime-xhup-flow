import { describe, expect, it } from "vitest";
import { evaluateCheckpoints } from "./checkpoints";
import { emptyProgress, type ItemProgress } from "../learning/progress";
import { emptyLessonEvidence } from "./lesson-state";
import { makeEntry } from "../testing/fixtures";
import { makeIndex } from "../testing/fixtures";

const NOW = 1_000_000_000_000;

function progress(overrides: Partial<ItemProgress> = {}): ItemProgress {
  return { ...emptyProgress(), ...overrides };
}

/** 带 24 个 2 码单字的数据集索引(覆盖类检查点需要足够大的池)。 */
function bigIndex() {
  const entries = Array.from({ length: 24 }, (_, i) =>
    makeEntry(`字${i}`, `${String.fromCharCode(97 + (i % 26))}b`, 10),
  );
  return makeIndex({ entries });
}

function char2Progress(count: number, mastery: number): Record<string, ItemProgress> {
  const index = bigIndex();
  const result: Record<string, ItemProgress> = {};
  for (const item of index.pools["char-2"].slice(0, count)) {
    result[item.id] = progress({ attempts: 4, correct: 4, mastery });
  }
  return result;
}

describe("evaluateCheckpoints", () => {
  it("空输入:全部未达成,进度为 0,按 beginner → mastery 顺序", () => {
    const results = evaluateCheckpoints({
      index: makeIndex(),
      progressById: {},
      lessonEvidence: {},
      now: NOW,
    });
    expect(results.map((result) => result.id)).toEqual([
      "beginner",
      "foundation",
      "intermediate",
      "advanced",
      "mastery",
    ]);
    for (const result of results) {
      expect(result.achieved).toBe(false);
      expect(result.progress).toBe(0);
    }
  });

  it("beginner:覆盖与质量组件分别推进,全部达标才 achieved", () => {
    const index = bigIndex();
    const onlyCoverage = evaluateCheckpoints({
      index,
      progressById: char2Progress(10, 0),
      lessonEvidence: {},
      now: NOW,
    });
    const beginner = onlyCoverage[0]!;
    // 10/20 覆盖 = 0.5,质量 0 → 总进度 0.25。
    expect(beginner.components[0]).toMatchObject({ progress: 0.5 });
    expect(beginner.components[1]).toMatchObject({ progress: 0 });
    expect(beginner.progress).toBe(0.25);
    expect(beginner.achieved).toBe(false);

    const full = evaluateCheckpoints({
      index,
      progressById: char2Progress(20, 80),
      lessonEvidence: {},
      now: NOW,
    });
    // 20/20 覆盖满分 + 平均掌握 80/70 满分 → 达成。
    expect(full[0]!.achieved).toBe(true);
    expect(full[0]!.progress).toBe(1);
  });

  it("章节练习证据推进 foundation 的章节组件", () => {
    const index = makeIndex();
    const results = evaluateCheckpoints({
      index,
      progressById: {},
      lessonEvidence: {
        shape: { ...emptyLessonEvidence(), practiceSessions: 2 },
      },
      now: NOW,
    });
    const foundation = results[1]!;
    // 基础层带练习入口的章节 = shape、shape-memory;练了 1/2。
    expect(foundation.components[2]).toMatchObject({ progress: 0.5 });
    expect(foundation.progress).toBeCloseTo(1 / 6);
  });

  it("mastery:总体准确率进入组件进度", () => {
    const index = makeIndex();
    const results = evaluateCheckpoints({
      index,
      progressById: {
        "行:xk": progress({ attempts: 10, correct: 10, mastery: 90 }),
        "我:wo": progress({ attempts: 10, correct: 9, wrong: 1, mastery: 80 }),
      },
      lessonEvidence: {},
      now: NOW,
    });
    const mastery = results[4]!;
    // 准确率 19/20 = 0.95 → 该组件满分;覆盖 2/100。
    expect(mastery.components[0]).toMatchObject({ progress: 0.02 });
    expect(mastery.components[1]).toMatchObject({ progress: 1 });
  });

  it("确定性:相同输入两次评估结果一致", () => {
    const args = {
      index: bigIndex(),
      progressById: char2Progress(5, 60),
      lessonEvidence: { double: { ...emptyLessonEvidence(), practiceSessions: 1 } },
      now: NOW,
    };
    expect(evaluateCheckpoints(args)).toEqual(evaluateCheckpoints(args));
  });
});
