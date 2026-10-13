import Taro from "@tarojs/taro";
import manifest from "../data/generated/dataset-manifest.json";
import { decodeTrainerData } from "./dataset-codec";
import type { TrainerData } from "./dataset-codec";

// One immutable package dataset; failed reads are not cached, so retry is real.
let loading: Promise<TrainerData> | undefined;
export function loadTrainerData(): Promise<TrainerData> {
  if (!loading) {
    loading = (async () => {
      const files = Taro.getFileSystemManager();
      if (typeof files.readCompressedFile !== "function") {
        throw new Error("当前微信不支持本地 Brotli 数据读取，请更新微信后重试。");
      }
      const data = await new Promise<ArrayBuffer>((resolve, reject) => {
        // Native filesystem managers use callbacks; do not assume the Taro
        // Promise return type is also implemented by the host object.
        const operation = files.readCompressedFile({
          filePath: manifest.artifact.file, compressionAlgorithm: "br",
          success: (result) => {
            if (result.data instanceof ArrayBuffer) resolve(result.data);
            else reject(new Error("无法读取离线训练数据，请重新打开或重新安装小程序。"));
          },
          fail: (error) => reject(new Error(error.errMsg || "离线训练数据读取失败。")),
        });
        void operation?.catch(reject);
      });
      return decodeTrainerData(data);
    })().catch((error: unknown) => {
      loading = undefined;
      throw error;
    });
  }
  return loading;
}
