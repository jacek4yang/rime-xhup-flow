#!/usr/bin/env node
/**
 * 从 Unicode Unihan 数据库(kMandarin 字段)导入带调拼音,生成
 * `pinyin_tone.tsv`(字 → 带调普通话读音)。
 *
 * 数据来源与许可见同目录 README.md。运行方式(需先下载 Unihan.zip
 * 并解出 Unihan_Readings.txt):
 *
 *   node data/education/import_unihan_tone.mjs <Unihan_Readings.txt> \
 *     <data/hanzi/readings.tsv> <data/education/pinyin_tone.tsv>
 *
 * 确定性:输出按 readings.tsv 的字符顺序排列;每个字取 kMandarin 的
 * 第一个读音(Unicode 的 zh-Hans 默认读音)。多音字的其他读音本子集
 * 不包含——展示层必须如实标注「规范读音」,不得自称覆盖全部读音。
 */

import { readFileSync, writeFileSync } from "node:fs";

const [readingsPath, sourcePath, outputPath] = process.argv.slice(2);
if (!readingsPath || !sourcePath || !outputPath) {
  console.error("用法: import_unihan_tone.mjs <Unihan_Readings.txt> <readings.tsv> <output.tsv>");
  process.exit(1);
}

// 1) 解析 Unihan kMandarin:U+XXXX\tkMandarin\tpīn(可能有逗号分隔多个)。
const kMandarin = new Map();
let unicodeVersion = "unknown";
for (const line of readFileSync(readingsPath, "utf8").split("\n")) {
  if (line.startsWith("# Unicode Version")) {
    unicodeVersion = line.replace("# Unicode Version", "").trim();
    continue;
  }
  if (line.startsWith("#") || line.trim() === "") continue;
  const [code, field, value] = line.split("\t");
  if (field !== "kMandarin" || !value) continue;
  const char = String.fromCodePoint(parseInt(code.replace("U+", ""), 16));
  if (!kMandarin.has(char)) {
    kMandarin.set(char, value.split(" ")[0].trim());
  }
}

// 2) 以仓库规范读音表的字符集为准,生成子集。
const lines = [];
let covered = 0;
let total = 0;
for (const line of readFileSync(sourcePath, "utf8").split("\n")) {
  if (line.trim() === "") continue;
  const char = line.split("\t")[0];
  if (!char || total > 0 && lines.some((existing) => existing.startsWith(`${char}\t`))) continue;
  const seen = new Set(lines.map((existing) => existing.split("\t")[0]));
  if (seen.has(char)) continue;
  total += 1;
  const tone = kMandarin.get(char);
  if (tone) {
    covered += 1;
    lines.push(`${char}\t${tone}`);
  }
}

writeFileSync(outputPath, lines.join("\n") + "\n", "utf8");
console.log(
  `Unihan ${unicodeVersion}: ${covered}/${total} 字有 kMandarin 读音 (${((covered / total) * 100).toFixed(1)}%) → ${outputPath}`,
);
