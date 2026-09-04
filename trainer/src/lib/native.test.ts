/**
 * 桌面 IPC 边界测试:桌面模式类型化调用、浏览器降级、命令错误归一化
 * (稳定机器码契约,见 trainer/src-tauri/src/commands.rs)。
 */

import { afterEach, describe, expect, it, vi } from "vitest";
import { CommandError, DesktopUnavailableError, invokeDesktop, isDesktopApp } from "./native";

afterEach(() => {
  delete (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__;
});

describe("native IPC boundary", () => {
  it("浏览器环境:isDesktopApp 为 false,invokeDesktop 抛降级错误", async () => {
    expect(isDesktopApp()).toBe(false);
    await expect(invokeDesktop("product_status")).rejects.toBeInstanceOf(
      DesktopUnavailableError,
    );
  });

  it("桌面模式:类型化调用转发命令与参数", async () => {
    const invoke = vi.fn().mockResolvedValue({ bundled_version: "0.1.0" });
    (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__ = { invoke };
    expect(isDesktopApp()).toBe(true);
    await expect(invokeDesktop("product_status")).resolves.toEqual({
      bundled_version: "0.1.0",
    });
    expect(invoke).toHaveBeenCalledWith("product_status", undefined);
    await invokeDesktop("learning_reset", { confirmed: true, dictName: "xhup_flow_user" });
    expect(invoke).toHaveBeenCalledWith("learning_reset", {
      confirmed: true,
      dictName: "xhup_flow_user",
    });
  });

  it("Rust 结构化错误({code,message})归一化为 CommandError", async () => {
    const invoke = vi
      .fn()
      .mockRejectedValue({ code: "rime_not_detected", message: "Rime 用户数据目录不存在" });
    (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__ = { invoke };
    const error = await invokeDesktop("product_status").catch((cause: unknown) => cause);
    expect(error).toBeInstanceOf(CommandError);
    expect((error as CommandError).code).toBe("rime_not_detected");
  });

  it("非结构化拒绝原样透传", async () => {
    const invoke = vi.fn().mockRejectedValue("boom");
    (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__ = { invoke };
    await expect(invokeDesktop("product_status")).rejects.toBe("boom");
  });

  it("internals 缺少 invoke 函数时视为非桌面环境", () => {
    (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__ = {};
    expect(isDesktopApp()).toBe(false);
  });
});
