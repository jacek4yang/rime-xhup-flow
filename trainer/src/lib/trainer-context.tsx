/**
 * 规范训练数据索引的 React 上下文。
 *
 * 数据集是不可变运行时数据:加载校验一次、建索引一次,通过 context
 * 分发给各视图;它不进入 zustand 持久化状态。
 */

import { createContext, useContext, type ReactNode } from "react";
import type { TrainerIndex } from "./trainer-index";

const TrainerIndexContext = createContext<TrainerIndex | null>(null);

export function TrainerIndexProvider({
  index,
  children,
}: {
  index: TrainerIndex;
  children: ReactNode;
}) {
  return (
    <TrainerIndexContext.Provider value={index}>
      {children}
    </TrainerIndexContext.Provider>
  );
}

export function useTrainerIndex(): TrainerIndex {
  const index = useContext(TrainerIndexContext);
  if (!index) {
    throw new Error("useTrainerIndex 必须在 TrainerIndexProvider 内使用");
  }
  return index;
}
