import { expect, it } from "vitest";
import { readFileSync } from "node:fs";
import { brotliDecompressSync } from "node:zlib";
import { TrainerDataError } from "@xhup/trainer-core";
import manifest from "../data/generated/dataset-manifest.json";
import { decodeTrainerData } from "./dataset-codec";

const raw = brotliDecompressSync(readFileSync("src/data/generated/trainer.json.br"));
const buffer = (bytes: Uint8Array) => Uint8Array.from(bytes).buffer;

it("损坏编码与同长度非法JSON不能启动训练", () => {
  const corrupt = Buffer.from(raw);
  corrupt[0] = 255;
  expect(() => decodeTrainerData(buffer(corrupt))).toThrow(Error);
  corrupt[0] = "!".charCodeAt(0);
  expect(() => decodeTrainerData(buffer(corrupt))).toThrow(SyntaxError);
});

it("字节数正确也必须经过完整的规范数据校验", () => {
  const invalid = raw.toString("ascii").replace(/("code"\s*:\s*")([a-z])/, (_match, prefix: string, key: string) => prefix + key.toUpperCase());
  expect(() => decodeTrainerData(buffer(Buffer.from(invalid, "ascii")))).toThrow(TrainerDataError);
});

it("同长度但属于另一产品版本的数据被拒绝", () => {
  const version = manifest.source.packageVersion;
  const otherVersion = (version[0] === "9" ? "8" : "9") + version.slice(1);
  const stale = raw.toString("ascii").replace(JSON.stringify(version), JSON.stringify(otherVersion));
  expect(() => decodeTrainerData(buffer(Buffer.from(stale, "ascii")))).toThrow(Error);
});
