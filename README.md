# XHUP Flow

**XHUP Flow** 是一套基于标准 librime 的小鹤音形增强输入方案,外加配套的
独立训练工具与手动安装教程。它在**冻结的静态肌肉记忆层**之上,提供连续组句与
**纯本机**的学习能力。`xhup_flow_static` 是独立兼容锚点；主方案保护
静态精确码菜单，不把组句中间状态与旧方案逐键等同。
当前能力与测试边界见 [运行时支持矩阵](docs/runtime-support-matrix.md)。

早期发布的经典方案「小鹤音形·全码优先」(`xhup_fullcode`,冻结维护)已随
v1 收口从 main 移除;既有版本仍可从 Releases 下载,文档见
[docs/legacy-fullcode-scheme.md](docs/legacy-fullcode-scheme.md)。

## 发布验收范围

本次 v2.0.0 按所有者明确授权，采用[云端底层运行时资格发布流程](docs/runtime-qualified-release.md)。
发布要求同一冻结 RC 源提交的 CI、Full Regression 和 RC Release 全部成功，
并核验完整原生审计证据、封存清单及实际附件字节；平台人工验收状态保持待验。
此前的[本地完整验收方案](docs/local-qualified-release.md)保留为历史替代流程，
不作为本次云端资格证明。

v2.0.0 可按所有者明确授权的「底层运行时合格、用户平台待验」范围发布，
不代表 Windows / macOS / Android 人工安装和实际输入法体验均已通过。
Windows、macOS 真机测试由用户完成；Linux 虚拟 Fcitx 前端实验未通过，
且明确不作为本次发布门禁。底层 librime 资格通过不代表真实桌面输入上下文通过。
Windows/macOS 包未签名，macOS 未公证；安装时请核对附件哈希和来源。
每次发布的实际范围以 Release 中 `ACCEPTANCE.json`、`QUALIFICATION.json`
及证明为准，`UNVERIFIED` 始终表示待验。默认完整平台门禁仍保留。
详见[发布资格策略](docs/runtime-qualified-release.md)。

## XHUP Flow 是什么?与普通小鹤/Rime 配置有何不同?

普通小鹤音形配置只提供固定的码表。XHUP Flow 在此之上做了四件事:

1. **静态层(冻结)**:一级简码、生产字符全码(2/3/4 码)、固定词与
   68,842 条 canonical v2 词语简码全部是**生成器按规范数据
   确定性产出**的冻结层。v2 映射分为 65,909 条 PRIMARY 和 2,933 条
   FIXED_FIRST,两者在同一静态 translator 中按显式 merged rank 排序;
   v1.0.0 后不改动既有映射与菜单次序。
2. **Flow 开放组句引擎**:完整 pinned 万象词汇层改善 exact 候选与分段
   排名，逐字两键音码原语让词表外组合、单字、结构助词和语气字也能参与
   组句。例如 `tiuici` 得到「提示词」，`enwojtdeveyhjqkeyile` 得到
   「嗯我觉得这样就可以了」；组合保持活动直到显式上屏，无自动提交。
3. **本地学习**:具备所需 Lua 学习接口时，上屏 Flow 组句会训练专用用户词典
   `xhup_flow_user`，参与后续组句排序，并受持久更新配额限制。
   缺少接口、存储异常或额度耗尽时，候选显示「学习暂停」及原因；输入仍可用，
   但不能把只读降级当成学习成功。学习数据只存本机，无账号、无遥测、无云端。
4. **简码提示(可选)**:装有 librime-lua 的环境里,全码输入时候选会
   标注更短的简码(如 `uijm` → 时间 `~uij`,纯 ASCII 极简呈现),帮助边用边记;可在
   方案开关中关闭。主方案的边界规划需要 librime-lua；缺少插件时请用
   `xhup_flow_static`，不能把仍有静态候选当成完整 Flow。

候选优先级契约(由 runtime 审计逐码断言):

```text
冻结静态层(一级简码 / 单字全码 / 词语简码 / 固定词)
  > 原生 Flow 候选(本地学习、开放组句、有界边界规划)
    > 未完成尾码的前缀候选(保留活动输入，不自动提交)
```

全量门禁遍历 141,138 个静态 exact 码(干净 userdb 与学习后两种状态)，
要求完整静态菜单是 Flow 菜单的同序前缀、top1 不变且无可见重复。
只有针对指定生成包完成该审计，才能报告该包通过；历史结果不自动适用于新 RC。

字符模型明确分层：`CoreStandardHanzi` 是 8,105 字规范语言学子集，
`InputHanzi` 是当前 8,208 字生产输入域（随可审计编码证据扩展，不是新的
Unicode 硬上限）。小鹤官网/历史词典中有来源的
`sound + shape` 编码事实独立保存，不要求伪造语言学拼音；因此「嗯」可用
`ogkx` / `onkx` / `enkx`，「诶」可用 `eiyu`。详见
[data/xhup/README.md](data/xhup/README.md)。

词语同样分层：100,000 条 hot 固定词只负责冻结静态体验；其余
1,301,434 条通过校验的 pinned 万象 semantic entries 进入 secondary
Flow 层，来源外文本仍可逐字或分段输入。连续整串的候选和名次还受
歧义与有界分段影响，**不承诺任意整句都出现在候选头或成为首选**。

## 产品组成

| 组件 | 说明 |
| --- | --- |
| `xhup_flow` 方案 | 主方案: 2.0 智能化方案, 静态层 + 组句 + mandatory Lua 策略层 |
| `xhup_flow_static` 方案 | 静态回退: 永久保留 v1.0.0 冻结肌肉记忆, 纯静态零-Lua (调试/基准/隐私/无插件场景) |
| Trainer 训练器 | 独立桌面/Web/Android 练习应用:12 种模式、错题、统计、键位与进度备份；不安装或管理输入法 |
| `xhup-cli` | 命令行:方案生成、运行环境与合同诊断 (doctor)、学习数据管理 |

## 支持平台

以下是目标兼容平台，不代表每个前端版本均已完成实际使用验收。
**Windows / macOS 人工验收由用户执行**；CI 编译或 smoke 通过不替代
真实小狼毫 / 鼠须管与 WebView 测试。请以对应 Release 的验收记录、
发布范围和附件哈希为准；旧 RC 或开发分支不能替代该版本的发布证据。

| 系统 | 前端 | 用户数据目录 |
| --- | --- | --- |
| Windows | 小狼毫 Weasel | `%APPDATA%\Rime` |
| macOS | 鼠须管 Squirrel | `~/Library/Rime` |
| Linux | Fcitx5-Rime | `~/.local/share/fcitx5/rime` |
| Linux | IBus-Rime | `~/.config/ibus/rime` |
| Android | fcitx5-android | 平台中立包手动导入(桌面端不做自动安装) |

### Windows / macOS 用户验收与反馈

请先备份整个 Rime 用户目录，在可恢复的测试环境安装候选版。不要把学习
词库或私人输入日志上传到公开 Issue。每次反馈须注明 **RC/tag、源提交、
下载资产文件名及 SHA-256、操作系统、前端和 librime/librime-lua 版本**。
Windows 可用 `Get-FileHash <文件> -Algorithm SHA256`；macOS 可用
`shasum -a 256 <文件>`。版本未知请明确填“未知”，不要凭文件存在推断模块已加载。

- [ ] **安装与部署**：按照手动教程备份原配置、复制方案、重新部署。
  Rime 方案菜单只显示 XHUP Flow；原全局自定义项在独占期间被替换，
  用户保存的首次备份应完整保留，其他方案和学习数据不应被删除或改写。
- [ ] **真实 Lua 与静态回退**：确认 Flow 所需模块实际加载，无插件报错；
  Static 可独立输入。缺少插件时可在排障中显式修改方案列表切换 Static
  （这会退出独占配置），不把静默降级当成 Flow 通过。
- [ ] **逐键输入与编辑**：测试 `jbzq` → `jbzqu`（“进去”），
  `nihcnz`（“你好”）、`nihcnzqu`（“你好去”）、
  `nihcnznihcnzqu`（“你好你好去”）。检查候选、未完成尾码、
  退格恢复、部分候选选择后的继续输入及最终上屏；不得丢键或重复上屏。
  连续输入期间不应自动提交。这些是必须测试的回归案例，不是当前已通过的承诺。
- [ ] **学习生命周期**：使用公开的合成测试短语，比较学习前后及前端重启后；
  退出输入法后做导出、导入、重置；确认只影响本方案，重置有确认，
  输入法占用数据库时管理操作安全拒绝。先保留原始备份。
- [ ] **Trainer / WebView**：无需安装 Rime 即可练习，错题、统计、键位、
  训练进度导出/导入及重启恢复可用；应用不能安装、检测或改写 Rime 用户目录。
- [ ] **更新与卸载**：输入方案由用户按教程更新或回退；卸载 Trainer
  不应影响系统输入法、Rime 方案或学习词库。先备份训练应用自己的进度。
- [ ] **性能与异常**：记录冷启动、长输入和连续退格有无明显卡顿；
  安装失败应显示原因和恢复路径，不应悄悄删除原文件。

请到 [Issues](https://github.com/jacek4yang/rime-xhup-flow/issues) 提交上述
版本信息、逐项“通过 / 失败 / 未测”、最小公开复现输入、预期和实际结果。
截图与日志请去除个人内容；不要求提交用户词库。工程门禁和产物验证仍由
项目维护，用户测试仅承担这里列出的平台人工验收，**未收到结果前状态保持待测**。

## 安装

请按[详细手动安装教程](docs/install-guide.zh-CN.md)操作，其中包含：

- Windows PowerShell、macOS/Linux 下载及 SHA-256 校验命令；
- Android 电脑下载、`adb devices`、指定设备、`adb install -r` 与故障处理；
- Fcitx5 for Android 主程序和 Rime 插件、系统文件入口导入及部署；
- 桌面 Rime 前端、首次备份、方案复制、更新、回退和卸载。

**Trainer 只是训练器，不是系统输入法或安装器。** 不装 Rime 也能使用 Trainer；
想在其他应用里打字，需要另装 Rime 前端并手动部署平台中立源包。
训练进度备份与 `xhup_flow_user.userdb` 输入法学习数据彼此独立。

源包包含 `default.custom.yaml`，会以独占配置替换原全局设置，
默认只启用 `xhup_flow`。复制前必须将原文件备份到 Rime 用户目录外，
更新时保留首次备份，卸载时恢复。不要删除其他方案、`rime.lua`、
`user.yaml`、`installation.yaml`、`sync/` 或任何 `*.userdb`。
Static 与 wrapper 文件随包保留作内部资源；需要静态回退时由用户显式修改方案列表。

旧版 Trainer 若曾管理输入法文件，请先按手动教程核对并恢复原始配置备份；
新 Trainer 不会接管、迁移或清理旧 Rime 安装。

## Flow 与 Static 模式怎么选?

- **Flow**:日常使用。组句 + 本地学习,学习数据仅存本机。
- **Static**:调试、性能基准或隐私敏感场景。与主方案共用同一组词典,
  不重复数据,行为完全可预测。

## 学习与可选会话调序

原生 `xhup_flow_user` 是唯一持久运行时学习存储。保守更新配额用尽或
存储不可用时，候选提示“学习暂停”，仍可继续输入；导出后可显式重置。
这不是断电不丢数据的保证。隐私敏感场景请选择零学习的 Static。

`context_ranker` 和 `user_memory` 均默认关闭；后者现为**会话记忆**，
最多保留 512 项、每项 256 字节，关闭即清除，重启不保留。
它们不控制原生学习。旧 `xhup_flow_user_model.tsv` 不读取、不改写，
需要保留时请自行备份。未完成的实验联合解码器不再提供开启开关；
当前边界规划是结构启发式，不宣称语言模型或最优分词。
详见 [Lua 支持范围](docs/lua-runtime.md)。

## 学习数据备份与恢复

- 命令行:`xhup-cli learning status|export|import|reset`
  (包装 librime 官方 `rime_dict_manager`,不解析 userdb 内部格式)。

导出的快照是 Rime 标准文本格式,可跨安装、跨机器恢复。重置是破坏性
操作，CLI 要求显式确认。Trainer 仅备份自身训练进度，不管理输入法 userdb。

## 更新与卸载

输入方案由用户按[手动教程](docs/install-guide.zh-CN.md#7-更新回退和卸载)
更新、回退或卸载，始终保留首次配置备份与输入法学习数据。
Trainer 使用系统应用安装/卸载机制；更新前可在设置中导出训练进度，
卸载应用不会替你删除 Rime 方案，删除应用数据也不会还原已删除的训练记录。

## 隐私

本地运行:无账号、无遥测、无云端同步、无网络依赖。

- 学习数据只存于本机 `xhup_flow_user.userdb`,绝不进入本仓库。
- Trainer 不探测或修改输入法用户目录，只在自身本地存储保存训练记录。
- 仓库 `.gitignore` 显式排除 `installation.yaml`、`user.yaml`、`sync/`、
  `*.userdb/` 等运行时状态。

## 已知限制

- librime 内置短语编码器为提交历史生成的学习词条码是 librime 内部
  派生(确定但不可读作小鹤语义;与静态码位重合时动态候选恒排在静态
  候选之后)。基于编码规则的确定性人读短语码属后续研究,不是 v1 阻塞。
- 学习导出/导入依赖 librime 官方 `rime_dict_manager`(发行包
  `librime-bin`);未安装时 CLI 会明确报错。
- 各平台均由用户独立下载平台中立源包并手动安装；Android 通过前端公开文件入口导入。
- Windows/macOS 桌面安装物在 CI 中未签名(SmartScreen/Gatekeeper 会
  提示;正式发布签名属人工发布决策)。

## 面向开发者

- 架构与数据流水线:[docs/architecture.md](docs/architecture.md)。
- XHUP Flow 2.0 workstream（当前仅基础、无生产行为变化）:
  [docs/architecture-v2.md](docs/architecture-v2.md)。
- 官方规则、Flow 扩展与离线兼容审计:[docs/xhup-rules.md](docs/xhup-rules.md)。
- 开发约束与验证命令:[AGENTS.md](AGENTS.md)、[CONTRIBUTING.md](CONTRIBUTING.md)。
- 训练器说明:[trainer/README.md](trainer/README.md)。
- 发布前的人工验收清单:[docs/release-readiness.md](docs/release-readiness.md)。
- 性能基线与测量方法:[docs/performance-baseline.md](docs/performance-baseline.md)。

## 上游与授权

小鹤相关词典数据来源及授权信息见 [NOTICE.md](NOTICE.md)。项目不是
小鹤官方项目,也不代表小鹤官方立场。仓库按 [LGPL-3.0](LICENSE) 授权
条款发布;第三方词典或数据仍遵循各自的来源与授权要求。
