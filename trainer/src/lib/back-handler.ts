/**
 * 应用内返回键协调:Android 系统返回手势经 WebView history 触发
 * popstate;本模块让嵌套屏幕(练习会话/设置屏/键位详情)优先消费
 * 返回事件,只有无人认领时才由导航栈逐层上退。
 *
 * 单例注册(应用只有一个壳层);所有方法在浏览器/WebView 环境安全。
 */

export type BackHandler = () => boolean;

let handler: BackHandler | null = null;

/** 注册/清除当前屏幕的返回拦截器(组件卸载时务必清除)。 */
export function registerBackHandler(next: BackHandler | null): void {
  handler = next;
}

/** 询问当前屏幕是否消费本次返回;无拦截器时返回 false。 */
export function consumeBack(): boolean {
  return handler ? handler() : false;
}

/** 仅供测试:清空注册(避免用例间串扰)。 */
export function resetBackHandler(): void {
  handler = null;
}
