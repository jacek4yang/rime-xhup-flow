import { readFileSync } from "node:fs";
import { join } from "node:path";
import { brotliDecompressSync } from "node:zlib";
import { decodeTrainerData } from "../lib/dataset-codec";

/** Tests consume the real production artifact, never a smaller substitute. */
export function readCompiledBuffer(): ArrayBuffer {
  const bytes = brotliDecompressSync(readFileSync(join(process.cwd(), "src/data/generated/trainer.json.br")));
  return bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength) as ArrayBuffer;
}
export const compiledTrainerData = decodeTrainerData(readCompiledBuffer());
export const { dataset, trainerIndex } = compiledTrainerData;
