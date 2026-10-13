import { buildTrainerIndex, validateTrainerDataset } from "@xhup/trainer-core";
import type { TrainerDataset, TrainerIndex } from "@xhup/trainer-core";
import manifest from "../data/generated/dataset-manifest.json";

export interface TrainerData {
  dataset: TrainerDataset;
  trainerIndex: TrainerIndex;
}

/** The build emits ASCII JSON so this needs neither TextDecoder nor a polyfill. */
export function decodeTrainerData(buffer: ArrayBuffer): TrainerData {
  if (manifest.format !== 1 || manifest.encoding !== "ascii-json-brotli"
    || buffer.byteLength !== manifest.artifact.decodedBytes
    || buffer.byteLength > 16 * 1024 * 1024) {
    throw new Error("训练数据格式或长度不匹配，请重新安装小程序。");
  }
  const bytes = new Uint8Array(buffer);
  const chunks: string[] = [];
  for (let offset = 0; offset < bytes.length; offset += 8192) {
    const chunk = bytes.subarray(offset, offset + 8192);
    for (let i = 0; i < chunk.length; i++) {
      if (chunk[i] > 127) throw new Error("训练数据编码损坏，请重新安装小程序。");
    }
    chunks.push(Reflect.apply(String.fromCharCode, null, chunk));
  }
  const dataset = validateTrainerDataset(JSON.parse(chunks.join("")));
  if (dataset.packageVersion !== manifest.source.packageVersion) {
    throw new Error("训练数据版本不匹配，请重新安装小程序。");
  }
  return { dataset, trainerIndex: buildTrainerIndex(dataset) };
}