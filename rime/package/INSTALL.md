# XHUP Flow · 便携 Rime 源包

跨平台 XHUP 双拼方案源文件,面向标准 librime 客户端:
小狼毫(Windows)、鼠须管(macOS)、Fcitx5-Rime / IBus-Rime(Linux)、
fcitx5-android。无需安装 Trainer 桌面应用即可使用。

# XHUP Flow · Portable Rime source package

Cross-platform XHUP double-pinyin schema sources for standard librime
clients: Weasel (Windows), Squirrel (macOS), Fcitx5-Rime / IBus-Rime
(Linux), fcitx5-android. No Trainer desktop app required.

## 包内容 / Contents

| 文件 | 说明 |
| ---- | ---- |
| `default.custom.yaml` | 独占列表：Rime 只显示 XHUP Flow（替换前必须备份原文件） |
| `xhup_flow.schema.yaml` | 主方案(Flow:静态层 + 组句 + 本地学习) |
| `xhup_flow_static.schema.yaml` | 静态回退方案(仅固定层,无组句学习) |
| `xhup_flow.dict.yaml` | 顶层词典(导入下列词典) |
| `xhup_flow_chars.dict.yaml` | 单字全码(2/3/4 码) |
| `xhup_flow_words.dict.yaml` | 固定词语层(4/6/8 键) |
| `xhup_flow_shortcuts.dict.yaml` | 26 个一级简码 |
| `xhup_flow_word_shortcuts.dict.yaml` | canonical v2 PRIMARY 词语简码(显式合并候选位) |
| `xhup_flow_fixed_first_shortcuts.dict.yaml` | canonical v2 FIXED_FIRST 词语简码 |
| `xhup_flow_flow.dict.yaml` | Flow 组句词典 |
| `xhup_flow_learn.dict.yaml` | Flow 学习词典(导入组句词典并补充全码原语) |
| `xhup_flow_flow.schema.yaml` | 词典编译 wrapper(同上) |
| `xhup_flow_learn.schema.yaml` | 词典编译 wrapper(同上) |
| `lua/xhup_flow/annotation.lua` | 候选注释格式化模块(极简纯 ASCII,清洗装饰标记) |
| `lua/xhup_flow/quick_hint.lua` | 简码提示模块(追加纯 ASCII 简码注释) |
| `lua/xhup_flow/init.lua` | Lua 命名空间入口与运行时合同自检模块 |
| `lua/xhup_flow/data/quick_hints.lua` | 简码提示数据(生成器产出) |
| `lua/xhup_flow/native_tail.lua` / `full_span.lua` | 有界原生边界候选与同文候选跨度处理 |
| `lua/xhup_flow/context_ranker.lua` / `user_memory.lua` | 可选会话内上下文；不写按键日志或 TSV |
| `lua/xhup_flow/joint_decoder.lua` | 不可用功能的兼容 shim，不是已交付联合解码器 |
| `xhup_flow.sources.tsv` | clean-v1 来源策略与完整登记 |
| `NOTICE.md` / `licenses/` | 数据署名、项目改动及许可文本（再分发须保留） |
| `INSTALL.md` | 本说明(部署时无需复制) |

**Lua 运行时合同与方案选择**:
- 主方案 `xhup_flow` 需要具有相应 API 的 `librime-lua`。客户端版本名本身不能证明
  `TableTranslator`、字典查询及有界学习所需 API 可用；较旧插件可能只读降级。
  `xhup-cli doctor` 区分文件/配置证据与未知的实时能力，不能替代逐键与重启测试。
- `xhup_flow_static` 是零 Lua、零学习、零网络的兼容基准。其冻结 exact 菜单合同
  不表示主方案每个中间 preedit 完全相同；无合适 Lua 插件时请使用此方案。
- 本包完全遵循零 `rime.lua` 架构, 绝不触碰用户 `lua/` 目录下 `lua/xhup_flow/` 以外的任何文件。

**为什么有 wrapper schema**:PRIMARY 与 FIXED_FIRST 是静态主词典的导入表；
默认主方案只依赖 Learn wrapper，它完整导入 Flow YAML 词汇并供单一原生
组句/学习 provider 使用，不重复编译另一份 Flow table。
Flow wrapper 保留作兼容/研究资源。wrapper 不在 `schema_list` 中，不供用户选择。

**安装后 Rime 的方案列表只有 `xhup_flow`。** Static 和 wrapper 文件保留作
兼容/编译资源，不显示在方案菜单，不会自动添加默认拼音或其他方案。
这只修改 Rime 内部方案，不卸载系统键盘、系统输入法或其他方案文件。
如确需排障时启用 Static，请显式修改自己的方案列表；这将退出独占配置。

## 安装 / Install

把全部 YAML、`xhup_flow.sources.tsv` 与 `lua/` 复制到 Rime 用户数据目录
（保持目录结构，不删除其它方案或用户状态）。**先按下述步骤处理共享的
`default.custom.yaml`，不要直接覆盖。** 在下载/备份处保留
`INSTALL.md`、`NOTICE.md` 与 `licenses/`；再分发时必须一并携带：

- Windows 小狼毫:`%APPDATA%\Rime`
- macOS 鼠须管:`~/Library/Rime`
- Linux Fcitx5:`~/.local/share/fcitx5/rime`(或 `$XDG_DATA_HOME/fcitx5/rime`)
- Linux IBus:`~/.config/ibus/rime`(部分发行版为 `~/.config/ibus/rime`)
- fcitx5-android:把文件放入应用可访问的 Rime 目录(应用内「部署」)

### 备份与恢复（手工安装必须执行）

1. 退出输入法或暂停其部署。在用户数据目录外新建一个专用备份目录。
2. 若原来存在 `default.custom.yaml`，把它原样复制到该备份目录并核对内容；
   若不存在，记录「原文件不存在」。**后续升级不得用本包文件覆盖这份首次备份。**
3. 再复制本包文件，包含新的 `default.custom.yaml`。它替换原全局自定义配置，
   因此原文件中的快捷键等自定义项不会继续生效；原始内容仍在备份中。
4. 重新部署并检查方案菜单只有 XHUP Flow。若客户端首次安装向导重写方案列表，
   重新检查 `default.custom.yaml`，不要在菜单另外添加其他方案。
5. 手工卸载或退出独占时，恢复上述原文件；原本不存在则只删除本包的
   `default.custom.yaml`。然后重新部署。不要删除其他方案、`user.yaml`、
   `installation.yaml`、`sync/` 或任何 `*.userdb`。

Trainer 是独立练习工具，不会安装、升级、修复、卸载或部署本方案。
所有复制、首次备份、恢复和重新部署均由用户完成。
旧版管理工具留下的 `.xhup-flow-default-backup.json` 可能含私人原始配置；
先核对并恢复正确的首次备份，不要将当前独占文件误认为原始文件。
不要把配置备份或学习词库随安装包分享。

最后在输入法菜单执行「重新部署」。原子文件替换不等于多文件断电事务；
中断后保留备份，修复前核查磁盘状态。

Before copying, back up the original `default.custom.yaml` outside the user
folder, or record that it was absent. Keep this **first** backup through upgrades.
Copy all YAML files, `xhup_flow.sources.tsv` and `lua/`, then redeploy: the Rime
schema list now contains only XHUP Flow. The replacement also supersedes any
other settings in the old default.custom.yaml; it does not remove system input
methods, other schema files or learning data. Restore the first backup (or remove
our default.custom.yaml if originally absent) when uninstalling, then redeploy.
Trainer is a separate practice application and does not install or manage Rime.
Users perform all backup, copy, restore and deployment steps themselves. Preserve
any existing edits and legacy backups. Retain INSTALL, NOTICE and licenses with the package.

## 隐私 / Privacy

本地运行:无账号、无遥测、无云端同步。组句学习数据仅存于本机
`xhup_flow_user.userdb`,可用 `xhup-cli learning export/import` 备份恢复。

Local only: no account, no telemetry, no cloud sync. Sentence-learning
data lives solely in the local `xhup_flow_user.userdb` and can be
exported/imported with `xhup-cli learning export/import`.
