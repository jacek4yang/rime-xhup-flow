/**
 * 首次启动向导组件级测试:以用户可见行为为准。覆盖浏览器降级、
 * 全新安装全流程、已健康、Rime 缺失、手动部署、跳过持久化与
 * 「已引导过不弹窗」。全部使用假 IPC,不触碰真实 Rime 目录。
 */

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { resetTrainerStore } from "@/stores/trainer-store";
import type { ProductStatusDto } from "@/lib/product";
import type { TauriInternals } from "@/lib/native";
import { FirstRunWizard } from "./FirstRunWizard";
import { clearOnboarding, writeOnboarding } from "./onboarding";

const invokeMock = vi.fn();

function stubDesktop() {
  (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__ = {
    invoke: invokeMock,
  } satisfies TauriInternals;
}

const USER_DIR = "C:\\Users\\tester\\AppData\\Roaming\\Rime";

function status(overrides: Partial<ProductStatusDto> = {}): ProductStatusDto {
  return {
    client: "Weasel",
    redeploy_guidance: "右键任务栏图标 → 重新部署。",
    user_data_dir: USER_DIR,
    rime_detected: true,
    install: {
      user_data_dir: USER_DIR,
      client: "Weasel",
      installed_files: 0,
      total_files: 11,
      missing_files: [],
      schemas: [],
      installed_version: null,
      integrity: [],
    },
    health: "not_installed",
    bundled_version: "1.0.0",
    update_available: false,
    learning: null,
    ...overrides,
  };
}

const PLAN = {
  actions: [{ kind: "write" as const, file: "xhup_flow.schema.yaml" }],
  notes: ["写入 11 个文件"],
};
const RESULT = { done: 11, redeploy_guidance: "右键任务栏图标 → 重新部署。" };

/** 默认命令路由:全新安装全流程(执行后状态转健康,模拟真实时序)。 */
function mockFullInstall() {
  let installed = false;
  invokeMock.mockImplementation((command: string) => {
    switch (command) {
      case "product_status":
        return Promise.resolve(
          installed
            ? status({
                health: "healthy",
                update_available: false,
                install: { ...status().install!, installed_files: 11, installed_version: "1.0.0" },
              })
            : status(),
        );
      case "product_plan":
        return Promise.resolve(PLAN);
      case "product_execute":
        installed = true;
        return Promise.resolve(RESULT);
      case "product_redeploy":
        return Promise.resolve("deployed");
      default:
        return Promise.reject(new Error(`unexpected: ${command}`));
    }
  });
}

beforeEach(() => {
  resetTrainerStore();
  invokeMock.mockReset();
  window.localStorage.clear();
  stubDesktop();
});

afterEach(() => {
  delete (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__;
});

async function openWizard() {
  render(<FirstRunWizard onStartTraining={vi.fn()} onOpenControlCenter={vi.fn()} />);
  await screen.findByText("欢迎使用 XHUP Flow");
}

describe("FirstRunWizard", () => {
  it("浏览器环境:检测降级为可重试的失败态,且不假装成功", async () => {
    delete (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__;
    await openWizard();
    await userEvent.click(screen.getByRole("button", { name: "开始" }));
    expect(await screen.findByText("检测失败")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "重试" })).toBeInTheDocument();
  });

  it("全新安装全流程:检测 → 选方案 → 计划 → 安装 → 部署 → 验证 → 试打 → 训练", async () => {
    mockFullInstall();
    const onTraining = vi.fn();
    render(<FirstRunWizard onStartTraining={onTraining} onOpenControlCenter={vi.fn()} />);
    await screen.findByText("欢迎使用 XHUP Flow");
    await userEvent.click(screen.getByRole("button", { name: "开始" }));

    // 方案选择(两套说明,不出现内部术语)。
    await screen.findByText("选择输入方式");
    expect(screen.getByText(/连续组句与本地学习/)).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: /流畅模式/ }));

    // 计划预览 → 确认。
    await screen.findByText("确认安装计划");
    expect(screen.getByText(/xhup_flow\.schema\.yaml/)).toBeInTheDocument();
    expect(screen.getByText(/学习数据与你的其他 Rime 配置不会被删除/)).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "确认安装" }));

    // 部署 → 验证 → 试打。
    await screen.findByText("试打一下");
    expect(screen.getByText(/11\/11 个文件校验一致/)).toBeInTheDocument();
    expect(screen.getByText("womf → 我们")).toBeInTheDocument();
    expect(screen.getByText("uijm → 时间")).toBeInTheDocument();

    // 训练路径 → 开始训练回调。
    await userEvent.click(screen.getByRole("button", { name: "下一步" }));
    await screen.findByText("开始新手训练");
    expect(screen.getByText("双拼(2 码)")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "开始训练" }));

    await waitFor(() => expect(onTraining).toHaveBeenCalledWith("double"));
    expect(window.localStorage.getItem("xhup-flow.onboarding.v1")).toContain("completed");
  });

  it("手动重新部署:展示官方指引而不是失败", async () => {
    invokeMock.mockImplementation((command: string) => {
      switch (command) {
        case "product_status":
          return Promise.resolve(status());
        case "product_plan":
          return Promise.resolve(PLAN);
        case "product_execute":
          return Promise.resolve(RESULT);
        case "product_redeploy":
          return Promise.reject(new Error("redeploy_unavailable"));
        default:
          return Promise.reject(new Error(`unexpected: ${command}`));
      }
    });
    render(<FirstRunWizard onStartTraining={vi.fn()} onOpenControlCenter={vi.fn()} />);
    await screen.findByText("欢迎使用 XHUP Flow");
    await userEvent.click(screen.getByRole("button", { name: "开始" }));
    await userEvent.click(await screen.findByRole("button", { name: /流畅模式/ }));
    await userEvent.click(await screen.findByRole("button", { name: "确认安装" }));

    expect(await screen.findByText("请手动重新部署")).toBeInTheDocument();
    expect(screen.getByText("右键任务栏图标 → 重新部署。")).toBeInTheDocument();

    // 用户完成部署后仍走验证。
    invokeMock.mockImplementation((command: string) => {
      if (command === "product_status") {
        return Promise.resolve(status({ health: "healthy", update_available: false }));
      }
      return Promise.reject(new Error(`unexpected: ${command}`));
    });
    await userEvent.click(screen.getByRole("button", { name: "已完成部署,验证安装" }));
    expect(await screen.findByText("试打一下")).toBeInTheDocument();
  });

  it("已安装且健康:不显示安装向导,可直接进入应用", async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === "product_status") {
        return Promise.resolve(
          status({
            health: "healthy",
            install: { ...status().install!, installed_files: 11, installed_version: "1.0.0" },
          }),
        );
      }
      return Promise.reject(new Error(`unexpected: ${command}`));
    });
    render(<FirstRunWizard onStartTraining={vi.fn()} onOpenControlCenter={vi.fn()} />);
    await screen.findByText("欢迎使用 XHUP Flow");
    await userEvent.click(screen.getByRole("button", { name: "开始" }));

    expect(await screen.findByText("已安装且状态健康")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "确认安装" })).not.toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "进入应用" }));
    await waitFor(() =>
      expect(window.localStorage.getItem("xhup-flow.onboarding.v1")).toContain("completed"),
    );
  });

  it("有可用更新 → 提供更新计划入口", async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === "product_status") {
        return Promise.resolve(
          status({
            health: "update_available",
            update_available: true,
            install: { ...status().install!, installed_files: 11, installed_version: "0.9.0" },
          }),
        );
      }
      if (command === "product_plan") return Promise.resolve(PLAN);
      return Promise.reject(new Error(`unexpected: ${command}`));
    });
    render(<FirstRunWizard onStartTraining={vi.fn()} onOpenControlCenter={vi.fn()} />);
    await screen.findByText("欢迎使用 XHUP Flow");
    await userEvent.click(screen.getByRole("button", { name: "开始" }));
    expect(await screen.findByText("有可用更新")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "生成安装计划" }));
    expect(await screen.findByText("确认安装计划")).toBeInTheDocument();
  });

  it("Rime 缺失:列出支持客户端,不代装第三方软件", async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === "product_status") {
        return Promise.resolve(status({ rime_detected: false, health: null, install: null }));
      }
      return Promise.reject(new Error(`unexpected: ${command}`));
    });
    render(<FirstRunWizard onStartTraining={vi.fn()} onOpenControlCenter={vi.fn()} />);
    await screen.findByText("欢迎使用 XHUP Flow");
    await userEvent.click(screen.getByRole("button", { name: "开始" }));

    expect(await screen.findByText("尚未检测到 Rime")).toBeInTheDocument();
    expect(screen.getByText(/不会替你下载或安装第三方输入法/)).toBeInTheDocument();
    // 只发起过检测调用,没有任何安装/下载类命令。
    expect(invokeMock.mock.calls.length).toBeGreaterThan(0);
    expect(invokeMock.mock.calls.every((call) => call[0] === "product_status")).toBe(true);
  });

  it("跳过:任何时刻可跳过并持久化 skipped", async () => {
    invokeMock.mockImplementation(() => new Promise(() => {})); // 检测挂起
    render(<FirstRunWizard onStartTraining={vi.fn()} onOpenControlCenter={vi.fn()} />);
    await screen.findByText("欢迎使用 XHUP Flow");
    await userEvent.click(screen.getByRole("button", { name: "跳过,稍后再说" }));
    await waitFor(() =>
      expect(window.localStorage.getItem("xhup-flow.onboarding.v1")).toContain("skipped"),
    );
    await waitFor(() =>
      expect(screen.queryByText("欢迎使用 XHUP Flow")).not.toBeInTheDocument(),
    );
  });

  it("已有引导记录:启动时完全不渲染", async () => {
    writeOnboarding("completed");
    mockFullInstall();
    render(<FirstRunWizard onStartTraining={vi.fn()} onOpenControlCenter={vi.fn()} />);
    await waitFor(() => expect(invokeMock).not.toHaveBeenCalled());
    expect(screen.queryByText("欢迎使用 XHUP Flow")).not.toBeInTheDocument();
  });

  it("重新运行信号:已引导用户可从设置页再次打开向导", async () => {
    writeOnboarding("skipped");
    mockFullInstall();
    const props = { onStartTraining: vi.fn(), onOpenControlCenter: vi.fn() };
    const { rerender } = render(<FirstRunWizard {...props} reopenSignal={0} />);
    await waitFor(() => expect(invokeMock).not.toHaveBeenCalled());
    expect(screen.queryByText("欢迎使用 XHUP Flow")).not.toBeInTheDocument();

    // 设置页入口:清除记录 + 自增信号。
    clearOnboarding();
    rerender(<FirstRunWizard {...props} reopenSignal={1} />);
    await screen.findByText("欢迎使用 XHUP Flow");

    // 再次走完检测(跳过即可),记录重新写入。
    await userEvent.click(screen.getByRole("button", { name: "开始" }));
    await userEvent.click(await screen.findByRole("button", { name: "跳过,稍后再说" }));
    await waitFor(() =>
      expect(window.localStorage.getItem("xhup-flow.onboarding.v1")).toContain("skipped"),
    );
  });
});
