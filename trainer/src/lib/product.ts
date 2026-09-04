/**
 * 控制中心桌面通道:Tauri v2 IPC 的类型化封装。
 *
 * Rust 侧(`trainer/src-tauri`)是唯一的业务来源;本模块只做类型映射,
 * 不含任何 XHUP 语义。零运行时依赖:直接使用 Tauri 注入的
 * `window.__TAURI_INTERNALS__.invoke`(与官方 `@tauri-apps/api` 的
 * `invoke` 同一 IPC 入口;本项目命令参数均为基础类型,无需额外封装)。
 *
 * 浏览器环境(纯 Web 构建)没有 Tauri IPC,`isDesktopApp()` 返回 false,
 * 控制中心据此展示「需要桌面应用」提示,而不是把调用打成运行时错误。
 */

/** 平台上检测到的 Rime 客户端(Rust `RimeClient` 的序列化形态)。 */
export type RimeClient = "Weasel" | "Squirrel" | "Fcitx5" | "Ibus";

/** 一条计划动作(Rust `PlanAction`,serde tag = kind)。 */
export interface PlanActionDto {
  kind: "write" | "overwrite" | "delete";
  file: string;
  /** 覆盖动作的备份文件路径。 */
  backup?: string;
}

/** 维护计划(dry-run 产物;确认后才执行)。 */
export interface PlanDto {
  actions: PlanActionDto[];
  notes: string[];
}

/** 用户数据目录内的安装状态。 */
export interface InstallStatusDto {
  user_data_dir: string;
  client: RimeClient;
  installed_files: number;
  total_files: number;
  missing_files: string[];
  schemas: string[];
  installed_version: string | null;
}

/** 学习数据摘要(不含任何学习内容)。 */
export interface LearningSummaryDto {
  user_dict: string;
  db_exists: boolean;
  snapshot_available: boolean;
  tool_available: boolean;
}

/** 控制中心完整产品状态。 */
export interface ProductStatusDto {
  client: RimeClient;
  redeploy_guidance: string;
  user_data_dir: string | null;
  rime_detected: boolean;
  install: InstallStatusDto | null;
  bundled_version: string;
  update_available: boolean;
  learning: LearningSummaryDto | null;
}

/** 计划执行结果。 */
export interface ExecuteResultDto {
  done: number;
  redeploy_guidance: string;
}

/** 维护类型:install 覆盖安装/升级/修复;uninstall 只删拥有文件。 */
export type MaintenanceKind = "install" | "uninstall";

/** Tauri v2 注入的 IPC 入口(测试中以此注入假实现)。 */
export interface TauriInternals {
  invoke: (command: string, args?: Record<string, unknown>) => Promise<unknown>;
}

function tauriInternals(): TauriInternals | null {
  if (typeof window === "undefined") return null;
  const internals = (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__ as
    | Partial<TauriInternals>
    | undefined;
  return internals && typeof internals.invoke === "function"
    ? (internals as TauriInternals)
    : null;
}

/** 是否运行在 Tauri 桌面容器内。 */
export function isDesktopApp(): boolean {
  return tauriInternals() !== null;
}

async function invokeDesktop<T>(
  command: string,
  args?: Record<string, unknown>,
): Promise<T> {
  const internals = tauriInternals();
  if (!internals) {
    throw new Error("此操作仅支持桌面应用 / desktop app required");
  }
  return internals.invoke(command, args) as Promise<T>;
}

/** 控制中心桌面 API。 */
export const productApi = {
  status: (): Promise<ProductStatusDto> => invokeDesktop("product_status"),
  plan: (kind: MaintenanceKind): Promise<PlanDto> =>
    invokeDesktop("product_plan", { kind }),
  execute: (kind: MaintenanceKind): Promise<ExecuteResultDto> =>
    invokeDesktop("product_execute", { kind }),
  diagnostics: (): Promise<string> => invokeDesktop("product_diagnostics"),
  learningExport: (): Promise<string> => invokeDesktop("learning_export"),
  learningImport: (snapshot: string): Promise<void> =>
    invokeDesktop("learning_import", { snapshot }),
  learningReset: (confirmed: boolean): Promise<void> =>
    invokeDesktop("learning_reset", { confirmed }),
};
