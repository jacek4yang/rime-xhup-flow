/**
 * 小程序包体积报告:主包与全部分包字节数、最大文件清单。
 * CI 体积门槛集中在 LIMITS,超出即退出非零。
 */

import { readdirSync, statSync } from "node:fs";
import { dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const distDir = resolve(here, "..", "dist");

// 主包平台安全目标(微信主包上限 2MB;留出余量定 1.5MB)。
const LIMITS = { mainBytes: 1.5 * 1024 * 1024 };

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

const files = walk(distDir).sort((a, b) => b.bytes - a.bytes);
const total = files.reduce((sum, f) => sum + f.bytes, 0);

console.log("=== miniapp weapp 体积报告 ===");
console.log(`dist 总计: ${total} bytes`);
const js = files.filter((f) => f.path.endsWith(".js"));
const data = files.filter((f) => f.path.endsWith(".json"));
console.log(`JS 最大: ${js.slice(0, 5).map((f) => `${f.path}=${f.bytes}`).join(", ")}`);
console.log(`JSON 最大: ${data.slice(0, 5).map((f) => `${f.path}=${f.bytes}`).join(", ")}`);
console.log(`主包上限: ${LIMITS.mainBytes} bytes`);

if (total > LIMITS.mainBytes) {
  console.error(`主包超限:${total} > ${LIMITS.mainBytes}`);
  process.exit(1);
}
