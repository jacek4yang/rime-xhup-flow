/**
 * 启动路径性能基准:数据分片在模块导入时同步走完整运行时校验
 * (validateTrainerDataset + buildTrainerIndex),这是小程序启动的
 * 固定开销。本测试用宽松预算(200ms)守护该成本——数据分片增长
 * 或校验逻辑退化时在 CI 直接暴露,而不是拖慢真机启动。
 *
 * 实测基线(2026-09,10KB 分片,node 26 CI 档):约 1-3ms,
 * 距预算有百倍余量;预算刻意宽松以吸收 CI 抖动。
 */

import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import {
  buildTrainerIndex,
  validateTrainerDataset,
} from "@xhup/trainer-core";
import shard from "../data/generated/dataset.json";

const BUDGET_MS = 200;

describe("数据分片校验性能", () => {
  it(`分片校验 + 索引构建不超过 ${BUDGET_MS}ms(CI 预算)`, () => {
    // 测试总是从 miniapp 包根运行(pnpm --filter)。
    const shardBytes = readFileSync(
      join(process.cwd(), "src", "data", "generated", "dataset.json"),
    ).length;
    const start = performance.now();
    const dataset = validateTrainerDataset(shard);
    buildTrainerIndex(dataset);
    const elapsedMs = performance.now() - start;
    // 输出实测值供 CI 日志观察趋势(确定性断言只用宽松预算)。
    console.info(
      `[perf] dataset validate+index: ${elapsedMs.toFixed(1)}ms / shard ${shardBytes}B / 预算 ${BUDGET_MS}ms`,
    );
    expect(elapsedMs).toBeLessThan(BUDGET_MS);
    expect(dataset.entries.length).toBeGreaterThan(0);
  });
});
