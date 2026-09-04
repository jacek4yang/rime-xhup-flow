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
| `xhup_flow.schema.yaml` | 主方案(Flow:静态层 + 组句 + 本地学习) |
| `xhup_flow_static.schema.yaml` | 静态回退方案(仅固定层,无组句学习) |
| `xhup_flow.dict.yaml` | 顶层词典(导入下列词典) |
| `xhup_flow_chars.dict.yaml` | 单字全码(2/3/4 码) |
| `xhup_flow_words.dict.yaml` | 固定词语层(4/6/8 键) |
| `xhup_flow_shortcuts.dict.yaml` | 词语简码(3~7 键零冲突别名) |
| `xhup_flow_two_key_shortcuts.dict.yaml` | 二码零冲突词语简码 |
| `xhup_flow_fixed_first_shortcuts.dict.yaml` | FIXED_FIRST 词语简码 |
| `xhup_flow_flow.dict.yaml` | Flow 组句词典 |
| `xhup_flow_learn.dict.yaml` | Flow 学习词典 |
| `INSTALL.md` | 本说明(部署时无需复制) |

两套方案同时安装;在输入法的方案菜单中选择 Flow 或 Static。

## 安装 / Install

把上表中除 `INSTALL.md` 外的全部文件复制到 Rime 用户数据目录:

- Windows 小狼毫:`%APPDATA%\Rime`
- macOS 鼠须管:`~/Library/Rime`
- Linux Fcitx5:`~/.config/fcitx5/rime`(或 `$XDG_CONFIG_HOME/fcitx5/rime`)
- Linux IBus:`~/.config/ibus/rime`(部分发行版为 `~/.config/ibus/rime`)
- fcitx5-android:把文件放入应用可访问的 Rime 目录(应用内「部署」)

然后在输入法菜单执行「重新部署」。Trainer 桌面应用的
「输入法」控制中心可以自动完成上述安装、升级、修复与卸载。

Copy all files except `INSTALL.md` into the Rime user data directory
listed above, then trigger "Redeploy" from the input method menu. The
Trainer desktop app's Input Method control center automates install,
update, repair and uninstall.

## 隐私 / Privacy

本地运行:无账号、无遥测、无云端同步。组句学习数据仅存于本机
`xhup_flow_user.userdb`,可用 `xhup-cli learning export/import` 备份恢复。

Local only: no account, no telemetry, no cloud sync. Sentence-learning
data lives solely in the local `xhup_flow_user.userdb` and can be
exported/imported with `xhup-cli learning export/import`.
