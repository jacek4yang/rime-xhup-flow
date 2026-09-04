# XHUP Flow

**XHUP Flow** 是一套基于标准 librime 的小鹤音形增强输入方案,外加配套的
训练与装机工具。它在**冻结的静态肌肉记忆层**之上,提供连续组句与
**纯本机**的学习能力,并保证静态层的每一次击键行为与既有习惯完全一致。

本仓库同时维护一个早期发布的经典方案「小鹤音形·全码优先」
(`xhup_fullcode`,冻结维护),其用户文档见
[docs/legacy-fullcode-scheme.md](docs/legacy-fullcode-scheme.md)。

## XHUP Flow 是什么?与普通小鹤/Rime 配置有何不同?

普通小鹤音形配置只提供固定的码表。XHUP Flow 在此之上做了三件事:

1. **静态层(冻结)**:一级简码、单字全码(2/3/4 码)、词语简码、
   固定词与 FIXED_FIRST 简码全部是**生成器按规范数据确定性产出**的
   冻结层——发布后不改动任何既有映射,保证肌肉记忆零回归。
2. **Flow 组句引擎**:连续键入多个词条的全码即可组成句子(例如
   `womf`+`uijm` 组出「我们时间」),可稳定组到 20 字长句;组句保持
   活动直到显式上屏,无自动提交。
3. **本地学习**:上屏 Flow 组句会训练专用用户词典 `xhup_flow_user`,
   参与后续组句排序。学习数据只存本机,无账号、无遥测、无云端。

候选优先级契约(由 runtime 审计逐码断言):

```text
冻结静态层(一级简码 / 单字全码 / 词语简码 / 固定词)
  > 动态候选(用户词典词条、学习词条)
    > 组句候选
```

对全部 140,038 个静态 exact 码的审计(干净 userdb 与学习后两种状态)
确认:菜单逐项同序相等、无可见重复,动态候选只允许追加在静态组之后。

## 产品组成

| 组件 | 说明 |
| --- | --- |
| `xhup_flow` 方案 | 主方案:静态层 + 组句 + 本地学习 |
| `xhup_flow_static` 方案 | 静态回退:同一静态层,无组句、无学习(调试/隐私场景) |
| Trainer 训练器 | 桌面/Web 应用:12 种练习模式、错题中心、统计、键位参考 |
| 控制中心 | Trainer 内「输入法」页:安装/升级/修复/卸载、学习管理、诊断 |
| `xhup-cli` | 命令行:方案生成与学习数据管理(status/export/import/reset) |

## 支持平台

| 系统 | 前端 | 用户数据目录 |
| --- | --- | --- |
| Windows | 小狼毫 Weasel | `%APPDATA%\Rime` |
| macOS | 鼠须管 Squirrel | `~/Library/Rime` |
| Linux | Fcitx5-Rime | `~/.config/fcitx5/rime` |
| Linux | IBus-Rime | `~/.config/ibus/rime` |
| Android | fcitx5-android | 平台中立包手动导入(桌面端不做自动安装) |

## 安装

### 方式一:Trainer 桌面应用(推荐)

1. 安装对应平台的 Rime 前端(小狼毫 / 鼠须管 / Fcitx5 / IBus)。
2. 启动 Trainer 桌面应用,进入「输入法」页。
3. 点击**安装**:应用会先展示完整执行计划(逐文件新建/覆盖+备份),
   确认后写入方案文件;随后按提示在输入法菜单执行**重新部署**。

升级与修复走同一入口:应用自动对比已安装版本与随附版本;卸载只删除
XHUP 拥有的文件,不影响学习数据与你的其它 Rime 配置。

### 方式二:平台中立 Rime 源包(不装 Trainer)

从 CI 产物获取 `xhup-flow-rime-vX.Y.Z.zip`(含 `INSTALL.md` 说明),
把其中全部 `.yaml` 复制到上表的 Rime 用户目录,然后重新部署。

### 启用方案(两套方案同时安装)

Flow(组句学习)与 Static(纯静态)一起安装;在输入法的方案菜单中
切换,不需要改写任何配置文件。

## Flow 与 Static 模式怎么选?

- **Flow**:日常使用。组句 + 本地学习,学习数据仅存本机。
- **Static**:调试、性能基准或隐私敏感场景。与主方案共用同一组词典,
  不重复数据,行为完全可预测。

## 学习数据备份与恢复

- 图形界面:控制中心「学习数据」卡(导出快照 / 导入快照 / 重置)。
- 命令行:`xhup-cli learning status|export|import|reset`
  (包装 librime 官方 `rime_dict_manager`,不解析 userdb 内部格式)。

导出的快照是 Rime 标准文本格式,可跨安装、跨机器恢复。重置是破坏性
操作,UI 与 CLI 均要求显式确认。

## 更新

- Trainer 控制中心:检测到新版本后点**升级**(覆盖前自动备份到用户
  目录的 `xhup_backup/`,可手动回滚)。
- 手动更新:用新包覆盖旧文件,**不要删除 `xhup_flow_user.userdb`**,
  然后重新部署。

## 卸载

- Trainer 控制中心:**卸载**(明确列出将删除的文件;默认保留学习数据)。
- 手动卸载:从 Rime 用户目录删除全部 `xhup_flow*.yaml` 方案/词典文件;
  如确认不再需要学习数据,可自行删除 `xhup_flow_user.userdb`(普通卸载
  无必要)。

## 隐私

本地运行:无账号、无遥测、无云端同步、无网络依赖。

- 学习数据只存于本机 `xhup_flow_user.userdb`,绝不进入本仓库。
- 控制中心的诊断报告已脱敏:不含学习词内容、个人文件与环境细节。
- 仓库 `.gitignore` 显式排除 `installation.yaml`、`user.yaml`、`sync/`、
  `*.userdb/` 等运行时状态。

## 已知限制

- librime 内置短语编码器为提交历史生成的学习词条码是 librime 内部
  派生(确定但不可读作小鹤语义;与静态码位重合时动态候选恒排在静态
  候选之后)。基于编码规则的确定性人读短语码属后续研究,不是 v1 阻塞。
- 学习导出/导入依赖 librime 官方 `rime_dict_manager`(发行包
  `librime-bin`);未安装时控制中心会明确提示。
- 桌面端不对 Android 做自动文件安装(fcitx5-android 使用平台中立包
  手动导入,待安全集成后自动化)。
- Windows/macOS 桌面安装物在 CI 中未签名(SmartScreen/Gatekeeper 会
  提示;正式发布签名属人工发布决策)。

## 面向开发者

- 架构与数据流水线:[docs/architecture.md](docs/architecture.md)。
- 开发约束与验证命令:[AGENTS.md](AGENTS.md)、[CONTRIBUTING.md](CONTRIBUTING.md)。
- 训练器说明:[trainer/README.md](trainer/README.md)。
- 发布前的人工验收清单:[docs/release-readiness.md](docs/release-readiness.md)。
- 性能基线与测量方法:[docs/performance-baseline.md](docs/performance-baseline.md)。

## 上游与授权

小鹤相关词典数据来源及授权信息见 [NOTICE.md](NOTICE.md)。项目不是
小鹤官方项目,也不代表小鹤官方立场。仓库按 [LGPL-3.0](LICENSE) 授权
条款发布;第三方词典或数据仍遵循各自的来源与授权要求。
