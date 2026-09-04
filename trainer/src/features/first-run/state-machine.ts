/**
 * 首次启动向导状态机(纯函数,无副作用)。
 *
 * 用类型化状态取代布尔旗标组合;每个状态只允许来自当前步骤的合法
 * 事件,非法转移原样返回(不崩溃、不跳步)。异步 IPC(productApi)
 * 由 React 组件驱动:组件观察状态发命令,把结果/错误作为事件回灌。
 * 业务判断(健康分支、验证门槛)集中在此,组件只做展示。
 */

import type {
  ExecuteResultDto,
  PlanDto,
  ProductStatusDto,
} from "@/lib/product";
import type { I18nKey } from "@/lib/i18n";
import type { PracticeMode } from "@/features/practice/types";

/** 用户在「输入方案说明」步骤的意向(仅用于展示切换指引,两套方案都会安装)。 */
export type SchemaChoice = "flow" | "static";

/** 向导事件(组件把 IPC 结果与用户操作翻译为事件)。 */
export type FirstRunEvent =
  | { type: "START" }
  | { type: "STATUS_RESOLVED"; status: ProductStatusDto }
  | { type: "STATUS_FAILED"; error: unknown }
  | { type: "SCHEMA_CHOSEN"; schema: SchemaChoice }
  | { type: "PLAN_REQUESTED" }
  | { type: "PLAN_RESOLVED"; plan: PlanDto }
  | { type: "PLAN_FAILED"; error: unknown }
  | { type: "CONFIRM_INSTALL" }
  | { type: "INSTALL_RESOLVED"; result: ExecuteResultDto }
  | { type: "INSTALL_FAILED"; error: unknown }
  | { type: "REDEPLOY_RESOLVED" }
  | { type: "REDEPLOY_MANUAL"; error: unknown }
  | { type: "VERIFY_REQUESTED" }
  | { type: "VERIFY_RESOLVED"; status: ProductStatusDto }
  | { type: "VERIFY_FAILED"; error: unknown }
  | { type: "RETRY" }
  | { type: "BACK" }
  | { type: "PROCEED" }
  | { type: "SKIP" }
  | { type: "FINISH" };

/** 终局:仅用于持久化最小引导状态(见 onboarding.ts)。 */
export type FirstRunOutcome = "completed" | "skipped";

export type FirstRunState =
  | { step: "welcome" }
  | { step: "detecting" }
  /** 检测失败(含浏览器环境:IPC 不可用的降级路径)。 */
  | { step: "detect-failed"; error: unknown }
  /** 未检测到 Rime:展示支持的客户端与下一步指引,绝不代装第三方输入法。 */
  | { step: "rime-missing"; status: ProductStatusDto }
  /** 已安装且健康:不进入安装向导,只提供训练与直接进入应用。 */
  | { step: "already-healthy"; status: ProductStatusDto }
  | { step: "offer-update"; status: ProductStatusDto }
  | { step: "offer-repair"; status: ProductStatusDto }
  | { step: "schema-choice"; status: ProductStatusDto }
  | { step: "planning"; status: ProductStatusDto; schema: SchemaChoice }
  | { step: "plan-preview"; status: ProductStatusDto; schema: SchemaChoice; plan: PlanDto }
  | { step: "plan-failed"; status: ProductStatusDto; schema: SchemaChoice; error: unknown }
  | { step: "installing"; status: ProductStatusDto; schema: SchemaChoice; plan: PlanDto }
  | { step: "install-failed"; status: ProductStatusDto; schema: SchemaChoice; plan: PlanDto; error: unknown }
  | { step: "redeploying"; status: ProductStatusDto; schema: SchemaChoice; result: ExecuteResultDto }
  | { step: "redeploy-manual"; status: ProductStatusDto; schema: SchemaChoice; result: ExecuteResultDto; error: unknown }
  | { step: "verifying"; status: ProductStatusDto; schema: SchemaChoice; result: ExecuteResultDto }
  | { step: "verify-failed"; status: ProductStatusDto; schema: SchemaChoice; result: ExecuteResultDto; error: unknown }
  | { step: "input-check"; status: ProductStatusDto; schema: SchemaChoice; result: ExecuteResultDto }
  | { step: "training-offer"; status: ProductStatusDto; schema: SchemaChoice }
  /** 终态:组件据此持久化并关闭(带 training 意图时先跳练习视图)。 */
  | { step: "completed"; outcome: "completed"; startTraining: boolean }
  | { step: "completed"; outcome: "skipped" };

export function initialFirstRunState(): FirstRunState {
  return { step: "welcome" };
}

/** 状态机是否到达终局(组件据此写 onboarding 记录并关闭对话框)。 */
export function isTerminal(
  state: FirstRunState,
): state is Extract<FirstRunState, { step: "completed" }> {
  return state.step === "completed";
}

/** 纯转移函数:非法事件原样返回当前状态。 */
export function transition(state: FirstRunState, event: FirstRunEvent): FirstRunState {
  switch (state.step) {
    case "welcome":
      switch (event.type) {
        case "START":
          return { step: "detecting" };
        case "SKIP":
          return { step: "completed", outcome: "skipped" };
        default:
          return state;
      }
    case "detecting":
      switch (event.type) {
        case "STATUS_RESOLVED":
          return branchByStatus(event.status);
        case "STATUS_FAILED":
          return { step: "detect-failed", error: event.error };
        case "SKIP":
          return { step: "completed", outcome: "skipped" };
        default:
          return state;
      }
    case "detect-failed":
      switch (event.type) {
        case "RETRY":
          return { step: "detecting" };
        case "SKIP":
          return { step: "completed", outcome: "skipped" };
        default:
          return state;
      }
    case "rime-missing":
      switch (event.type) {
        case "RETRY":
          return { step: "detecting" };
        case "BACK":
          return { step: "welcome" };
        case "SKIP":
          return { step: "completed", outcome: "skipped" };
        default:
          return state;
      }
    case "already-healthy":
      switch (event.type) {
        case "PROCEED":
          return { step: "training-offer", status: state.status, schema: "flow" };
        case "FINISH":
          return { step: "completed", outcome: "completed", startTraining: false };
        case "SKIP":
          return { step: "completed", outcome: "skipped" };
        default:
          return state;
      }
    case "offer-update":
    case "offer-repair":
      switch (event.type) {
        case "PLAN_REQUESTED":
          return {
            step: "planning",
            status: state.status,
            schema: "flow",
          };
        case "SKIP":
          return { step: "completed", outcome: "skipped" };
        default:
          return state;
      }
    case "schema-choice":
      switch (event.type) {
        case "SCHEMA_CHOSEN":
          return {
            step: "planning",
            status: state.status,
            schema: event.schema,
          };
        case "SKIP":
          return { step: "completed", outcome: "skipped" };
        default:
          return state;
      }
    case "planning":
      switch (event.type) {
        case "PLAN_RESOLVED":
          return {
            step: "plan-preview",
            status: state.status,
            schema: state.schema,
            plan: event.plan,
          };
        case "PLAN_FAILED":
          return {
            step: "plan-failed",
            status: state.status,
            schema: state.schema,
            error: event.error,
          };
        case "SKIP":
          return { step: "completed", outcome: "skipped" };
        default:
          return state;
      }
    case "plan-preview":
      switch (event.type) {
        case "CONFIRM_INSTALL":
          return {
            step: "installing",
            status: state.status,
            schema: state.schema,
            plan: state.plan,
          };
        case "PLAN_REQUESTED":
          return { step: "planning", status: state.status, schema: state.schema };
        case "BACK":
          return { step: "schema-choice", status: state.status };
        case "SKIP":
          return { step: "completed", outcome: "skipped" };
        default:
          return state;
      }
    case "plan-failed":
      switch (event.type) {
        case "RETRY":
        case "PLAN_REQUESTED":
          return { step: "planning", status: state.status, schema: state.schema };
        case "SKIP":
          return { step: "completed", outcome: "skipped" };
        default:
          return state;
      }
    case "installing":
      switch (event.type) {
        case "INSTALL_RESOLVED":
          return {
            step: "redeploying",
            status: state.status,
            schema: state.schema,
            result: event.result,
          };
        case "INSTALL_FAILED":
          return {
            step: "install-failed",
            status: state.status,
            schema: state.schema,
            plan: state.plan,
            error: event.error,
          };
        case "SKIP":
          return { step: "completed", outcome: "skipped" };
        default:
          return state;
      }
    case "install-failed":
      switch (event.type) {
        case "RETRY":
          return {
            step: "installing",
            status: state.status,
            schema: state.schema,
            plan: state.plan,
          };
        case "PLAN_REQUESTED":
          return { step: "planning", status: state.status, schema: state.schema };
        case "SKIP":
          return { step: "completed", outcome: "skipped" };
        default:
          return state;
      }
    case "redeploying":
      switch (event.type) {
        case "REDEPLOY_RESOLVED":
          return {
            step: "verifying",
            status: state.status,
            schema: state.schema,
            result: state.result,
          };
        case "REDEPLOY_MANUAL":
          return {
            step: "redeploy-manual",
            status: state.status,
            schema: state.schema,
            result: state.result,
            error: event.error,
          };
        case "SKIP":
          return { step: "completed", outcome: "skipped" };
        default:
          return state;
      }
    case "redeploy-manual":
      switch (event.type) {
        case "PROCEED":
          return {
            step: "verifying",
            status: state.status,
            schema: state.schema,
            result: state.result,
          };
        case "SKIP":
          return { step: "completed", outcome: "skipped" };
        default:
          return state;
      }
    case "verifying":
      switch (event.type) {
        case "VERIFY_RESOLVED":
          // 不因文件复制完成就宣称成功:必须重新检测且健康才放行。
          if (event.status.health === "healthy") {
            return {
              step: "input-check",
              status: event.status,
              schema: state.schema,
              result: state.result,
            };
          }
          return {
            step: "verify-failed",
            status: event.status,
            schema: state.schema,
            result: state.result,
            error: event.status.health,
          };
        case "VERIFY_FAILED":
          return {
            step: "verify-failed",
            status: state.status,
            schema: state.schema,
            result: state.result,
            error: event.error,
          };
        case "SKIP":
          return { step: "completed", outcome: "skipped" };
        default:
          return state;
      }
    case "verify-failed":
      switch (event.type) {
        case "RETRY":
        case "VERIFY_REQUESTED":
          return {
            step: "verifying",
            status: state.status,
            schema: state.schema,
            result: state.result,
          };
        case "SKIP":
          return { step: "completed", outcome: "skipped" };
        default:
          return state;
      }
    case "input-check":
      switch (event.type) {
        case "PROCEED":
          return {
            step: "training-offer",
            status: state.status,
            schema: state.schema,
          };
        case "SKIP":
          return { step: "completed", outcome: "skipped" };
        default:
          return state;
      }
    case "training-offer":
      switch (event.type) {
        case "PROCEED":
          return { step: "completed", outcome: "completed", startTraining: true };
        case "FINISH":
          return { step: "completed", outcome: "completed", startTraining: false };
        case "SKIP":
          return { step: "completed", outcome: "skipped" };
        default:
          return state;
      }
    case "completed":
      return state;
  }
}

/** 按产品状态健康度分支(与控制中心同一状态模型)。 */
function branchByStatus(status: ProductStatusDto): FirstRunState {
  if (!status.rime_detected) {
    return { step: "rime-missing", status };
  }
  switch (status.health) {
    case "healthy":
      return { step: "already-healthy", status };
    case "update_available":
      return { step: "offer-update", status };
    case "modified":
    case "incomplete":
      return { step: "offer-repair", status };
    case "not_installed":
    case null:
      return { step: "schema-choice", status };
  }
}

/** 新手训练路径(复用训练器 V2 既有模式,不新建第二套引擎)。 */
export const STARTER_TRAINING_PATH: readonly {
  mode: PracticeMode;
  label: I18nKey;
}[] = [
  { mode: "double", label: "firstRun.starterDouble" },
  { mode: "sound-shape", label: "firstRun.starterSoundShape" },
  { mode: "full", label: "firstRun.starterFull" },
  { mode: "level1", label: "firstRun.starterLevel1" },
  { mode: "fixed-word", label: "firstRun.starterWord" },
  { mode: "sentence", label: "firstRun.starterSentence" },
];
