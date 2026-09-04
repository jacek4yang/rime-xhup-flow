/**
 * 触感反馈测试:模式门控、强度缩放与不支持环境的静默降级。
 */

import { afterEach, describe, expect, it, vi } from "vitest";
import { haptic } from "./haptics";

const vibrateMock = vi.fn(() => true);

function stubVibrate(supported: boolean) {
  if (supported) {
    Object.defineProperty(navigator, "vibrate", {
      value: vibrateMock,
      configurable: true,
    });
  } else {
    Object.defineProperty(navigator, "vibrate", {
      value: undefined,
      configurable: true,
    });
  }
}

afterEach(() => {
  vi.restoreAllMocks();
  // 清理 stub(jsdom 的 navigator 默认无 vibrate)。
  delete (navigator as unknown as Record<string, unknown>).vibrate;
});

describe("haptics", () => {
  it("off 模式永不触发", () => {
    stubVibrate(true);
    expect(haptic("off", "key")).toBe(false);
    expect(vibrateMock).not.toHaveBeenCalled();
  });

  it("light 模式按键触发短促反馈", () => {
    stubVibrate(true);
    expect(haptic("light", "key")).toBe(true);
    expect(vibrateMock).toHaveBeenCalledWith(8);
  });

  it("medium 强度放大 50%;wrong 比按键更强", () => {
    stubVibrate(true);
    haptic("medium", "key");
    expect(vibrateMock).toHaveBeenLastCalledWith(12);
    haptic("light", "wrong");
    expect(vibrateMock).toHaveBeenLastCalledWith(22);
  });

  it("环境不支持时静默返回 false,不崩溃", () => {
    stubVibrate(false);
    expect(haptic("light", "key")).toBe(false);
    expect(haptic("medium", "wrong")).toBe(false);
  });

  it("vibrate 抛错时静默降级", () => {
    stubVibrate(true);
    vibrateMock.mockImplementation(() => {
      throw new Error("blocked");
    });
    expect(haptic("light", "success")).toBe(false);
  });
});