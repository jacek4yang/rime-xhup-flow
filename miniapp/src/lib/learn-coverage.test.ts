/**
 * 学习中心覆盖数推导测试(真实生产数据):
 * - 覆盖数只来自本机真实练习进度;
 * - 空进度 → 全部为零(不伪造);
 * - 练过的条目计入对应章节,不练不计;
 * - 状态不可推导:接口不暴露状态字段(类型层强制)。
 */

import { describe, expect, it } from "vitest";
import { emptyProgress, selectPool } from "@xhup/trainer-core";
import { chapterCoverageById } from "./learn-coverage";
import { trainerIndex as index } from "../testing/compiled-dataset";

const NOW = 1_788_710_400_000;

describe("chapterCoverageById(小程序学习中心)", () => {
  it("空进度 → 所有章节 practiced=0,不伪造;总数为真实关联池规模", () => {
    const coverage = chapterCoverageById(index, {}, NOW);
    expect(coverage.size).toBe(9);
    for (const entry of coverage.values()) {
      expect(entry.practiced).toBe(0);
      expect(entry.total).toBeGreaterThanOrEqual(0);
    }
    // 双拼章节 = char-2 池;概览/FAQ 无练习小节 → 0。
    const double = coverage.get("double")!;
    expect(double.total).toBe(index.pools["char-2"].length); // 全池,非难度子集
    expect(coverage.get("overview")!.total).toBe(0);
  });

  it("练过的条目计入对应章节;跨章共享池条目在两章都计入", () => {
    const doubleItems = selectPool(index, "char-2", "daily");
    const mixedItems = selectPool(index, "char-3", "daily");
    const progress = {
      [doubleItems[0]!.id]: { ...emptyProgress(), attempts: 3, correct: 3 },
      [mixedItems[1]!.id]: { ...emptyProgress(), attempts: 1 },
    };
    const coverage = chapterCoverageById(index, progress, NOW);
    expect(coverage.get("double")!.practiced).toBe(1);
    expect(coverage.get("shape")!.practiced).toBe(1);
    // mastery 章节包含 char-2/char-3 → 两章条目都计入。
    expect(coverage.get("mastery")!.practiced).toBe(2);
  });
});
