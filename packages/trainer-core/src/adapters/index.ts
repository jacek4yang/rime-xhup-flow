/**
 * 跨平台宿主能力契约(仅类型,零运行时依赖)。
 *
 * 共享核心不 import `window` / `document` / `localStorage` / `navigator`
 * / React / Tauri / `wx.*`:平台差异一律通过这些契约由宿主注入。
 * 桌面端(Tauri Web)、Android 与微信小程序各自提供实现;实现可以同步
 * 或异步,调用方按 Promise 语义统一处理。
 */

/** 持久化键值存储(桌面/Web:localStorage;小程序:Taro/wx 本地存储)。 */
export interface StorageAdapter {
  get(key: string): Promise<string | null> | string | null;
  set(key: string, value: string): Promise<void> | void;
  remove(key: string): Promise<void> | void;
}

/**
 * 键位触感反馈。语义事件而不是物理强度:宿主若无法区分强度档位,
 * 必须把产品设置如实映射到可用的震动能力,不得假装档位不同。
 */
export interface HapticsAdapter {
  /** 普通按键/轻点。 */
  tap(): void;
  /** 输入错误。 */
  wrong(): void;
  /** 题目完成/会话成功。 */
  success(): void;
}

/** 前后台生命周期(移动端切后台时暂停练习计时等)。 */
export interface LifecycleAdapter {
  onForeground(listener: () => void): () => void;
  onBackground(listener: () => void): () => void;
}

/** 导航(返回栈由宿主维护;核心只表达意图)。 */
export interface NavigationAdapter {
  back(): void;
  push(path: string): void;
  replace(path: string): void;
}
