/**
 * 平台纯度守卫:共享核心源码不得出现任何宿主平台依赖。
 *
 * 该测试是架构约束的机器可执行版本——任何 import window / document /
 * localStorage / navigator / React / Tauri / wx.* / Taro 的 PR 都会在
 * 这里失败,而不是在某个平台突然炸掉。
 */

import { readFileSync, readdirSync, statSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

const SRC = new URL("./", import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1");

const FORBIDDEN: { pattern: RegExp; reason: string }[] = [
  { pattern: /\bwindow\b/, reason: "window(DOM)" },
  { pattern: /\bdocument\b/, reason: "document(DOM)" },
  { pattern: /\blocalStorage\b/, reason: "localStorage(Web)" },
  { pattern: /\bnavigator\b/, reason: "navigator(Web)" },
  { pattern: /from\s+["']react["']/, reason: "react(视图层)" },
  { pattern: /@tauri-apps/, reason: "Tauri(桌面)" },
  { pattern: /__TAURI_INTERNALS__/, reason: "Tauri IPC" },
  { pattern: /\bwx\.\w/, reason: "wx.*(微信)" },
  { pattern: /from\s+["']@tarojs/, reason: "Taro(小程序)" },
  { pattern: /\bimport\.meta\b/, reason: "import.meta(打包器特有)" },
  { pattern: /\bfetch\(\s*[^)]/, reason: "裸 fetch(须经 TrainerDataFetch 注入)" },
];

function* walk(dir: string): Generator<string> {
  for (const name of readdirSync(dir)) {
    const p = join(dir, name);
    const s = statSync(p);
    if (s.isDirectory()) yield* walk(p);
    else if (name.endsWith(".ts") && !name.endsWith(".test.ts") && name !== "index.ts") {
      yield p;
    }
  }
}

describe("共享核心平台纯度", () => {
  it("源码不含任何平台耦合(import/运行时访问)", () => {
    const offenders: string[] = [];
    for (const file of walk(SRC)) {
      const source = readFileSync(file, "utf8");
      for (const line of source.split("\n")) {
        const trimmed = line.trim();
        // 注释里允许讨论平台差异;只约束可执行代码行。
        if (trimmed.startsWith("*") || trimmed.startsWith("//") || trimmed.startsWith("/*")) {
          continue;
        }
        for (const { pattern, reason } of FORBIDDEN) {
          if (pattern.test(trimmed)) {
            offenders.push(`${file}: ${reason} → ${trimmed}`);
          }
        }
      }
    }
    expect(offenders).toEqual([]);
  });
});
