/**
 * 桌面/Android 宿主侧的数据集加载:解析 Vite 基址下的生成产物并把
 * 当前界面语言注入共享核心。核心(`@xhup/trainer-core`)保持平台中立,
 * 所有 Tauri/Web 特定的路径与 fetch 语义都留在这一层。
 */

import { loadTrainerDataset as coreLoad, type TrainerDataset } from "@xhup/trainer-core";
import { useTrainerStore } from "@/stores/trainer-store";

/** 生成数据的默认加载地址(构建管线把数据集产物放到 public/generated)。 */
export const TRAINER_DATA_URL = `${import.meta.env.BASE_URL}generated/xhup_flow_trainer.json`;

/** 加载并校验训练数据集(错误文案使用当前界面语言)。 */
export async function loadTrainerDataset(): Promise<TrainerDataset> {
  return coreLoad({
    url: TRAINER_DATA_URL,
    language: useTrainerStore.getState().language,
  });
}
