/**
 * 首次启动状态机测试:覆盖全新安装、Rime 缺失、已健康、更新/修复、
 * 安装失败、自动/手动部署、验证失败、跳过与完成的全部路径。
 * 状态机是纯函数:所有测试不依赖 IPC 与环境。
 */

import { describe, expect, it } from "vitest";
import type { ProductStatusDto } from "@/lib/product";
import {
  initialFirstRunState,
  transition,
  type FirstRunEvent,
  type FirstRunState,
} from "./state-machine";

const USER_DIR = "/home/tester/.config/fcitx5/rime";

function status(overrides: Partial<ProductStatusDto> = {}): ProductStatusDto {
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
      integrity: [],
    },
    health: "not_installed",
    bundled_version: "1.0.0",
    update_available: false,
    learning: null,
    ...overrides,
  };
}

/** 步骤状态的可读断言助手。 */
function stepOf(state: FirstRunState): string {
  return state.step;
}

function drive(state: FirstRunState, ...events: FirstRunEvent[]): FirstRunState {
  return events.reduce(transition, state);
}

const PLAN = { actions: [{ kind: "write" as const, file: "xhup_flow.schema.yaml" }], notes: [] };
const RESULT = { done: 11, redeploy_guidance: "重启 Fcitx5。" };

describe("first-run state machine", () => {
  it("从欢迎进入检测,状态解析按健康分支", () => {
    expect(stepOf(drive(initialFirstRunState(), { type: "START" }))).toBe("detecting");
    expect(
      stepOf(
        drive(initialFirstRunState(), { type: "START" }, { type: "STATUS_RESOLVED", status: status() }),
      ),
    ).toBe("schema-choice");
  });

  it("Rime 未检测到 → rime-missing,重试回检测", () => {
    const missing = status({ rime_detected: false, health: null, install: null });
    const state = drive(
      initialFirstRunState(),
      { type: "START" },
      { type: "STATUS_RESOLVED", status: missing },
    );
    expect(stepOf(state)).toBe("rime-missing");
    expect(stepOf(transition(state, { type: "RETRY" }))).toBe("detecting");
    expect(stepOf(transition(state, { type: "BACK" }))).toBe("welcome");
  });

  it("已健康 → 不进安装向导,可直接进入应用或转训练", () => {
    const healthy = status({
      health: "healthy",
      install: { ...status().install!, installed_files: 11, installed_version: "1.0.0" },
    });
    const state = drive(
      initialFirstRunState(),
      { type: "START" },
      { type: "STATUS_RESOLVED", status: healthy },
    );
    expect(stepOf(state)).toBe("already-healthy");
    const finish = transition(state, { type: "FINISH" });
    expect(finish).toEqual({ step: "completed", outcome: "completed", startTraining: false });
    const offer = transition(state, { type: "PROCEED" });
    expect(stepOf(offer)).toBe("training-offer");
  });

  it("update_available → offer-update;modified → offer-repair", () => {
    const upd = drive(
      initialFirstRunState(),
      { type: "START" },
      { type: "STATUS_RESOLVED", status: status({ health: "update_available", update_available: true }) },
    );
    expect(stepOf(upd)).toBe("offer-update");
    const rep = drive(
      initialFirstRunState(),
      { type: "START" },
      { type: "STATUS_RESOLVED", status: status({ health: "modified" }) },
    );
    expect(stepOf(rep)).toBe("offer-repair");
    expect(stepOf(transition(upd, { type: "PLAN_REQUESTED" }))).toBe("planning");
  });

  it("安装主路径:选方案 → 计划 → 确认 → 执行 → 自动部署 → 验证 → 试打 → 训练", () => {
    let state = drive(
      initialFirstRunState(),
      { type: "START" },
      { type: "STATUS_RESOLVED", status: status() },
    );
    state = transition(state, { type: "SCHEMA_CHOSEN", schema: "flow" });
    expect(state).toMatchObject({ step: "planning", schema: "flow" });
    state = transition(state, { type: "PLAN_RESOLVED", plan: PLAN });
    expect(stepOf(state)).toBe("plan-preview");
    state = transition(state, { type: "CONFIRM_INSTALL" });
    expect(stepOf(state)).toBe("installing");
    state = transition(state, { type: "INSTALL_RESOLVED", result: RESULT });
    expect(stepOf(state)).toBe("redeploying");
    state = transition(state, { type: "REDEPLOY_RESOLVED" });
    expect(stepOf(state)).toBe("verifying");
    state = transition(state, { type: "VERIFY_RESOLVED", status: status({ health: "healthy" }) });
    expect(stepOf(state)).toBe("input-check");
    state = transition(state, { type: "PROCEED" });
    expect(stepOf(state)).toBe("training-offer");
    state = transition(state, { type: "PROCEED" });
    expect(state).toEqual({ step: "completed", outcome: "completed", startTraining: true });
  });

  it("验证不健康 → verify-failed,不宣称成功;重试回验证", () => {
    let state = drive(
      initialFirstRunState(),
      { type: "START" },
      { type: "STATUS_RESOLVED", status: status() },
      { type: "SCHEMA_CHOSEN", schema: "static" },
      { type: "PLAN_RESOLVED", plan: PLAN },
      { type: "CONFIRM_INSTALL" },
      { type: "INSTALL_RESOLVED", result: RESULT },
      { type: "REDEPLOY_RESOLVED" },
    );
    state = transition(state, {
      type: "VERIFY_RESOLVED",
      status: status({ health: "incomplete" }),
    });
    expect(stepOf(state)).toBe("verify-failed");
    expect(stepOf(transition(state, { type: "RETRY" }))).toBe("verifying");
  });

  it("自动部署不可用 → 手动指引 → 用户确认后仍验证", () => {
    let state = drive(
      initialFirstRunState(),
      { type: "START" },
      { type: "STATUS_RESOLVED", status: status() },
      { type: "SCHEMA_CHOSEN", schema: "flow" },
      { type: "PLAN_RESOLVED", plan: PLAN },
      { type: "CONFIRM_INSTALL" },
      { type: "INSTALL_RESOLVED", result: RESULT },
    );
    state = transition(state, { type: "REDEPLOY_MANUAL", error: new Error("redeploy") });
    expect(stepOf(state)).toBe("redeploy-manual");
    expect(stepOf(transition(state, { type: "PROCEED" }))).toBe("verifying");
  });

  it("安装失败 → 重试回 installing,或重新生成计划", () => {
    let state = drive(
      initialFirstRunState(),
      { type: "START" },
      { type: "STATUS_RESOLVED", status: status() },
      { type: "SCHEMA_CHOSEN", schema: "flow" },
      { type: "PLAN_RESOLVED", plan: PLAN },
      { type: "CONFIRM_INSTALL" },
      { type: "INSTALL_FAILED", error: new Error("io") },
    );
    expect(stepOf(state)).toBe("install-failed");
    expect(stepOf(transition(state, { type: "RETRY" }))).toBe("installing");
    expect(stepOf(transition(state, { type: "PLAN_REQUESTED" }))).toBe("planning");
  });

  it("任何非终局步骤都可跳过并落在终局 skipped", () => {
    const midStates: FirstRunState[] = [
      { step: "detecting" },
      { step: "detect-failed", error: new Error("x") },
      { step: "rime-missing", status: status() },
      { step: "schema-choice", status: status() },
      { step: "planning", status: status(), schema: "flow" },
      { step: "installing", status: status(), schema: "flow", plan: PLAN },
      { step: "verifying", status: status(), schema: "flow", result: RESULT },
      { step: "training-offer", status: status(), schema: "flow" },
    ];
    for (const state of midStates) {
      expect(transition(state, { type: "SKIP" })).toEqual({ step: "completed", outcome: "skipped" });
    }
    expect(transition(initialFirstRunState(), { type: "SKIP" })).toEqual({
      step: "completed",
      outcome: "skipped",
    });
  });

  it("检测失败 → 浏览器降级路径可重试", () => {
    const state = drive(
      initialFirstRunState(),
      { type: "START" },
      { type: "STATUS_FAILED", error: new Error("desktop_unavailable") },
    );
    expect(stepOf(state)).toBe("detect-failed");
    expect(stepOf(transition(state, { type: "RETRY" }))).toBe("detecting");
  });

  it("非法事件不改变状态(不崩溃、不跳步)", () => {
    const welcome = initialFirstRunState();
    expect(transition(welcome, { type: "CONFIRM_INSTALL" })).toBe(welcome);
    expect(transition(welcome, { type: "STATUS_RESOLVED", status: status() })).toBe(welcome);
    const done: FirstRunState = { step: "completed", outcome: "skipped" };
    expect(transition(done, { type: "START" })).toBe(done);
  });
});
