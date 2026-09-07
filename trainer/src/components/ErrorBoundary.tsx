/**
 * 应用级错误边界:捕获子树渲染崩溃,展示恢复卡片。
 *
 * 只做 UI 恢复,绝不触碰持久化用户状态(localStorage / trainer store):
 * - 「重试」与「回到今日」都通过 key 自增重挂载子树——AppShell 重挂载后
 *   内部视图状态回到默认的「今日」,等效于安全退出到仪表盘。
 * - 「复制错误信息」走 navigator.clipboard,环境不支持时静默降级。
 */

import { Component, useState, type ReactNode } from "react";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { useI18n } from "@/lib/use-i18n";

interface ErrorBoundaryProps {
  children: ReactNode;
}

interface ErrorBoundaryState {
  error: Error | null;
  /** 子树重挂载键:自增即丢弃旧子树、重挂载(回到默认视图)。 */
  resetKey: number;
}

/** 类组件无法用 hook,恢复卡片拆成函数组件以使用 useI18n。 */
function RecoveryCard({
  error,
  onRetry,
  onBackToToday,
}: {
  error: Error;
  onRetry: () => void;
  onBackToToday: () => void;
}) {
  const { t } = useI18n();
  const [copied, setCopied] = useState(false);

  const copyError = async () => {
    // clipboard 仅在安全上下文可用;失败时静默降级,不阻塞恢复流程。
    try {
      if (typeof navigator === "undefined" || !navigator.clipboard) return;
      await navigator.clipboard.writeText(error.stack ?? error.message);
      setCopied(true);
    } catch {
      // 剪贴板不可写(权限/非安全上下文):保持原状即可。
    }
  };

  return (
    <main className="flex min-h-svh items-center justify-center bg-background p-6 text-foreground">
      <Card className="w-full max-w-md">
        <CardHeader>
          <CardTitle>{t("errorBoundary.title")}</CardTitle>
          <CardDescription>{error.message}</CardDescription>
        </CardHeader>
        <CardContent className="flex flex-col gap-2">
          <Button onClick={onRetry}>{t("errorBoundary.retry")}</Button>
          <div className="flex gap-2">
            <Button variant="outline" className="flex-1" onClick={copyError}>
              {copied ? t("errorBoundary.copied") : t("errorBoundary.copy")}
            </Button>
            <Button variant="ghost" className="flex-1" onClick={onBackToToday}>
              {t("errorBoundary.backToToday")}
            </Button>
          </div>
        </CardContent>
      </Card>
    </main>
  );
}

export class ErrorBoundary extends Component<
  ErrorBoundaryProps,
  ErrorBoundaryState
> {
  state: ErrorBoundaryState = { error: null, resetKey: 0 };

  static getDerivedStateFromError(error: unknown): Partial<ErrorBoundaryState> {
    return {
      error: error instanceof Error ? error : new Error(String(error)),
    };
  }

  componentDidCatch(error: unknown) {
    // 仅记录,不清理任何持久化状态。
    console.error("[ErrorBoundary]", error);
  }

  /** 重挂载子树:重试,或回到 AppShell 默认的「今日」视图。 */
  private reset = () => {
    this.setState((state) => ({ error: null, resetKey: state.resetKey + 1 }));
  };

  render() {
    const { error, resetKey } = this.state;
    if (error) {
      return (
        <RecoveryCard
          error={error}
          onRetry={this.reset}
          onBackToToday={this.reset}
        />
      );
    }
    return <div key={resetKey} className="contents">{this.props.children}</div>;
  }
}
