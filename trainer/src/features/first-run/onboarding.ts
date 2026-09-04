/**
 * 首次启动引导的最小持久化:只记录「完成/跳过」一个事实。
 *
 * 不存任何安装细节或用户内容;解析失败按未引导处理(宁可再走一次
 * 向导,也不因脏数据阻塞应用);隐私模式下写入失败静默降级为
 * 本会话内不重复弹出(由调用方决定)。
 */

export type OnboardingStatus = "completed" | "skipped";

export interface OnboardingRecord {
  status: OnboardingStatus;
  /** ISO 8601 时间戳;仅用于设置页展示,不参与逻辑。 */
  at: string;
}

const STORAGE_KEY = "xhup-flow.onboarding.v1";

function storage(): Storage | null {
  if (typeof window === "undefined") return null;
  try {
    return window.localStorage;
  } catch {
    // 某些嵌入环境访问 localStorage 直接抛错。
    return null;
  }
}

/** 读取引导记录;无记录、损坏或形状不符一律返回 null。 */
export function readOnboarding(): OnboardingRecord | null {
  const store = storage();
  if (!store) return null;
  try {
    const raw = store.getItem(STORAGE_KEY);
    if (!raw) return null;
    const parsed: unknown = JSON.parse(raw);
    if (typeof parsed !== "object" || parsed === null) return null;
    const shape = parsed as { status?: unknown; at?: unknown };
    if (
      (shape.status === "completed" || shape.status === "skipped") &&
      typeof shape.at === "string"
    ) {
      return { status: shape.status, at: shape.at };
    }
    return null;
  } catch {
    return null;
  }
}

/** 写入引导记录;存储不可用时静默失败并报告未持久化。 */
export function writeOnboarding(status: OnboardingStatus): boolean {
  const store = storage();
  if (!store) return false;
  const record: OnboardingRecord = { status, at: new Date().toISOString() };
  try {
    store.setItem(STORAGE_KEY, JSON.stringify(record));
    return true;
  } catch {
    return false;
  }
}
