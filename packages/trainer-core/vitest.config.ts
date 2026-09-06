import { defineConfig } from "vitest/config";

// 共享核心是平台中立的纯 TS:测试跑在 node 环境,不引入 jsdom——
// 任何测试若需要 DOM,说明核心被平台耦合污染了,应当失败。
export default defineConfig({
  test: {
    environment: "node",
    include: ["src/**/*.test.ts"],
  },
});
