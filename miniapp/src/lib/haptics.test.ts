/**
 * 触感映射测试:设置档位 → 微信物理震动类型的诚实映射。
 */

import { describe, expect, it } from "vitest";
import { createHaptics, resolveVibrationType } from "./haptics";

describe("resolveVibrationType", () => {
  it("off 永不触发", () => {
    expect(resolveVibrationType("off", "tap")).toBeNull();
    expect(resolveVibrationType("off", "wrong")).toBeNull();
    expect(resolveVibrationType("off", "success")).toBeNull();
  });

  it("tap / success 使用设置档位本身(light→light,medium→medium)", () => {
    expect(resolveVibrationType("light", "tap")).toBe("light");
    expect(resolveVibrationType("light", "success")).toBe("light");
    expect(resolveVibrationType("medium", "tap")).toBe("medium");
    expect(resolveVibrationType("medium", "success")).toBe("medium");
  });

  it("wrong 在设置档位上真实加强一档(不伪装时长)", () => {
    expect(resolveVibrationType("light", "wrong")).toBe("medium");
    expect(resolveVibrationType("medium", "wrong")).toBe("heavy");
  });
});

describe("createHaptics(HapticsAdapter 契约)", () => {
  it("按当前设置触发注入的 vibrate;设置即时生效", () => {
    const fired: string[] = [];
    let mode: "off" | "light" | "medium" = "light";
    const adapter = createHaptics((type) => fired.push(type), () => mode);

    adapter.tap();
    adapter.wrong();
    adapter.success();
    expect(fired).toEqual(["light", "medium", "light"]);

    mode = "off";
    adapter.wrong();
    expect(fired).toHaveLength(3);

    mode = "medium";
    adapter.tap();
    expect(fired).toEqual(["light", "medium", "light", "medium"]);
  });

  it("vibrate 抛错(设备不支持)静默无效", () => {
    const adapter = createHaptics(() => {
      throw new Error("not supported");
    }, () => "light");
    expect(() => adapter.tap()).not.toThrow();
    expect(() => adapter.wrong()).not.toThrow();
  });
});
