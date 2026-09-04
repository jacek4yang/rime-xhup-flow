/**
 * ErrorBoundary 测试:崩溃 → 恢复卡片 → 重试恢复,且全程不清除持久化状态。
 */

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { ErrorBoundary } from "./ErrorBoundary";

/** 通过外部开关控制子组件是否抛错,模拟「修复后重试恢复」。 */
let shouldThrow = true;
function MaybeBoom() {
  if (shouldThrow) throw new Error("boom-123");
  return <p data-testid="recovered">back in business</p>;
}

/**
 * jsdom 自带 no-op clipboard 桩;用 defineProperty 强制/替换其值。
 * 注意:userEvent.setup() 会无条件挂上自己的 clipboard 桩,
 * 因此 copy 行为测试用 fireEvent + 在点击前覆写。
 */
function setClipboard(value: unknown) {
  Object.defineProperty(navigator, "clipboard", { value, configurable: true });
}

describe("ErrorBoundary", () => {
  beforeEach(() => {
    shouldThrow = true;
    localStorage.clear();
    vi.spyOn(console, "error").mockImplementation(() => {});
    localStorage.setItem("trainer-state", '{"persisted":true}');
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("renders the recovery card with the error message when a child throws", () => {
    render(
      <ErrorBoundary>
        <MaybeBoom />
      </ErrorBoundary>,
    );
    expect(screen.getByText("界面出现错误")).toBeInTheDocument();
    expect(screen.getByText("boom-123")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "重试" })).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "复制错误信息" }),
    ).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "回到今日" })).toBeInTheDocument();
  });

  it("recovers via retry when the child stops throwing, without clearing persisted state", async () => {
    const removeItem = vi.spyOn(Storage.prototype, "removeItem");
    const user = userEvent.setup();
    render(
      <ErrorBoundary>
        <MaybeBoom />
      </ErrorBoundary>,
    );
    expect(screen.getByText("界面出现错误")).toBeInTheDocument();

    shouldThrow = false;
    await user.click(screen.getByRole("button", { name: "重试" }));

    expect(screen.getByTestId("recovered")).toBeInTheDocument();
    expect(screen.queryByText("界面出现错误")).not.toBeInTheDocument();
    // 关键约束:恢复过程绝不删除用户持久化状态。
    expect(removeItem).not.toHaveBeenCalled();
    expect(localStorage.getItem("trainer-state")).toBe('{"persisted":true}');
    removeItem.mockRestore();
  });

  it("back-to-today also remounts the subtree and keeps persisted state", async () => {
    const removeItem = vi.spyOn(Storage.prototype, "removeItem");
    const user = userEvent.setup();
    render(
      <ErrorBoundary>
        <MaybeBoom />
      </ErrorBoundary>,
    );
    expect(screen.getByText("界面出现错误")).toBeInTheDocument();

    shouldThrow = false;
    await user.click(screen.getByRole("button", { name: "回到今日" }));

    expect(screen.getByTestId("recovered")).toBeInTheDocument();
    expect(removeItem).not.toHaveBeenCalled();
    expect(localStorage.getItem("trainer-state")).toBe('{"persisted":true}');
    removeItem.mockRestore();
  });

  it("copy button degrades silently when clipboard is unavailable", () => {
    setClipboard(undefined);
    render(
      <ErrorBoundary>
        <MaybeBoom />
      </ErrorBoundary>,
    );
    fireEvent.click(screen.getByRole("button", { name: "复制错误信息" }));
    // 无 clipboard API:按钮文案不变,不崩溃。
    expect(
      screen.getByRole("button", { name: "复制错误信息" }),
    ).toBeInTheDocument();
  });

  it("copy button reports success when clipboard works", async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    setClipboard({ writeText });
    render(
      <ErrorBoundary>
        <MaybeBoom />
      </ErrorBoundary>,
    );
    fireEvent.click(screen.getByRole("button", { name: "复制错误信息" }));
    expect(writeText).toHaveBeenCalledTimes(1);
    expect(
      await screen.findByRole("button", { name: "已复制错误信息" }),
    ).toBeInTheDocument();
  });
});
