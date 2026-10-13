import { resolve } from "node:path";
import { execFileSync } from "node:child_process";
import { defineConfig } from "@tarojs/cli";

// Full canonical data is a local Brotli resource, not a JavaScript JSON import.
// Direct Taro invocations must also reject stale or corrupt generated artifacts.
export default defineConfig(async (merge) => {
  execFileSync(process.execPath, [resolve(__dirname, "../scripts/build-dataset.mjs"), "--check"], { stdio: "inherit" });
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
    copy: {
      patterns: [
        { from: "src/data/generated/trainer.json.br", to: "dist/data/trainer.json.br" },
        { from: "src/data/generated/dataset-manifest.json", to: "dist/data/dataset-manifest.json" },
      ],
      options: {},
    },
    framework: "react",
    compiler: "webpack5",
    // Exercise the awaited, maintained XML minifier in production builds.
    minifyXML: { collapseWhitespace: true },
    mini: {
      // Taro 4.2 webpackbar passes obsolete presentation options to Webpack.
      // Remove only the progress UI, not validation, optimization or build checks.
      webpackChain(chain: { plugins: { delete(name: string): unknown } }) {
        chain.plugins.delete("webpackbar");
      },
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
