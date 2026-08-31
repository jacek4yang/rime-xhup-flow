# AGENTS.md

AI 代理在本仓库工作的入口约束。详细流程见 [CONTRIBUTING.md](CONTRIBUTING.md);用户文档见 [README.md](README.md)。

## 项目概述

本仓库是 XHUP Flow 项目,包含:

- 根目录的 Rime「小鹤音形·全码优先」输入方案与词典(静态 YAML,行为保持不变);校验、打包与发布由 `.github/workflows/release.yml` 完成,各文件用途见 README 的「仓库文件说明」一节。
- Rust workspace(`crates/xhup-core`、`crates/xhup-analyzer`、`crates/xhup-generator`、`crates/xhup-cli`):承载领域逻辑。
- `trainer/`:Tauri 2 + React + TypeScript + Vite + pnpm 桌面应用,`trainer/src-tauri` 是 workspace 成员。
- `data/`、`rime/`:预留目录。

架构边界:Rust 负责领域逻辑;React 负责展示与交互;Tauri 仅作薄平台层。

## 硬性规则

- `main` 受保护,不在其上直接开发;GitHub 是已提交项目状态的唯一权威来源。
- 一个任务 = 一个分支 = 一个 PR;PR 目标为 `main`;代理绝不自行合并 PR。
- 人类操作员拥有架构、范围、评审与合并决策权;代理只实现当前指派的任务,不做无关重构。
- 提交使用 Conventional Commits(中文描述,与提交历史一致),提交保持聚焦。
- 除非任务要求,不新增依赖;除非被明确要求,不创建 GitHub issue,不撰写大型设计文档。
- 优先沿用仓库现有模式与约定。

## 每个任务的流程

1. 检查当前分支与工作区状态。
2. 只读取任务所需的文件。
3. 编辑前给出简洁的实现计划(文件、步骤、验证命令),等待批准。
4. 只实现已批准的范围。
5. 先做聚焦验证,再跑项目要求的检查。
6. 运行 `git diff --check`,并审查最终 diff 有无意外或无关改动。
7. 提交、推送分支、创建目标为 `main` 的 PR;不合并。
8. PR 创建后只汇报:摘要、变更文件、已执行验证、commit SHA、PR URL。

## 入库规则

绝不提交:构建产物(`build/`、`target/`、`dist/`、`*.bin` 等)、运行时状态、本地缓存与依赖目录(`node_modules/` 等)、机器相关文件(如 `installation.yaml`、`user.yaml`、`sync/`)、密钥与凭据、本地 AI 会话数据、个人用户词典与输入学习数据。

可复现生成的项目产物,仅在仓库策略明确要求跟踪时才可提交——例如 `xhup_fullcode_fixed_chars.dict.yaml` 由发布流程生成并按策略固化跟踪。

`.gitignore` 已覆盖主要的本地生成物;新增本地状态类文件时应同步更新 `.gitignore`。

## 最小 token 规则

- 复用本会话已掌握的信息;不重复扫描未改动的文件;不重复输出仓库概述。
- 只检查与当前任务相关的文件;优先精准命令而非全仓扫描。
- 状态汇报保持简洁;不解释显而易见的 shell 命令;不做推测性架构讨论。
- 避免不必要的网络搜索。
- 保留依赖与构建缓存;除非被明确要求,绝不运行缓存清理命令。

## 验证

- 通用:`git diff --check`、`git status --short`、`git diff --stat`。
- Rust:`cargo check --workspace`、`cargo test --workspace`。
- trainer 前端:`pnpm -C trainer build`;完整 Tauri 管线:`pnpm -C trainer tauri build --debug --no-bundle`。
- 修改 YAML 方案或词典时:校验 YAML 语法,并在 PR 中说明验证方式;发布流程会执行完整性与隐私文件检查。
