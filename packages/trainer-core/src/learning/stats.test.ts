import { describe, expect, it } from "vitest";
import {
  accuracy,
  formatDuration,
  formatPercent,
  kpm,
  localDateKey,
} from "./stats";

describe("localDateKey", () => {
  it("使用本地日历日而非 UTC", () => {
    // 2026-09-01 23:30 本地时间,UTC 可能已是次日;键必须是本地日。
    const date = new Date(2026, 8, 1, 23, 30, 0);
    expect(localDateKey(date)).toBe("2026-09-01");
  });

  it("月日补零", () => {
    expect(localDateKey(new Date(2026, 0, 5))).toBe("2026-01-05");
  });
});

describe("accuracy", () => {
  it("零输入返回 null 而不是假数据", () => {
    expect(accuracy(0, 0)).toBeNull();
  });

  it("按键级事件计算", () => {
    expect(accuracy(10, 2)).toBeCloseTo(0.8);
    expect(accuracy(4, 0)).toBe(1);
  });
});

describe("kpm", () => {
  it("零时长/零输入返回 null", () => {
    expect(kpm(0, 1000)).toBeNull();
    expect(kpm(10, 0)).toBeNull();
  });

  it("按活跃分钟计算", () => {
    expect(kpm(120, 60_000)).toBeCloseTo(120);
    expect(kpm(45, 30_000)).toBeCloseTo(90);
  });
});

describe("formatDuration / formatPercent", () => {
  it("时长格式化", () => {
    expect(formatDuration(45_000)).toBe("45 秒");
    expect(formatDuration(12 * 60_000)).toBe("12 分钟");
    expect(formatDuration(63 * 60_000)).toBe("1 小时 3 分钟");
    expect(formatDuration(120 * 60_000)).toBe("2 小时");
  });

  it("百分比格式化", () => {
    expect(formatPercent(null)).toBe("—");
    expect(formatPercent(0.923)).toBe("92%");
  });
});
