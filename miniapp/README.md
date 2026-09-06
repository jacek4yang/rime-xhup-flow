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
- **进度持久化**:`src/lib/state-schema.ts`(纯逻辑,可测)定义 V2 状态
  文档(偏好 + 稀疏进度 + 按日统计 + 键位错误),逐字段校验降级——
  损坏字段回退默认,绝不整体清空;`src/lib/storage.ts` 把
  `Taro.getStorageSync` 实现为共享核心 `StorageAdapter` 契约,并支持
  旧版 v1 进度一次性迁入。备份格式与桌面端同一份(`exportBackup`)。
- **触感反馈**:`src/lib/haptics.ts` 按共享核心 `HapticsAdapter` 契约
  把产品档位(off/light/medium)诚实映射到 `Taro.vibrateShort` 的
  物理档位(light/medium/heavy);设备不支持时静默无效。
- **主包体积**:启动只加载约 19KB 数据分片;完整数据集不进小程序。
  `pnpm --filter miniapp size` 输出体积报告并在超限时失败。

## 页面(里程碑 #42 MVP)

| 页面 | 路径 | 说明 |
| --- | --- | --- |
| 今日(tab) | `pages/index/index` | 今日统计、推荐入口、数据概览 |
| 学习(tab) | `pages/learn/index` | LEARN_CHAPTERS 9 章按 level 分组 |
| 课程 | `pages/lesson/index?id=` | 全部 section 种类(text/list/practice/shape-explorer) |
| 练习 | `pages/practice/index` | 12 种模式分组选择;池为空的模式禁用 |
| 练习设置 | `pages/practice-setup/index?mode=` | 题数/难度/提示/键帽参考 |
| 会话 | `pages/session/index?mode=&src=` | 核心循环:目标/槽位/教学键盘/暂停/错误教学 |
| 小结 | `pages/summary/index` | 准确率/KPM/CPM/连对 + 本场错题 + 重开 |
| 键位 | `pages/keyboard/index` | 教学键盘完整视图 + 零声母规则表 |
| 键详情 | `pages/key-detail/index?key=` | 声母/韵母/零声母/一级简码/形码例字/我的错误 |
| 错题 | `pages/mistakes/index` | listWeakItems 清单 + keyHeatmap 热力 + 维度聚合 |
| 设置(tab「我的」) | `pages/settings/index` | 语言/提示/错误教学/键帽/触感/导出/重置 |

tabBar 三页:今日 / 学习 / 我的。练习会话相关页面不在 tab 内。

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
pnpm miniapp:typecheck    # 严格 TS
pnpm miniapp:test         # 语义测试:分片校验、多模式端到端、持久化往返/降级、触感/键帽标签
pnpm miniapp:build        # 实际 weapp 构建(成功才算支持微信)
pnpm --filter miniapp size  # 主包体积报告 + 门槛
```

CI 会运行以上全部,并运行共享核心的平台纯度守卫
(`packages/trainer-core/src/purity.test.ts`),防止桌面/小程序依赖泄漏进核心。

## 当前范围与已知边界

- 11 个页面覆盖训练主流程;教学键盘质量对齐桌面端 PR #39
  (rpx 固定几何、大键帽、情境双拼/形码标签——标签只来自规范数据,
  无数据的键留空,绝不编造)。
- 提示方式(HintMode)独立控制答案类信息;键帽参考(KeyRefMode)
  只影响键帽标签,两者正交,与桌面端一致。
- 触屏无实体键盘:输入全部经屏显键盘;反馈展示 1 秒自动前进
  (桌面端 150ms 是为键盘用户设计,触屏适当放慢)。
- 分片只含高频子集:部分一级简码字的全码不在分片(备用合法码为空),
  引擎按「仅简码路线」降级,不影响练习;形码探索器例字数量受分片限制。
- 暂无统计页(按日时长趋势);错题页已覆盖键位热力与维度聚合。
