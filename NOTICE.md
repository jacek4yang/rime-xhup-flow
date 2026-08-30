# 来源、致谢与再分发说明

本项目是一个面向通用 Rime 环境整理的小鹤音形输入方案，不是小鹤官方项目。

## 上游来源

基础字词数据来自社区项目：

- `boomker/rime-fast-xhup`
- https://github.com/boomker/rime-fast-xhup

小鹤输入法及编码规则相关资料：

- https://flypy.cc/

本项目固定使用经过校验的上游 Git Blob，而不是在构建时直接跟随上游 `main` 的最新内容，以避免上游更新导致同一版本的 Release 内容发生变化。

### `flypy_base.dict.yaml`

来源仓库：`boomker/rime-fast-xhup`

固定 Git Blob：

```text
8c3303f5cf46f03c9e60fd64f6436bab9eaec294
```

SHA-256：

```text
214f963d1f2f112599c7a6f466ffc1f93f213ef0c018e015d186e34b75f13fbb
```

### `flypy_chars.dict.yaml`

来源仓库：`boomker/rime-fast-xhup`

固定 Git Blob：

```text
3c2773335c8108bbe896b9af588d619e22045132
```

SHA-256：

```text
5eeda7a9976cf7d8ed1bc487bdccc02d7f423425515d814309328ff61b6b4291
```

### `xhup_fullcode_fixed_chars.dict.yaml`

该文件由本项目的 GitHub Actions 根据固定版本的 `flypy_chars.dict.yaml` 可重复生成：

1. 移除编码中的 `~` 分隔符；
2. 在原单字权重基础上增加 `1000000000`；
3. 使用独立词典名 `xhup_fullcode_fixed_chars`。

生成结果 SHA-256：

```text
2279019377051ceaf264c77f09dc2653623ec30d6297a1e40b13ee50044f1507
```

这样可以在四码完整音形输入时提高单字固定候选的优先级，同时保持原始单字数据可追溯。

## 本项目做出的通用化调整

公开版本主要做了以下整理：

- 将设备相关 schema ID 改为通用的 `xhup_fullcode`；
- 移除设备名称、安装标识和同步 UUID；
- 移除 `sync/`、`userdb`、`build/` 等运行时/个人数据；
- 将四码单字高权重表改为可重复生成；
- 增加跨平台中文安装教程和自动 Release 流程。

## 许可证

仓库保留上游配置随附的 LGPL-3.0 许可证文本，见 [`LICENSE`](LICENSE)。

如果以后从其他项目导入新的词库、Lua 插件或配置文件，请在合并和再分发前单独确认对应来源的许可证、署名和再分发要求，不应默认认为所有 Rime 词库都采用同一种许可证。
