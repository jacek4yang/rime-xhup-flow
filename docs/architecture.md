# XHUP Flow 架构

本文档描述 XHUP Flow 的数据流水线、优先级契约、简码语法与兼容性承诺。
运行约束见 [AGENTS.md](../AGENTS.md);端用户说明见 [README.md](../README.md)。

## 数据流水线

```text
canonical source data(data/:音形数据、词频、固定词来源)
        ↓
Rust 生成器(xhup-generator,唯一语义来源)
        ↓
┌──────────────────────────┬─────────────────────────────┐
│ 静态 Rime 源文件           │ Trainer 规范数据集            │
│ xhup-cli generate rime   │ xhup-cli generate trainer    │
│ (11 个 YAML:方案+词典)    │ xhup_flow_trainer.json (V2)  │
└────────────┬─────────────┴──────────────┬──────────────┘
             ↓                            ↓
      Static Engine                Trainer 前端(V2 契约校验)
      (xhup_flow_static)                  ↓
             ↓                       练习/错题/统计(本地)
      Flow Engine(xhup_flow)
      静态层 + 组句 + 本地学习
             ↓
      Trainer 控制中心(Rust manager:安装/升级/修复/卸载/诊断)
             ↓
      打包(product-packaging 工作流:Rime 包 + 桌面/移动安装物)
```

核心边界:

- **Rust 是唯一语义来源**。React/TypeScript 不维护任何码表;前端只校验
  与消费生成的规范数据集(`schemaVersion: 2` 契约,构建期重新生成,
  不回退过期数据)。
- **生成是确定性的**:同一规范数据 + 同一生成器源码(含版本)+ 同一
  模板 ⇒ 字节级一致的产物(有测试兜底;CI 生成 `CANONICAL-SHA256SUMS.txt`)。
- **Tauri 是薄平台层**:控制中心的业务逻辑全部在
  `trainer/src-tauri/src/manager.rs`(纯 Rust、可单测),Tauri 命令
  只做环境检测与转发;前端通过 `window.__TAURI_INTERNALS__.invoke`
  类型化调用,浏览器环境自动降级。

## 候选优先级契约

```text
FROZEN STATIC  >  DYNAMIC USER LEARNING  >  SENTENCE COMPOSITION
```

- Flow 引擎绝不改变任何静态候选的相对次序与 top1。runtime 审计对全部
  140,038 个静态 exact 码逐码断言(干净 userdb 与学习后两种状态):
  菜单逐项同序相等、无可见重复,动态候选只允许追加在静态组之后。
- 冻结哨兵(永久有效):`uij → [铈, 鼫, 时间]` 精确序、`uijm → 时间`
  top1、`uj`/`ujm` **不得**出现 时间。
- 既有门禁:FIXED_FIRST 2380/2380、占用二码 405/405、二码 ZR 245/245。

## 简码语法

| 语法 | 适用范围 | 形态 |
| --- | --- | --- |
| `LegacyAnyFiV1` | 仅冻结旧数据(既有 production 简码) | 任意含 I 的 F/I 组合(冻结语法,不再新增) |
| `MonotoneSuffixInitialsV2` | 未来生产简码 | 单调后缀缩写 `F* I*`,至少一个 I |

`ShortcutPolicyId`(FIXED_FIRST / ZERO_REGRESSION / FIXED_FIRST_SHORTCUT
等策略身份)是兼容接口,发布后不得变更语义。

## 兼容性契约(v1.x 冻结)

以下接口在 v1.x 内冻结,任何修改都属破坏性方案变更,必须由人工决策:

- canonical FullCode(单字 2/3/4 码全码);
- 已发布简码映射的既有映射与菜单次序(一级简码 26、二/三/四码字符
  菜单、100k 固定词 FullCode、44,448 ZERO_REGRESSION、2,380 FIXED_FIRST、
  245 二码 ZERO_REGRESSION);
- `ShortcutPolicyId` 值;
- `xhup_flow_user` 用户词典身份(学习数据载体);
- Trainer 持久化数据迁移兼容(进度/备份可跨版本导入);
- 控制中心所有权清单规则(`OWNED_FILES`:卸载只删 XHUP 拥有文件,
  绝不触碰其它 Rime 配置与学习数据)。

## 版本模型

| 载体 | 当前值 | 含义 |
| --- | --- | --- |
| workspace `Cargo.toml` `version` | 0.1.0 | XHUP Flow 产品版本;Rime 源包内嵌版本(`{{VERSION}}` 模板) |
| `trainer/src-tauri/tauri.conf.json` `version` | 0.1.0 | 桌面/移动安装包版本(测试强制与 workspace 一致) |
| `VERSION` 文件 | 1.0.0 | 经典方案 `xhup_fullcode` 的发布版本(`release.yml` 使用),**不是** XHUP Flow 产品版本 |

Rime 包版本随生成器内嵌;桌面应用版本与 workspace 版本由
`product_versions_are_synchronized` 测试兜底。正式 v1 发布时的统一
升版(如 1.0.0-rc.1)是人工决策,见
[docs/release-readiness.md](release-readiness.md)。

## 平台中立 Rime 源包

`xhup-cli generate rime` 产出 11 个源文件(两套方案 + 全部词典),
不含 userdb;面向 Weasel / Squirrel / fcitx5-rime / ibus-rime /
fcitx5-android 等标准 librime 客户端。打包时附
[rime/package/INSTALL.md](../rime/package/INSTALL.md) 安装说明。
CI 在临时目录用 librime 实机编译两套方案作为发布门禁。

## 控制中心(产品管理)

```text
React(状态/计划展示/确认) → Tauri 命令(薄) → manager.rs(纯逻辑)
      → 平台适配(目录探测/环境变量) → 文件系统/Rime 环境
```

安全不变量(单测覆盖):

- 计划先于动作:install/upgrade/repair/uninstall 都先产出 dry-run
  `Plan`,执行前按当前磁盘状态重新规划;
- 只写/只删 `OWNED_FILES`;覆盖前备份到 `xhup_backup/`(保留最早版本);
  临时文件 + 原子 rename 写入;
- `xhup_flow_user.userdb` 永不在计划内;用户其它 Rime 配置不被触碰;
- 学习管理复用 `xhup-cli` `learning`(rime_dict_manager 官方机制);
- 诊断报告脱敏(不含学习词内容/个人文件/环境细节)。
