/**
 * 控制中心组件级测试:以用户可见行为为准(状态卡、计划确认、卸载确认、
 * 浏览器降级提示)。Rust 侧管理逻辑由 `trainer/src-tauri` 的单元测试覆盖。
 */

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { resetTrainerStore } from "@/stores/trainer-store";
import type { ProductStatusDto, TauriInternals } from "@/lib/product";
import { ControlCenterView } from "./ControlCenterView";

const invokeMock = vi.fn();

/** 注入假 Tauri IPC(Tauri v2 通过 window.__TAURI_INTERNALS__ 暴露)。 */
function stubDesktop() {
  (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__ = {
    invoke: invokeMock,
  } satisfies TauriInternals;
}

const USER_DIR = "/home/tester/.config/fcitx5/rime";

/** 未安装状态(全新 Rime 目录)。 */
function freshStatus(): ProductStatusDto {
  return {
    client: "Fcitx5",
    redeploy_guidance: "重启 Fcitx5。",
    user_data_dir: USER_DIR,
    rime_detected: true,
    install: {
      user_data_dir: USER_DIR,
      client: "Fcitx5",
      installed_files: 0,
      total_files: 11,
      missing_files: [],
      schemas: [],
      installed_version: null,
    },
    bundled_version: "1.0.0",
    update_available: false,
    learning: {
      user_dict: "xhup_flow_user",
      db_exists: false,
      snapshot_available: false,
      tool_available: false,
    },
  };
}

beforeEach(() => {
  resetTrainerStore();
  invokeMock.mockReset();
  stubDesktop();
});

afterEach(() => {
  delete (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__;
  vi.unstubAllEnvs();
});

describe("ControlCenterView", () => {
  it("浏览器环境降级:只显示需要桌面应用的提示,不发起任何调用", async () => {
    delete (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__;
    render(<ControlCenterView />);
    expect(screen.getByText(/桌面应用/)).toBeInTheDocument();
    await waitFor(() => expect(invokeMock).not.toHaveBeenCalled());
  });

  it("展示安装状态卡(客户端、目录、未安装徽章)", async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === "product_status") return Promise.resolve(freshStatus());
      return Promise.reject(new Error(`unexpected command: ${command}`));
    });
    render(<ControlCenterView />);
    expect(await screen.findByText(/Fcitx5-Rime/)).toBeInTheDocument();
    expect(screen.getByText(USER_DIR)).toBeInTheDocument();
    expect(screen.getByText("未安装")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "安装" })).toBeInTheDocument();
  });

  it("安装走「计划 → 确认 → 执行」流程并展示重新部署指引", async () => {
    const user = userEvent.setup();
    invokeMock.mockImplementation((command: string) => {
      if (command === "product_status") return Promise.resolve(freshStatus());
      if (command === "product_plan") {
        return Promise.resolve({
          actions: [
            { kind: "write", file: "xhup_flow.schema.yaml" },
            { kind: "write", file: "xhup_flow.dict.yaml" },
          ],
          notes: [],
        });
      }
      if (command === "product_execute") {
        return Promise.resolve({ done: 11, redeploy_guidance: "重启 Fcitx5。" });
      }
      return Promise.reject(new Error(`unexpected command: ${command}`));
    });
    render(<ControlCenterView />);
    await user.click(await screen.findByRole("button", { name: "安装" }));
    expect(await screen.findByText(/尚未写入任何文件/)).toBeInTheDocument();
    expect(screen.getByText("xhup_flow.schema.yaml")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "确认执行" }));
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("product_execute", { kind: "install" }),
    );
    expect(await screen.findByText(/完成 11 项操作/)).toBeInTheDocument();
  });

  it("卸载需要显式确认,确认后只执行 uninstall", async () => {
    const user = userEvent.setup();
    const status = freshStatus();
    status.install = {
      ...status.install!,
      installed_files: 11,
      installed_version: "1.0.0",
      schemas: ["xhup_flow", "xhup_flow_static"],
    };
    invokeMock.mockImplementation((command: string) => {
      if (command === "product_status") return Promise.resolve(status);
      if (command === "product_execute") {
        return Promise.resolve({ done: 11, redeploy_guidance: "重启 Fcitx5。" });
      }
      return Promise.reject(new Error(`unexpected command: ${command}`));
    });
    render(<ControlCenterView />);
    await user.click(await screen.findByRole("button", { name: "卸载" }));
    expect(await screen.findByText(/只删除 XHUP 拥有的 11 个方案文件/)).toBeInTheDocument();
    await user.click(screen.getAllByRole("button", { name: "卸载" })[1]);
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("product_execute", { kind: "uninstall" }),
    );
  });

  it("学习数据区展示本地状态与隐私说明,重置需要确认", async () => {
    const user = userEvent.setup();
    invokeMock.mockImplementation((command: string) => {
      if (command === "product_status") return Promise.resolve(freshStatus());
      if (command === "learning_reset") return Promise.resolve();
      return Promise.reject(new Error(`unexpected command: ${command}`));
    });
    render(<ControlCenterView />);
    expect(await screen.findByText("尚无学习数据(开始组句练习后自动生成)")).toBeInTheDocument();
    expect(screen.getByText(/无账号、无遥测、无云端同步/)).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "重置学习" }));
    expect(await screen.findByText("重置学习数据?")).toBeInTheDocument();
    await user.click(screen.getAllByRole("button", { name: "重置学习" })[1]);
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("learning_reset", { confirmed: true }),
    );
  });
});
