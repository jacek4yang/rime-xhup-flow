/**
 * 控制中心命令契约:对 Tauri 命令的类型化封装(G2)。
 *
 * Rust 侧(`trainer/src-tauri`)是唯一的业务来源;本模块把每个命令
 * 映射为具名函数与结果类型,React 组件不接触 IPC 细节(见
 * `lib/native.ts`)。所有命令错误都是 `CommandError`(稳定 code),
 * 组件按码翻译展示,不解析错误字符串。
 */

import { invokeDesktop } from "@/lib/native";

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

/** 单个拥有文件的完整性(Rust `FileIntegrity`)。 */
export type FileIntegrity = "missing" | "match" | "different";

/** 安装健康分类(Rust `InstallHealth`)。 */
export type InstallHealth =
  | "not_installed"
  | "healthy"
  | "update_available"
  | "modified"
  | "incomplete";

/** 用户数据目录内的安装状态。 */
export interface InstallStatusDto {
  user_data_dir: string;
  client: RimeClient;
  installed_files: number;
  total_files: number;
  missing_files: string[];
  schemas: string[];
  installed_version: string | null;
  /** 每个拥有文件相对随附包的完整性(与 Rust OWNED_FILES 同序)。 */
  integrity: FileIntegrity[];
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
  health: InstallHealth | null;
  learning: LearningSummaryDto | null;
}

/** 计划执行结果。 */
export interface ExecuteResultDto {
  done: number;
  redeploy_guidance: string;
}

/** 维护类型:install 覆盖安装/升级/修复;uninstall 只删拥有文件。 */
export type MaintenanceKind = "install" | "uninstall";

/** 重新部署能力(Rust `RedeploySupport`;自动 = 官方机制,manual = 指引)。 */
export type RedeploySupportDto =
  | { mode: "automatic"; program: string; args: string[] }
  | { mode: "manual" };

/** 学习用户词典身份(破坏性操作的类型化确认值;Rust 侧二次校验)。 */
export const FLOW_USER_DICT_NAME = "xhup_flow_user";

/** 命令错误码集合(与 Rust `commands.rs` 一一对应;用于 i18n 翻译)。 */
export const ERROR_CODES = [
  "rime_not_detected",
  "user_data_dir_missing",
  "user_data_dir_unavailable",
  "package_invalid",
  "package_missing",
  "source_missing",
  "io",
  "dict_manager_missing",
  "snapshot_missing",
  "snapshot_name_mismatch",
  "snapshot_not_produced",
  "user_dict_absent",
  "tool_failed",
  "reset_not_confirmed",
  "reset_failed",
  "desktop_unavailable",
] as const;

export type ErrorCode = (typeof ERROR_CODES)[number];

/**
 * 控制中心桌面 API(与 Rust `trainer/src-tauri/src/commands.rs` 一一
 * 对应;命令名与参数名即 IPC 契约)。
 */
export const productApi = {
  /** 读取完整产品状态(探测 + 完整性 + 学习摘要)。 */
  status: (): Promise<ProductStatusDto> => invokeDesktop("product_status"),
  /** 产出维护计划(dry-run,不落盘)。 */
  plan: (kind: MaintenanceKind): Promise<PlanDto> =>
    invokeDesktop("product_plan", { kind }),
  /** 确认并执行维护(执行前按磁盘现状重新规划)。 */
  execute: (kind: MaintenanceKind): Promise<ExecuteResultDto> =>
    invokeDesktop("product_execute", { kind }),
  /** 生成脱敏诊断报告。 */
  diagnostics: (): Promise<string> => invokeDesktop("product_diagnostics"),
  /** 重新部署输入法(仅官方机制;manual 能力时抛 redeploy_unavailable)。 */
  redeploy: (): Promise<string> => invokeDesktop("product_redeploy"),
  /** 导出 Android 兼容 Rime 包(目录形式),返回导出目录。 */
  exportPackage: (destination: string): Promise<string> =>
    invokeDesktop("product_export_package", { destination }),
  /** 导出学习数据快照,返回快照文件路径。 */
  learningExport: (): Promise<string> => invokeDesktop("learning_export"),
  /** 从快照恢复学习数据。 */
  learningImport: (snapshot: string): Promise<void> =>
    invokeDesktop("learning_import", { snapshot }),
  /** 重置学习数据(破坏性;必须传词典名作类型化二次确认)。 */
  learningReset: (confirmed: boolean): Promise<void> =>
    invokeDesktop("learning_reset", { confirmed, dictName: FLOW_USER_DICT_NAME }),
};
