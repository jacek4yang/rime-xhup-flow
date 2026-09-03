import { describe, expect, it } from "vitest";
import {
  applyImperfect,
  applyPerfect,
  emptyProgress,
  nextAvgLatency,
} from "./progress";

describe("mastery 更新规则", () => {
  it("完美完成:attempts/correct/streak +1,掌握度按连对加速", () => {
    const now = 1000;
    const once = applyPerfect(emptyProgress(), now);
    expect(once).toMatchObject({
      attempts: 1,
      correct: 1,
      wrong: 0,
      streak: 1,
      mastery: 4, // min(8, 3 + 1)
      lastSeenAt: now,
    });

    const twice = applyPerfect(once, now + 1);
    expect(twice.streak).toBe(2);
    expect(twice.mastery).toBe(4 + 5); // min(8, 3 + 2)
  });

  it("掌握度增量封顶为 8", () => {
    let progress = emptyProgress();
    for (let i = 0; i < 10; i += 1) progress = applyPerfect(progress, i);
    const before = progress.mastery;
    const after = applyPerfect(progress, 11);
    expect(after.mastery - before).toBe(8);
  });

  it("掌握度不超过 100", () => {
    let progress = { ...emptyProgress(), mastery: 98 };
    progress = applyPerfect(progress, 0);
    expect(progress.mastery).toBe(100);
  });

  it("有误完成:attempts/wrong +1,streak 清零,掌握度 -15", () => {
    const base = {
      ...emptyProgress(),
      attempts: 3,
      correct: 3,
      streak: 3,
      mastery: 40,
    };
    const after = applyImperfect(base, 500);
    expect(after).toMatchObject({
      attempts: 4,
      correct: 3,
      wrong: 1,
      streak: 0,
      mastery: 25,
      lastSeenAt: 500,
    });
  });

  it("掌握度不低于 0", () => {
    const base = { ...emptyProgress(), mastery: 10 };
    expect(applyImperfect(base, 0).mastery).toBe(0);
  });
});

describe("avgLatencyMs(V2)", () => {
  it("首个有效样本直接成为均值", () => {
    expect(nextAvgLatency(null, 1200)).toBe(1200);
    const after = applyPerfect(emptyProgress(), 0, 1200);
    expect(after.avgLatencyMs).toBe(1200);
  });

  it("后续样本按 0.3 权重指数滑动", () => {
    let progress = emptyProgress();
    progress = applyPerfect(progress, 0, 1000);
    progress = applyPerfect(progress, 1, 2000);
    expect(progress.avgLatencyMs).toBeCloseTo(1000 * 0.7 + 2000 * 0.3);
  });

  it("无效样本(负值)不改变均值", () => {
    const base = applyPerfect(emptyProgress(), 0, 500);
    expect(applyImperfect(base, 1, -5).avgLatencyMs).toBe(500);
    expect(applyImperfect(base, 1, null).avgLatencyMs).toBe(500);
  });

  it("imperfect 同样记录延迟", () => {
    const after = applyImperfect(emptyProgress(), 0, 3000);
    expect(after.avgLatencyMs).toBe(3000);
  });
});
