/**
 * onboarding 持久化测试:最小记录的读写、损坏数据防御与无存储降级。
 */

import { beforeEach, describe, expect, it, vi } from "vitest";
import { readOnboarding, writeOnboarding } from "./onboarding";

const KEY = "xhup-flow.onboarding.v1";

beforeEach(() => {
  window.localStorage.clear();
});

describe("onboarding persistence", () => {
  it("写入后可读回,时间戳为 ISO 字符串", () => {
    expect(writeOnboarding("completed")).toBe(true);
    const record = readOnboarding();
    expect(record?.status).toBe("completed");
    expect(() => new Date(record!.at).toISOString()).not.toThrow();
  });

  it("skipped 状态同样往返", () => {
    writeOnboarding("skipped");
    expect(readOnboarding()?.status).toBe("skipped");
  });

  it("无记录返回 null", () => {
    expect(readOnboarding()).toBeNull();
  });

  it("损坏 JSON 按未引导处理", () => {
    window.localStorage.setItem(KEY, "{not json");
    expect(readOnboarding()).toBeNull();
  });

  it("形状不符(status 非法值)返回 null", () => {
    window.localStorage.setItem(KEY, JSON.stringify({ status: "maybe", at: "2026-01-01" }));
    expect(readOnboarding()).toBeNull();
    window.localStorage.setItem(KEY, JSON.stringify({ status: "completed" }));
    expect(readOnboarding()).toBeNull();
  });

  it("localStorage 抛错时静默降级,不崩溃", () => {
    const throwing = {
      getItem: () => {
        throw new Error("blocked");
      },
      setItem: () => {
        throw new Error("blocked");
      },
    };
    vi.stubGlobal("localStorage", throwing);
    expect(writeOnboarding("completed")).toBe(false);
    expect(readOnboarding()).toBeNull();
    vi.unstubAllGlobals();
  });
});
