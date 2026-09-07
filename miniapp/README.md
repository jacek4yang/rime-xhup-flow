# XHUP Flow 微信小程序(Taro + React)

基于共享训练核心 `@xhup/trainer-core` 的微信小程序训练器。学习/练习语义
全部来自共享核心与 Rust 生成数据;本包只承载平台 UI 与宿主适配。

## 架构位置

```text
Rust 生成规范数据 → packages/trainer-core(平台中立)→ miniapp(Taro/React)
```

- **数据分片**:`scripts/generate-shards.mjs` 从桌面端规范数据集
  (`trainer/public/generated/xhup_flow_trainer.json`)确定性截取启动子集,
  输出到 `src/data/generated/dataset.json`,加载时仍走共享核心完整校验。
  唯一事实来源始终是 Rust;本脚本不手写任何编码/读音/词码。
- **进度持久化**:`src/lib/storage.ts` 把 `Taro.getStorageSync` 等实现为
  共享核心 `StorageAdapter` 契约;进度 Schema 与桌面端同一份
  (`ItemProgress`),备份格式可跨端移植(不包含规范数据)。
- **主包体积**:启动只加载约 19KB 数据分片;完整数据集不进小程序。
  `pnpm --filter miniapp size` 输出体积报告并在超限时失败。

## 本地开发

前置:Node ≥ 20、pnpm、WeChat DevTools(微信开发者工具)。

```bash
# 仓库根(一次性)
pnpm install

# 先确保桌面端规范数据存在(小程序分片从它截取)
pnpm -C trainer generate:data

# 构建 weapp 产物(自动生成数据分片)
pnpm miniapp:build

# 产物在 miniapp/dist/
```

打开 WeChat DevTools →「导入项目」→ 目录选择 `miniapp/dist`,
AppID 使用测试号(本地 `project.config.json` 内置 `touristappid`)。

- **不要**把真实 AppSecret / 上传私钥提交到仓库;如需真机预览,
  在 DevTools 中用「测试号」或本地配置自己的 AppID,不要改
  `project.config.json` 后提交。

## 测试与门槛

```bash
pnpm miniapp:test      # 语义测试:分片校验、5 题真实编码练习、进度持久化
pnpm miniapp:build     # 实际 weapp 构建(成功才算支持微信)
pnpm --filter miniapp size  # 主包体积报告 + 门槛
```

CI 会运行以上全部,并运行共享核心的平台纯度守卫
(`packages/trainer-core/src/purity.test.ts`),防止桌面/小程序依赖泄漏进核心。

## 当前范围(#41 验证实现)

- 今日/学习中心/练习 3 个页面;渲染真实课程章节、跑 5 题双拼码练习、
  逐键校验真实编码、本机持久化进度。
- 完整产品化(教学键盘、错题、弱点、统计、分包等)在后续里程碑推进。
