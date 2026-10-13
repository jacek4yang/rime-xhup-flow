/** Preserve the existing validate+index budget. Parsing/decompression are measured separately. */
import { describe, expect, it } from "vitest";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { brotliDecompressSync } from "node:zlib";
import { buildTrainerIndex, validateTrainerDataset } from "@xhup/trainer-core";

const BUDGET_MS = 200;
describe("完整规范数据校验与索引性能", () => {
  it(`完整数据校验和索引不超过 ${BUDGET_MS}ms(既有CI预算)`, () => {
    const compressed = readFileSync(join(process.cwd(), "src/data/generated/trainer.json.br"));
    // The former JSON import also occurred outside this budget. This is not a
    // claim that decompression, parsing, rendering or device startup take 200ms.
    const raw = JSON.parse(brotliDecompressSync(compressed).toString("ascii"));
    const start = performance.now();
    buildTrainerIndex(validateTrainerDataset(raw));
    const elapsedMs = performance.now() - start;
    console.info(`[perf] full dataset validate+index: ${elapsedMs.toFixed(1)}ms; budget=${BUDGET_MS}ms; excludes decode/parse/render`);
    expect(elapsedMs).toBeLessThan(BUDGET_MS);
  });
});
