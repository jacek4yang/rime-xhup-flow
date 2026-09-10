/**
 * 生成小程序数据分片:从 Rust 产出的规范训练数据集中确定性截取
 * 小程序启动/练习所需的最小子集。
 *
 * 唯一事实来源仍是 Rust(`pnpm -C trainer generate:data` 的产物);
 * 本脚本只做确定性切片,绝不手写/猜测任何编码、读音或词码。
 * 输出:src/data/generated/dataset.json(schemaVersion 4,可被
 * @xhup/trainer-core 的 validateTrainerDataset 完整校验)。
 */

import { readFileSync, writeFileSync, mkdirSync, statSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(here, "..", "..");
const sourcePath =
  process.env.TRAINER_DATA ??
  join(repoRoot, "trainer", "public", "generated", "xhup_flow_trainer.json");
const outPath = resolve(here, "..", "src", "data", "generated", "dataset.json");

const raw = readFileSync(sourcePath, "utf8");
const dataset = JSON.parse(raw);

if (dataset.schemaVersion !== 4) {
  throw new Error(`期望规范数据 schemaVersion 4,实际 ${dataset.schemaVersion}`);
}

// 频率降序为唯一排序键;相同分数保持原始顺序(稳定切片)。
const byFrequency = (a, b) =>
  b.frequencyScore - a.frequencyScore ||
  (a.code < b.code ? -1 : a.code > b.code ? 1 : 0);
const byWeight = (a, b) =>
  b.rimeWeight - a.rimeWeight || (a.code < b.code ? -1 : a.code > b.code ? 1 : 0);

const shard = {
  schemaVersion: dataset.schemaVersion,
  packageVersion: dataset.packageVersion,
  // 启动级单字:高频 2 码 20 + 3 码 8 + 4 码 4(教学演示足够,主包最小)。
  entries: [
    ...dataset.entries.filter((e) => e.length === 2).sort(byFrequency).slice(0, 20),
    ...dataset.entries.filter((e) => e.length === 3).sort(byFrequency).slice(0, 8),
    ...dataset.entries.filter((e) => e.length === 4).sort(byFrequency).slice(0, 4),
  ],
  words: [...dataset.words].sort(byWeight).slice(0, 8),
  level1Shortcuts: dataset.level1Shortcuts,
  primaryShortcuts: [
    ...dataset.primaryShortcuts.filter((entry) => entry.shortcutCode.length > 2).slice(0, 20),
    ...dataset.primaryShortcuts.filter((entry) => entry.shortcutCode.length === 2).slice(0, 10),
  ],
  fixedFirstShortcuts: [...dataset.fixedFirstShortcuts].sort(byWeight).slice(0, 5),
  sentences: dataset.sentences,
  doublePinyin: dataset.doublePinyin,
};

mkdirSync(dirname(outPath), { recursive: true });
writeFileSync(outPath, JSON.stringify(shard, null, 2) + "\n");

const bytes = statSync(outPath).size;
console.log(
  `miniapp dataset shard: ${bytes} bytes ` +
    `(entries=${shard.entries.length} words=${shard.words.length} ` +
    `primaryShortcuts=${shard.primaryShortcuts.length} sentences=${shard.sentences.length})`,
);
if (bytes > 200 * 1024) {
  throw new Error(`启动分片超过 200KB 上限:${bytes} bytes`);
}
