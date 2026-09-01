import { describe, expect, it } from "vitest";
import {
  applyImperfect,
  applyPerfect,
  emptyProgress,
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
