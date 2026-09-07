import { resolve } from "node:path";
import { defineConfig } from "@tarojs/cli";

// 微信小程序构建配置。主包只允许极小的启动数据分片;完整数据集
// 仍在桌面端(trainer)由 Rust 生成,这里通过 scripts/generate-shards.mjs
// 从规范数据集确定性截取。
export default defineConfig(async (merge) => {
  const baseConfig = {
    projectName: "xhup-flow-miniapp",
    date: "2026-9-6",
    designWidth: 750,
    deviceRatio: {
      640: 2.34 / 2,
      750: 1,
      828: 1.81 / 2,
    },
    sourceRoot: "src",
    outputRoot: "dist",
    plugins: [],
    defineConstants: {},
    copy: { patterns: [], options: {} },
    framework: "react",
    compiler: "webpack5",
    mini: {
      // 共享核心以 TS 源码直接进 bundle:官方扩展点是 compile.include
      // (脚本规则默认只包含 src 与 Taro 自身的 node_modules)。
      compile: {
        include: [resolve(__dirname, "..", "..", "packages", "trainer-core")],
      },
      postcss: {
        pxtransform: {
          enable: true,
          config: {},
        },
        cssModules: {
          enable: false,
          config: {
            namingPattern: "module_[name]_[local]",
            generateScopedName: "[name]__[local]",
          },
        },
      },
    },
    h5: {},
  };
  return merge({}, baseConfig, {});
});
