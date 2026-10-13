# XHUP Flow 微信小程序(Taro + React)

基于共享训练核心 `@xhup/trainer-core` 的微信小程序训练器。学习/练习语义
全部来自共享核心与 Rust 生成数据;本包只承载平台 UI 与宿主适配。

## 架构位置

```text
Rust 规范 Trainer JSON → ASCII JSON/Brotli + SHA-256 清单 → 本地文件读取 → 共享校验/按需索引 → Taro/React
```

- **完整规范数据**:`scripts/build-dataset.mjs` 无损编译桌面端同一份
  `trainer/public/generated/xhup_flow_trainer.json`，不再抽取演示子集。
  输出 `src/data/generated/trainer.json.br` 和 `dataset-manifest.json`；
  清单绑定源 JSON、Node/Brotli 版本、构建器源码及压缩/解压产物 SHA-256。
  正常生成先运行 Rust；直接调用 Taro 也必须通过构建器 `--check`，陈旧产物不能继续打包。
  唯一事实来源仍是 Rust，不在脚本或前端手写编码。
- **进度持久化**:`src/lib/state-schema.ts`(纯逻辑,可测)定义 V3 状态
  文档(偏好 + 稀疏进度 + 跨天复习 + 按日统计 + 键位错误),逐字段校验降级——
  损坏字段回退默认,绝不整体清空;`src/lib/storage.ts` 把
  `Taro.getStorageSync` 实现为共享核心 `StorageAdapter` 契约,并支持
  旧版 v1 进度一次性迁入。备份格式与桌面端同一份(`exportBackup`)。
- **触感反馈**:`src/lib/haptics.ts` 按共享核心 `HapticsAdapter` 契约
  把产品档位(off/light/medium)诚实映射到 `Taro.vibrateShort` 的
  物理档位(light/medium/heavy);设备不支持时静默无效。
- **主包体积**:完整规范数据以本地 Brotli 资源随包分发，不请求网络。
  当前压缩资源 944,651 字节，解压为 14,979,909 字节 ASCII JSON；
  包含 28,851 个单字条目、20,000 个词、65,909 个 PRIMARY 简码、
  2,933 个 FIXED_FIRST 简码、26 个一级简码及 14 条句子。
  这是完整 Trainer 数据集，不等于 Rime 全部输入词典。
  `pnpm --filter miniapp size` 保持总包 1,572,864 字节、单 JS 204,800 字节门槛；
  `--json`/`--out <file>` 可供 CI 留档。
- **加载与错误处理**:通过宿主 `readCompressedFile` 回调读取代码包文件，
  无须额外解压库。缺 API、读取失败或数据损坏会阻止练习并显示重试入口，
  不回退旧数据或缺字子集，不清除用户进度。需要支持该文件 API 的微信版本。
  App 始终保留 Taro 的原生页面根节点；数据门禁位于各页面内，加载期间也能显示错误和重试，
  不会因替换 App children 而移除 PageWrapper。
- **启动与索引**:解压后完整校验所有规范行，再建立全部 ID 索引；
  训练项和题池排序按访问生成并在当前数据集内缓存，首次进入新题池仍有成本。
  `dataset.perf.test.ts` 的 200ms 门槛仅计校验及索引初始化，
  不含解压、JSON 解析、题池首次物化或渲染；单独进程执行以免与其他测试争用。
  Node 测量不能代表微信真机启动、内存或交互验收。

## 页面

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
| 复习与错题 | `pages/mistakes/index` | 到期条目与薄弱项清单 + keyHeatmap 热力 + 维度聚合 |
| 统计 | `pages/stats/index` | 按日训练统计与错误分布 |
| 导入 | `pages/import/index` | 备份校验、预览与恢复 |
| 设置(tab「我的」) | `pages/settings/index` | 语言/提示/错误教学/键帽/触感/导出/重置 |

tabBar 三页:今日 / 学习 / 我的。练习会话相关页面不在 tab 内。

## 本地开发

前置:Node ≥ 20、pnpm、WeChat DevTools(微信开发者工具)。

```bash
# 仓库根(一次性)
pnpm install

# 构建 weapp 产物(自动运行 Rust 生成、完整数据编译及源身份校验)
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
pnpm miniapp:test         # 构建负向/重现性、真实页面盲打与重试、跨端状态、独立性能门槛
pnpm miniapp:build        # 实际 weapp 构建 + 编译产物运行回归,不等于真机验收
pnpm --filter miniapp size  # 主包体积报告 + 门槛
```

CI 会运行以上全部,并运行共享核心的平台纯度守卫
(`packages/trainer-core/src/purity.test.ts`),防止桌面/小程序依赖泄漏进核心。
`scripts/test-weapp-runtime.cjs` 在显式模拟的原生宿主中执行真正编译后的 Taro/React，
验证回调式读包前页面已挂载、恢复规范弱项后不序列化完整答案以及页面卸载。
它能捕获单独 jsdom 页面测试看不到的 App/Page 集成错误，但不能替代微信宿主验收。

复习状态和间隔策略见[Trainer 跨天复习说明](../trainer/README.md#跨天复习与迁移)。
本地存储键保持不变,V1/V2 记录保留原有统计并迁入 V3;新备份为 V3,
可导入旧 V2 备份。后台恢复时重新计算到期条目,盲打提示授权不因到期而改变。
复习入口的一级简码和长句可通过真实页面组件的触屏按钮完成;这些 DOM 测试
不是微信真机结果。桌面与小程序使用同一份完整规范 Trainer JSON。

## 当前范围与已知边界

- 13 个页面覆盖训练主流程;教学键盘沿用现有布局
  (rpx 固定几何、大键帽、情境双拼/形码标签——标签只来自规范数据,
  无数据的键留空,绝不编造)。
- 盲打中不因提示偏好或键帽参考自动揭示答案；主动帮助会记录为非独立回忆。
- 触屏无实体键盘:输入全部经屏显键盘;反馈展示 1 秒自动前进
  (桌面端 150ms 是为键盘用户设计,触屏适当放慢)。
- 缺失完整码的演示子集已移除；例如规范 `去=quts` 可以从复习入口完成盲打，
  读取失败→重试→真实页面触屏作答有回归测试。
- `touristappid` 只用于本地开发。weapp 构建和 DOM 测试不是微信 DevTools／
  真机验收；本次完整数据包的真实微信启动与内存仍为 **UNVERIFIED**。
