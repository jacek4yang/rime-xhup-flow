import { createContext, useContext, useEffect, useMemo, useState } from "react";
import type { ComponentType, PropsWithChildren } from "react";
import { Text, View } from "@tarojs/components";
import { loadTrainerData } from "./dataset";
import type { TrainerData } from "./dataset-codec";
import { useI18n } from "./i18n";

type DataState = { data: TrainerData | null; error: string | null; retry: () => void };
const DataContext = createContext<DataState | null>(null);

export function useTrainerData(): TrainerData {
  const state = useContext(DataContext);
  if (!state?.data) throw new Error("Trainer data is not loaded");
  return state.data;
}

/** Keep Taro's native page roots mounted; the App itself has no visible page. */
export function TrainerDataProvider({ children }: PropsWithChildren) {
  const [data, setData] = useState<TrainerData | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [attempt, setAttempt] = useState(0);
  useEffect(() => {
    let active = true;
    setError(null);
    void loadTrainerData().then(
      (loaded) => { if (active) setData(loaded); },
      (failure: unknown) => {
        if (active) setError(failure instanceof Error ? failure.message : String(failure));
      },
    );
    return () => { active = false; };
  }, [attempt]);
  const state = useMemo(() => ({ data, error, retry: () => setAttempt((value) => value + 1) }), [data, error]);
  return <DataContext.Provider value={state}>{children}</DataContext.Provider>;
}

/** Loading/error UI must live inside the native page, before its practice hooks mount. */
export function withTrainerData<P extends object>(Page: ComponentType<P>): ComponentType<P> {
  return function TrainerDataPage(props: P) {
    const state = useContext(DataContext);
    const { t } = useI18n();
    if (!state) throw new Error("Trainer data provider is missing");
    if (state.data) return <Page {...props} />;
    return <View className="page">
      <Text className="title">{t(state.error ? "app.loadFailed" : "app.loading")}</Text>
      {state.error && <>
        <View className="subtitle" role="alert"><Text>{state.error}</Text></View>
        <View className="button" role="button" onClick={state.retry}>
          {t("common.retry")}
        </View>
      </>}
    </View>;
  };
}
