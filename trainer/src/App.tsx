import { useCallback, useEffect, useMemo, useState } from "react";
import { MotionConfig } from "motion/react";
import { Button } from "@/components/ui/button";
import { AppShell } from "@/components/AppShell";
import { ErrorBoundary } from "@/components/ErrorBoundary";
import { TrainerIndexProvider } from "@/lib/trainer-context";
import {
  loadTrainerDataset,
  TrainerDataError,
  type TrainerDataset,
} from "@/lib/trainer-data";
import { buildTrainerIndex } from "@/lib/trainer-index";
import { applyThemeToDocument, onSystemThemeChange } from "@/lib/theme";
import { translate, type I18nKey } from "@/lib/i18n";
import { useTrainerStore } from "@/stores/trainer-store";

type LoadState =
  | { status: "loading" }
  | { status: "error"; reason: string }
  | { status: "ready"; dataset: TrainerDataset };

// 加载/错误屏在 React 上下文与数据就绪之前渲染,直接读 store 语言并 translate。
function currentLanguage() {
  return useTrainerStore.getState().language;
}

function t(key: I18nKey): string {
  return translate(currentLanguage(), key);
}

function errorReason(error: unknown): string {
  if (error instanceof TrainerDataError) return error.message;
  return t("app.unknownError");
}

export default function App() {
  const theme = useTrainerStore((state) => state.theme);
  const [loadState, setLoadState] = useState<LoadState>({ status: "loading" });

  // 主题控制器:跟随偏好;"system" 时监听系统主题变化。
  useEffect(() => {
    applyThemeToDocument(theme);
    if (theme !== "system") return;
    return onSystemThemeChange(() => applyThemeToDocument(theme));
  }, [theme]);

  const load = useCallback(() => {
    setLoadState({ status: "loading" });
    loadTrainerDataset()
      .then((dataset) => setLoadState({ status: "ready", dataset }))
      .catch((error: unknown) =>
        setLoadState({ status: "error", reason: errorReason(error) }),
      );
  }, []);

  useEffect(load, [load]);

  if (loadState.status === "loading") {
    return (
      <CenteredScreen>
        <p className="text-sm text-muted-foreground">{t("app.loading")}</p>
      </CenteredScreen>
    );
  }

  if (loadState.status === "error") {
    return (
      <CenteredScreen>
        <div className="flex flex-col items-center gap-3 text-center">
          <h1 className="text-lg font-semibold">{t("app.loadFailed")}</h1>
          <p className="max-w-sm text-sm text-muted-foreground">
            {loadState.reason}
          </p>
          <Button onClick={load}>{t("common.retry")}</Button>
        </div>
      </CenteredScreen>
    );
  }

  return (
    <MotionConfig reducedMotion="user">
      {/* 仅包裹数据就绪后的应用;数据加载/错误屏有独立的重试路径。 */}
      <ErrorBoundary>
        <ReadyApp dataset={loadState.dataset} />
      </ErrorBoundary>
    </MotionConfig>
  );
}

function CenteredScreen({ children }: { children: React.ReactNode }) {
  return (
    <main className="flex min-h-svh items-center justify-center bg-background p-6 text-foreground">
      {children}
    </main>
  );
}

function ReadyApp({ dataset }: { dataset: TrainerDataset }) {
  // 26753 条数据只校验、建索引一次;后续渲染不再全量 filter/sort。
  const index = useMemo(() => buildTrainerIndex(dataset), [dataset]);
  return (
    <TrainerIndexProvider index={index}>
      <AppShell />
    </TrainerIndexProvider>
  );
}
