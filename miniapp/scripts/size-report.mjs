/**
 * 小程序包体积报告与门槛:
 *
 * - 主包门槛:微信主包上限 2MB,这里留余量定 1.5MB(无分包,
 *   dist 总计即主包体积);
 * - 单 JS chunk 门槛:200KB(实测最大 taro.js ≈ 130KB,距门槛约
 *   50% 余量;超限应优先考虑 Taro 分包/commonChunks 拆分而不是调门槛);
 * - 分区统计:pages(页面)/ vendor(框架与运行时)/ data(配置与
 *   结构化 JSON)/ app(业务入口与样式等其余主包文件);
 * - `--json`:输出确定性 JSON(键序、文件排序固定,适合 CI 产物对比);
 *   `--out <file>`:同时把 JSON 报告写入文件。
 *
 * 超出任何门槛即退出非零。
 */

import { readdirSync, statSync, writeFileSync } from "node:fs";
import { dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const distDir = resolve(here, "..", "dist");

const LIMITS = {
  /** 主包(= 全部产物,无分包)字节上限。 */
  mainBytes: Math.round(1.5 * 1024 * 1024),
  /** 单个 JS chunk 字节上限。 */
  chunkBytes: Math.round(200 * 1024),
};

/** 框架/运行时 chunk(vendor 区)。 */
const VENDOR_FILES = new Set(["taro.js", "vendors.js", "common.js", "runtime.js", "comp.js"]);

function walk(dir) {
  const files = [];
  for (const name of readdirSync(dir)) {
    const p = join(dir, name);
    const s = statSync(p);
    if (s.isDirectory()) files.push(...walk(p));
    else files.push({ path: relative(distDir, p).replaceAll("\\", "/"), bytes: s.size });
  }
  return files;
}

/** 文件 → 分区:pages / vendor / data / app。 */
function areaOf(path) {
  if (path.startsWith("pages/")) return "pages";
  if (VENDOR_FILES.has(path)) return "vendor";
  if (path.endsWith(".json")) return "data";
  return "app";
}

const args = process.argv.slice(2);
const jsonMode = args.includes("--json");
const outIndex = args.indexOf("--out");
const outPath = outIndex >= 0 ? args[outIndex + 1] : null;

const files = walk(distDir).sort((a, b) => a.path.localeCompare(b.path));
const totalBytes = files.reduce((sum, f) => sum + f.bytes, 0);

// 分区小计(字节与文件数),分区名固定顺序保证 JSON 确定性。
const AREAS = ["pages", "vendor", "data", "app"];
const areas = Object.fromEntries(
  AREAS.map((area) => {
    const group = files.filter((f) => areaOf(f.path) === area);
    return [
      area,
      {
        bytes: group.reduce((sum, f) => sum + f.bytes, 0),
        fileCount: group.length,
      },
    ];
  }),
);

const jsFiles = files
  .filter((f) => f.path.endsWith(".js"))
  .sort((a, b) => b.bytes - a.bytes);

const violations = [];
if (totalBytes > LIMITS.mainBytes) {
  violations.push(
    `主包超限:${totalBytes} > ${LIMITS.mainBytes} bytes`,
  );
}
for (const f of jsFiles) {
  if (f.bytes > LIMITS.chunkBytes) {
    violations.push(`JS chunk 超限:${f.path} ${f.bytes} > ${LIMITS.chunkBytes} bytes`);
  }
}

const report = {
  generatedBy: "miniapp/scripts/size-report.mjs",
  limits: LIMITS,
  totals: { bytes: totalBytes, fileCount: files.length },
  areas,
  largestJs: jsFiles.slice(0, 10).map((f) => ({ path: f.path, bytes: f.bytes })),
  violations,
  ok: violations.length === 0,
};

if (outPath) {
  writeFileSync(resolve(process.cwd(), outPath), `${JSON.stringify(report, null, 2)}\n`);
}

if (jsonMode) {
  console.log(JSON.stringify(report, null, 2));
} else {
  console.log("=== miniapp weapp 体积报告 ===");
  console.log(`主包(dist 总计): ${totalBytes} bytes / 上限 ${LIMITS.mainBytes}`);
  for (const area of AREAS) {
    console.log(`  ${area}: ${areas[area].bytes} bytes (${areas[area].fileCount} 文件)`);
  }
  console.log(`JS 最大: ${jsFiles.slice(0, 5).map((f) => `${f.path}=${f.bytes}`).join(", ")}`);
  console.log(`单 JS chunk 上限: ${LIMITS.chunkBytes}`);
}

if (violations.length > 0) {
  for (const message of violations) console.error(`[size] ${message}`);
  process.exit(1);
}
