# XHUP Flow 架构

本文档保留早期架构背景，含历史文件数与双 translator 描述，**不是当前
实现或发布就绪证明**。当前支持边界以 [运行时矩阵](runtime-support-matrix.md)、
[Lua 运行时](lua-runtime.md) 与生成包源码为准；端用户说明见 [README.md](../README.md)。
[architecture-v2.md](architecture-v2.md) 是研究目标，不代表完整联合解码器已交付。

## 数据流水线

```text
canonical source data(data/:core 音形、attested 输入码、词频/词汇证据)
        ↓
Rust 生成器(xhup-generator,唯一语义来源)
        ↓
┌──────────────────────────┬─────────────────────────────┐
│ 静态 Rime 源文件           │ Trainer 规范数据集            │
│ xhup-cli generate rime   │ xhup-cli generate trainer    │
│ (12 个 YAML + 2 Lua)     │ xhup_flow_trainer.json (V4)  │
└────────────┬─────────────┴──────────────┬──────────────┘
             ↓                            ↓
      Static Engine                Trainer 前端(V4 契约校验)
      (xhup_flow_static)                  ↓
             ↓                       练习/错题/统计(本地)
      Flow Engine(xhup_flow)
      静态层 + 完整词汇/单字开放组句 + 本地学习
             ↓
      用户按安装教程复制方案、备份配置并重新部署
             ↓
      打包(product-packaging 工作流:Rime 包 + 桌面/移动安装物)
```

核心边界:

- **Rust 是唯一语义来源**。React/TypeScript 不维护任何码表;前端只校验
  与消费生成的规范数据集(`schemaVersion: 4` 契约,构建期重新生成,
  不回退过期数据)。
- **生成是确定性的**:同一规范数据 + 同一生成器源码(含版本)+ 同一
  模板 ⇒ 字节级一致的产物(有测试兜底;CI 生成 `CANONICAL-SHA256SUMS.txt`)。
- **Tauri 是纯训练容器**：不注册应用自定义 IPC，不链接输入法管理核心，
  不内嵌 Rime 安装包。练习、错题、统计、键位与进度备份由同一前端和共享核心负责；
  输入法安装、部署及学习词库管理与 Trainer 分离。

## 候选优先级契约

```text
FROZEN STATIC  >  DYNAMIC USER LEARNING  >  OPEN SENTENCE COMPOSITION
```

- Flow 引擎绝不改变任何静态候选的相对次序与 top1。runtime 审计对全部
  141,138 个静态 exact 码逐码断言(干净 userdb 与学习后两种状态):
  完整 static 菜单是 Flow 菜单的同序前缀、无可见重复，动态/开放候选
  只允许追加在静态组之后。
- 冻结哨兵(永久有效):
  `uij → [时间, 史记, 实践, 事迹, 铈, 鼫]` 精确序、`uijm → 时间`
  top1、`uj`/`ujm` **不得**出现 时间。
- v2 门禁:68,842/68,842 映射完整性、65,909 条 PRIMARY 绝对名次、
  2,933 条 FIXED_FIRST rank1、传统别名与 2–5 键 runtime 哨兵。

## 简码语法

| 语法 | 适用范围 | 形态 |
| --- | --- | --- |
| `LegacyAnyFiV1` | v2 PRIMARY 中的传统别名;旧 selector 仅研究重放 | 任意含 I 的 F/I 组合 |
| `MonotoneSuffixInitialsV2` | v2 FIXED_FIRST 与其他 v2 候选 | 单调后缀缩写 `F* I*`,至少一个 I |

v1 selector 的 ZERO_REGRESSION/FIXED_FIRST/二码数据冻结在
`data/shortcuts/legacy/`,只供 research-only 重放,不是 production layer。

## 输入域与词汇可达性

```text
CoreStandardHanzi(8,105 linguistic facts) ⊂ InputHanzi(current attested 8,208; extensible)
hot static words(100,000) ⊂ pinned extended words(1,301,434)
                              ⊂ open-composition reachable text
```

`HanziReading → XhupInputSyllable` 只是 core 的机械推导路径；小鹤官网或
固定历史词典证明的 `AttestedXhupCode(sound, shape)` 可直接产生候选，不反向
篡改语言学读音。Flow 词典把完整词汇证据与全部两键单字原语放在隔离的
低质量 translator 中，既允许未知组合/语气字组句，也不污染 static exact。

## 兼容性契约(v1.x 冻结)

以下接口在 v1.x 内冻结,任何修改都属破坏性方案变更,必须由人工决策:

- canonical FullCode(单字 2/3/4 码全码);
- 已发布简码映射的既有映射与菜单次序(一级简码 26、二/三/四码字符
  菜单、100k 固定词 FullCode、68,842 条 canonical v2 词语简码;
  自 v1.0.0 起冻结);
- `ShortcutPolicyId` 值;
- `xhup_flow_user` 用户词典身份(学习数据载体);
- Trainer 持久化数据迁移兼容(进度/备份可跨版本导入);
- Trainer 不读取、写入或删除输入法用户目录；手动安装须保存原配置与学习数据。

## 版本模型

| 载体 | 当前值 | 含义 |
| --- | --- | --- |
| workspace `Cargo.toml` `version` | 1.0.0 | XHUP Flow 产品版本唯一真源;Rime 源包内嵌版本(`{{VERSION}}` 模板) |
| `trainer/src-tauri/tauri.conf.json` `version` | 1.0.0 | 桌面/移动安装包版本(测试强制与 workspace 一致) |
| `trainer/package.json`、`miniapp/package.json`、`packages/trainer-core/package.json` `version` | 1.0.0 | 前端包元数据(测试强制与 workspace 一致) |
| (已移除)legacy `VERSION` 文件 | — | 经典方案 `xhup_fullcode` 已于 v1 收口时从 main 移除(源码见 git 历史与既有 GitHub Releases) |

Rime 包版本随生成器内嵌;全部产品级版本来源由
`product_versions_are_synchronized` 测试与发布管线
(xhup-flow-rc-release.yml)的一致性门禁双重兜底。
[docs/release-readiness.md](release-readiness.md)。

## 平台中立 Rime 源包

`xhup-cli generate rime` 确定性产出完整源文件与来源登记，
不含 userdb;面向 Weasel / Squirrel / fcitx5-rime / ibus-rime /
fcitx5-android 等标准 librime 客户端。打包时附
[rime/package/INSTALL.md](../rime/package/INSTALL.md) 安装说明。
CI 在临时目录用 librime 实机编译两套方案作为发布门禁。

## 训练应用与手动安装边界

Trainer 的 Rust 层仅初始化 Tauri。应用启动、练习和重启不要求存在 Rime；
历史 product_*、learning_*、explain_* 自定义 IPC 均未注册。
`xhup-flow.trainer.v2` 保存练习进度，JSON 备份不包含 Rime userdb。
构建期仍用生成器生成训练数据，但不将 Rime 安装包或管理实现链接进应用。

发布包的真实 Linux WebKit 测试验证无 Rime 环境可练习、旧管理 IPC 拒绝、
原有 Rime 文件与学习字节保持不变，以及实际关闭重启后进度仍可恢复。
用户通过[手动教程](install-guide.zh-CN.md)独立安装输入法前端与方案；
旧安装器设计文档只作历史记录，不代表当前支持功能。
