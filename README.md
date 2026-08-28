# Rime 小鹤音形·全码优先

一个面向中文用户的 Rime 小鹤音形输入方案，采用**全码单字优先**策略，同时保留小鹤双拼连续组词、句子输入和形码辅助选字。

本项目由个人使用配置整理为通用公开版本，不包含设备名称、安装标识、同步 UUID、构建缓存、个人用户词典或输入历史，可直接用于 Windows、macOS 和 Linux 上的 Rime 前端。

## 主要特点

- 小鹤双拼 + 鹤形辅助码。
- 四码单字使用高权重固定候选，适合全码盲打。
- 连续双拼音节可以正常组词、组句。
- 支持“两字母双拼 + 首形码”的三码输入。
- 支持使用 `/` 显式追加一至两个形码辅助选字。
- 默认每页显示 5 个候选项。
- 启用 Rime 本地用户词典，可以学习个人词频，但个人数据不会进入本仓库。
- 方案 ID 使用通用名称 `xhup_fullcode`，不与任何具体电脑或设备绑定。

## 下载

普通用户建议进入本仓库的 **Releases** 页面，下载最新版本：

`rime-xhup-fullcode-vX.Y.Z.zip`

不要下载或复制 `build/`、`*.userdb/`、`installation.yaml` 等 Rime 运行时文件。

## 安装前准备

你需要先安装一个 Rime 输入法前端：

| 系统 | 推荐前端 | 常见用户目录 |
| --- | --- | --- |
| Windows | 小狼毫 Weasel | `%APPDATA%\\Rime` |
| macOS | 鼠须管 Squirrel | `~/Library/Rime/` |
| Linux + Fcitx5 | Fcitx5-Rime | `~/.local/share/fcitx5/rime/` |
| Linux + IBus | IBus-Rime | `~/.config/ibus/rime/` |

如果你的 Rime 用户目录与表中不同，以实际前端显示的“用户文件夹 / 用户目录”为准。

## Windows：小狼毫 Weasel

1. 安装并确认小狼毫能够正常输入。
2. 从 Releases 下载最新的 `rime-xhup-fullcode-vX.Y.Z.zip` 并解压。
3. 右键任务栏中的小狼毫图标，打开**用户文件夹**。
4. 将压缩包中的以下文件复制到用户文件夹：

   ```text
   xhup_fullcode.schema.yaml
   xhup_fullcode.dict.yaml
   xhup_fullcode_fixed_chars.dict.yaml
   flypy_chars.dict.yaml
   flypy_base.dict.yaml
   ```

5. 按下文“启用输入方案”修改或创建 `default.custom.yaml`。
6. 右键小狼毫图标，选择**重新部署**。
7. 打开输入法方案选择界面，选择“**小鹤音形·全码优先**”。

## macOS：鼠须管 Squirrel

1. 安装并确认鼠须管能够正常使用。
2. 下载并解压最新 Release。
3. 打开终端：

   ```bash
   mkdir -p ~/Library/Rime
   open ~/Library/Rime
   ```

4. 将以下文件复制到 `~/Library/Rime/`：

   ```text
   xhup_fullcode.schema.yaml
   xhup_fullcode.dict.yaml
   xhup_fullcode_fixed_chars.dict.yaml
   flypy_chars.dict.yaml
   flypy_base.dict.yaml
   ```

5. 按下文“启用输入方案”处理 `default.custom.yaml`。
6. 从鼠须管菜单执行**重新部署**。
7. 在方案菜单中选择“**小鹤音形·全码优先**”。

## Linux：Fcitx5-Rime

创建用户目录：

```bash
mkdir -p ~/.local/share/fcitx5/rime
```

将 Release 解压后的五个输入方案/词典文件复制进去。例如在解压目录执行：

```bash
cp xhup_fullcode.schema.yaml \
   xhup_fullcode.dict.yaml \
   xhup_fullcode_fixed_chars.dict.yaml \
   flypy_chars.dict.yaml \
   flypy_base.dict.yaml \
   ~/.local/share/fcitx5/rime/
```

然后按下文修改 `default.custom.yaml`，重新部署 Rime。若前端没有提供明显的“重新部署”菜单，可以退出并重新启动 Fcitx5 后再部署/切换方案。

## Linux：IBus-Rime

创建用户目录：

```bash
mkdir -p ~/.config/ibus/rime
```

在 Release 解压目录执行：

```bash
cp xhup_fullcode.schema.yaml \
   xhup_fullcode.dict.yaml \
   xhup_fullcode_fixed_chars.dict.yaml \
   flypy_chars.dict.yaml \
   flypy_base.dict.yaml \
   ~/.config/ibus/rime/
```

然后按下文修改 `default.custom.yaml`，并重新部署或重新启动 IBus-Rime。

## 启用输入方案

### 已经有 `default.custom.yaml`

**不要覆盖原文件。** 在原有 `patch:` 节点中追加：

```yaml
patch:
  schema_list/+:
    - schema: xhup_fullcode
```

如果文件中已经存在 `patch:`，只需要合并 `schema_list/+` 部分，不要再写第二个 `patch:`。

### 没有 `default.custom.yaml`

可以直接复制本项目提供的 `default.custom.yaml`，或者手动创建：

```yaml
patch:
  schema_list/+:
    - schema: xhup_fullcode
  menu/page_size: 5
```

保存后必须执行一次**重新部署**。

## 输入方式

本方案的核心编码逻辑是：

- 两码：按小鹤双拼输入音节。
- 三码：两码双拼后直接追加第一个形码，用于辅助缩小单字候选。
- 四码：完整音形码，单字采用高权重固定候选，适合全码盲打。
- 显式形码辅助：在双拼后输入 `/`，再追加一至两个形码。
- 连续输入：连续双拼音节仍然可以组成词语和句子。

示例的具体编码以小鹤音形规则及本项目词典为准。

## 更新

1. 下载新的 Release。
2. 备份自己的 Rime 用户目录，尤其是个人自定义的 `*.custom.yaml`。
3. 覆盖本项目提供的方案与词典文件。
4. **不要删除自己的 `*.userdb/` 用户词典。**
5. 重新部署。

正常更新项目文件不会要求你提交或公开个人词频。

## 卸载

从 Rime 用户目录删除：

```text
xhup_fullcode.schema.yaml
xhup_fullcode.dict.yaml
xhup_fullcode_fixed_chars.dict.yaml
flypy_chars.dict.yaml
flypy_base.dict.yaml
```

然后从 `default.custom.yaml` 中删除：

```yaml
- schema: xhup_fullcode
```

最后重新部署。

如果你希望同时删除该方案学习到的个人词频，可以在确认不再需要后自行删除对应的 `xhup_fullcode_user.userdb` 数据；普通卸载没有必要删除它。

## 常见问题

### 安装后找不到“小鹤音形·全码优先”

优先检查：

1. `xhup_fullcode.schema.yaml` 是否位于正确的 Rime 用户目录。
2. `default.custom.yaml` 是否包含 `schema_list/+` 和 `xhup_fullcode`。
3. YAML 缩进是否正确，只使用空格，不要使用 Tab。
4. 修改后是否执行了“重新部署”。

### 重新部署时报词典缺失

确认以下三个被主词典引用的文件同时存在：

```text
xhup_fullcode_fixed_chars.dict.yaml
flypy_chars.dict.yaml
flypy_base.dict.yaml
```

### 会不会上传我的输入历史？

不会。本仓库只是静态输入方案和词典。Rime 的用户词频由本机 `userdb` 管理；项目的 `.gitignore` 也显式排除了用户词典、同步目录和设备状态文件。

### 可以和其他 Rime 输入方案共存吗？

可以。使用 `schema_list/+` 是追加方案，不会替换 Rime 原有方案列表。不要直接覆盖已有的 `default.custom.yaml`。

## 仓库文件说明

| 文件 | 用途 |
| --- | --- |
| `xhup_fullcode.schema.yaml` | 输入方案核心配置 |
| `xhup_fullcode.dict.yaml` | 主词典入口 |
| `xhup_fullcode_fixed_chars.dict.yaml` | 四码单字高权重表 |
| `flypy_chars.dict.yaml` | 小鹤音形单字数据 |
| `flypy_base.dict.yaml` | 基础词库 |
| `default.custom.yaml` | 新用户启用方案的参考配置 |
| `NOTICE.md` | 上游来源与授权说明 |
| `VERSION` | 当前发布版本 |

## 隐私说明

公开仓库不会包含以下内容：

- `installation.yaml`：可能含设备安装 ID。
- `user.yaml`：本机运行状态。
- `sync/`：Rime 同步数据及设备标识。
- `*.userdb/`、`*.userdb.txt`：个人词频、自造词及输入学习数据。
- `build/`、`*.bin`：本地部署生成文件。

不建议把整个 Rime 用户目录直接提交到公开 Git 仓库。

## 上游与授权

本项目使用的小鹤相关词典数据来源及授权信息见 [NOTICE.md](NOTICE.md)。项目不是小鹤官方项目，也不代表小鹤官方立场。

仓库按 [LGPL-3.0](LICENSE) 授权文件所述条款发布。第三方词典或数据仍遵循各自的来源与授权要求。

## 版本发布

仓库通过 GitHub Actions 自动校验配置、补齐固定来源的基础词典、生成 ZIP 与 SHA-256 校验文件，并发布对应版本的 GitHub Release。
