/**
 * 规范数据分片加载:构建时由 scripts/generate-shards.mjs 从 Rust 生成
 * 的规范数据集确定性截取;加载时仍走共享核心的完整运行时校验。
 */

import rawShard from "../data/generated/dataset.json";
import {
  buildTrainerIndex,
  validateTrainerDataset,
  type TrainerDataset,
  type TrainerIndex,
} from "@xhup/trainer-core";

export const dataset: TrainerDataset = validateTrainerDataset(rawShard);
export const trainerIndex: TrainerIndex = buildTrainerIndex(dataset);
