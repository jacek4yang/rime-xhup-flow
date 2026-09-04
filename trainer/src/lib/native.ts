/**
 * 桌面 IPC 边界:应用内唯一接触 Tauri 运行时的模块。
 *
 * React 组件不得直接接触 Tauri 内部;所有命令经本模块类型化转发,
 * 浏览器环境(纯 Web 构建)返回可识别的降级错误而非崩溃。
 *
 * 关于依赖选择的说明:官方 `@tauri-apps/api` 的 `invoke` 就是对
 * `window.__TAURI_INTERNALS__.invoke` 的薄封装(签名一致;本项目命令
 * 参数均为基础类型)。当前开发/CI 环境离线,无法新增 npm 依赖,故
 * 直接调用该稳定入口并集中在此封装;若未来引入官方包,仅需替换
 * `tauriInternals()` 的实现,调用方(产品 API 层)不受影响。
 */

/** Tauri v2 注入的 IPC 入口(测试中以此注入假实现)。 */
export interface TauriInternals {
  invoke: (command: string, args?: Record<string, unknown>) => Promise<unknown>;
}

/** 是否运行在 Tauri 桌面容器内。 */
export function isDesktopApp(): boolean {
  return tauriInternals() !== null;
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

/** 浏览器环境的降级错误(不是桌面应用,或 IPC 不可用)。 */
export class DesktopUnavailableError extends Error {
  readonly code = "desktop_unavailable";
  constructor() {
    super("此操作仅支持桌面应用 / desktop app required");
    this.name = "DesktopUnavailableError";
  }
}

/** 命令错误:Rust 侧的稳定机器码 + 人读兜底消息(见 src-tauri commands.rs)。 */
export class CommandError extends Error {
  readonly code: string;

  constructor(code: string, message: string) {
    super(message);
    this.name = "CommandError";
    this.code = code;
  }
}

/** 把 IPC 拒绝值归一化为 CommandError(对象 {code,message} 或字符串)。 */
function normalizeRejection(cause: unknown): unknown {
  if (cause && typeof cause === "object" && "code" in cause) {
    const shaped = cause as { code?: unknown; message?: unknown };
    if (typeof shaped.code === "string") {
      return new CommandError(
        shaped.code,
        typeof shaped.message === "string" ? shaped.message : String(cause),
      );
    }
  }
  return cause;
}

/** 类型化 invoke:浏览器环境抛 DesktopUnavailableError,命令错误归一
 * 化为 CommandError。 */
export async function invokeDesktop<T>(
  command: string,
  args?: Record<string, unknown>,
): Promise<T> {
  const internals = tauriInternals();
  if (!internals) {
    throw new DesktopUnavailableError();
  }
  try {
    return (await internals.invoke(command, args)) as T;
  } catch (cause: unknown) {
    throw normalizeRejection(cause);
  }
}
