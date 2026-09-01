import { useCallback, useEffect, useMemo, useState } from "react";
import { MotionConfig } from "motion/react";
import { Button } from "@/components/ui/button";
import { AppShell } from "@/components/AppShell";
import { TrainerIndexProvider } from "@/lib/trainer-context";
import {
  loadTrainerDataset,
  TrainerDataError,
  type TrainerDataset,
} from "@/lib/trainer-data";
import { buildTrainerIndex } from "@/lib/trainer-index";
import { applyThemeToDocument, onSystemThemeChange } from "@/lib/theme";
import { useTrainerStore } from "@/stores/trainer-store";

type LoadState =
  | { status: "loading" }
  | { status: "error"; reason: string }
  | { status: "ready"; dataset: TrainerDataset };

function errorReason(error: unknown): string {
  if (error instanceof TrainerDataError) return error.message;
  return "发生未知错误";
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
        <p className="text-sm text-muted-foreground">正在加载训练数据…</p>
      </CenteredScreen>
    );
  }

  if (loadState.status === "error") {
    return (
      <CenteredScreen>
        <div className="flex flex-col items-center gap-3 text-center">
          <h1 className="text-lg font-semibold">训练数据加载失败</h1>
          <p className="max-w-sm text-sm text-muted-foreground">
            {loadState.reason}
          </p>
          <Button onClick={load}>重试</Button>
        </div>
      </CenteredScreen>
    );
  }

  return (
    <MotionConfig reducedMotion="user">
      <ReadyApp dataset={loadState.dataset} />
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
