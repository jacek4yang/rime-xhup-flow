import { describe, expect, it } from "vitest";
import { aggregateWeakness, keyHeatmap } from "./weakness";
import { emptyProgress } from "./progress";
import { makeIndex } from "@/test/fixtures";

describe("aggregateWeakness", () => {
  it("只统计见过的条目;薄弱排序按错误率 × 掌握度缺口", () => {
    const index = makeIndex();
    const progress = new Map([
      ["行:xk", { ...emptyProgress(), attempts: 4, wrong: 4, mastery: 0, lastSeenAt: 10 }],
      ["好:hk", { ...emptyProgress(), attempts: 5, correct: 5, mastery: 90, lastSeenAt: 11 }],
      ["我:wo", { ...emptyProgress(), attempts: 2, wrong: 1, mastery: 50, lastSeenAt: 12 }],
    ]);
    const report = aggregateWeakness(index, progress, {});
    // 行:xk 错误率 1.0 × 缺口 100 = 100 分,应为最弱。
    expect(report.items[0]?.id).toBe("行:xk");
    expect(report.items[0]?.score).toBe(100);
    // 没有错题的条目不出现在列表里。
    expect(report.items.some((item) => item.id === "好:hk")).toBe(false);
  });

  it("维度聚合:byKind / byCodeLength 覆盖条目并计算错误率", () => {
    const index = makeIndex();
    const progress = new Map([
      ["行:xk", { ...emptyProgress(), attempts: 4, wrong: 2, mastery: 10, lastSeenAt: 10 }],
      ["word:我们:womf", { ...emptyProgress(), attempts: 2, wrong: 1, mastery: 20, lastSeenAt: 11 }],
    ]);
    const report = aggregateWeakness(index, progress, {});
    expect(report.byKind.char.items).toBe(1);
    expect(report.byKind.word.items).toBe(1);
    expect(report.byKind.char.wrongRate).toBeCloseTo(0.5);
    expect(report.byKind.word.wrongRate).toBeCloseTo(0.5);
    expect(report.byCodeLength[2]?.attempts).toBe(4);
    expect(report.byCodeLength[4]?.attempts).toBe(2);
  });

  it("recentMistakes 按最近见过排序", () => {
    const index = makeIndex();
    const progress = new Map([
      ["行:xk", { ...emptyProgress(), attempts: 1, wrong: 1, lastSeenAt: 1 }],
      ["我:wo", { ...emptyProgress(), attempts: 1, wrong: 1, lastSeenAt: 99 }],
    ]);
    const report = aggregateWeakness(index, progress, {}, 20);
    expect(report.recentMistakes[0]?.id).toBe("我:wo");
  });

  it("limit 截断条目列表", () => {
    const index = makeIndex();
    const progress = new Map([
      ["行:xk", { ...emptyProgress(), attempts: 1, wrong: 1, lastSeenAt: 1 }],
      ["我:wo", { ...emptyProgress(), attempts: 1, wrong: 1, lastSeenAt: 2 }],
    ]);
    const report = aggregateWeakness(index, progress, {}, 1);
    expect(report.items).toHaveLength(1);
  });
});

describe("keyHeatmap", () => {
  it("只保留单个小写字母键且过滤 0 值", () => {
    const heat = keyHeatmap({ a: 3, Z: 1, "1": 2, b: 0 });
    expect(heat).toEqual({ a: 3 });
  });
});
