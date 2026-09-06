import { defineConfig } from "vitest/config";

// 小程序测试:语义层跑 node 环境(核心共享测试 + 分片校验 +
// 5 题练习全流程),不依赖 DOM;真机行为由 DevTools 验证。
export default defineConfig({
  test: {
    environment: "node",
    include: ["src/**/*.test.ts", "src/**/*.test.tsx"],
  },
});
