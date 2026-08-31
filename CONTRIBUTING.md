# 贡献指南

本文档定义本仓库的开发工作流,同时适用于人类贡献者与 AI 代理。仓库包含根目录的 Rime 输入方案与词典、Rust workspace(`crates/`)和 `trainer/` 桌面应用(Tauri 2 + React)。仓库采用「人类操作员 + 长时运行代理」模式:操作员拥有架构、范围、评审与合并决策权;代理在同一会话中按任务逐个实现。

## 分支策略

- `main` 受保护,禁止直接在其上开发。
- 一个任务 = 一个分支 = 一个 PR。
- 分支命名:`<type>/<简短描述>`,如 `docs/...`、`fix/...`、`chore/...`、`feat/...`,类型与 Conventional Commits 一致。
- GitHub 是已提交项目状态的唯一权威来源;本地工作以 `origin/main` 为准。

## 开发工作流

每个任务按以下顺序执行:

1. **检查状态**:确认当前分支与工作区;新任务从最新 `main` 切出分支。
2. **按需阅读**:只读取任务所需的文件。
3. **计划**:编辑前给出简洁的实现计划(文件、步骤、验证命令),等待操作员批准。
4. **实现**:只实现已批准的范围;不做无关重构,不新增非必要依赖。
5. **验证**:先做聚焦验证,再跑项目要求的检查。
6. **自查**:运行 `git diff --check`,并通读最终 diff,确认无意外或无关改动。
7. **提交**:Conventional Commits,中文描述,提交保持聚焦。
8. **推送与 PR**:推送当前分支,创建目标为 `main` 的 PR。
9. **评审与合并**:由操作员评审并 squash 合并;代理绝不自行合并。
10. **下一任务**:合并后更新本地 `main`,切出下一分支,在同一会话中继续。

## 长时运行代理规则

- 同一代理会话跨任务保持活跃,以最大化上下文复用。
- 代理只实现当前指派的任务;架构与范围决策由操作员做出。
- 除非任务要求,不新增依赖;除非被明确要求,不创建 GitHub issue,不撰写大型设计文档。
- 优先沿用仓库现有模式与约定。
- PR 创建后只汇报:摘要、变更文件、已执行验证、commit SHA、PR URL。

## 最小 token 开发规则

- 复用会话中已掌握的信息;不重复扫描未改动的文件;不重复输出仓库概述。
- 只检查与当前任务相关的文件;优先精准命令而非全仓扫描。
- 状态汇报保持简洁;不解释显而易见的 shell 命令。
- 避免不必要的网络搜索与推测性架构讨论。
- 保留依赖与构建缓存;除非被明确要求,绝不运行缓存清理命令。

## 提交规范

- 使用 Conventional Commits:`<type>: <中文描述>`,如 `docs: ...`、`fix: ...`、`chore: ...`。
- 提交保持聚焦:一个提交只做一件事;bug 修复不顺带清理周边代码。
- 不提交与任务无关的格式化、重命名或元数据变更。

## 入库规则

以下内容**绝不提交**:

- 构建产物与部署生成文件(`build/`、`*.bin`、`target/`、`dist/` 等)。
- 运行时状态与机器相关文件(`installation.yaml`、`user.yaml`、`sync/` 等)。
- 本地缓存与依赖目录(`node_modules/` 等)。
- 密钥与凭据(`.env`、私钥、访问令牌等)。
- 个人数据(`*.userdb/`、`*.userdb.txt` 等用户词典与输入学习数据)。
- AI 工具的本地会话、状态与缓存数据。

可复现生成的项目产物,仅在仓库策略明确要求跟踪时才可提交——例如 `xhup_fullcode_fixed_chars.dict.yaml` 由发布流程生成并按策略固化跟踪。

`.gitignore` 已覆盖主要的本地生成物;引入新的本地状态类文件时,应同步更新 `.gitignore`。

## 验证要求

- 所有变更:`git diff --check`、`git status --short`、`git diff --stat`,并人工审查最终 diff。
- Rust 变更:`cargo fmt --all -- --check`、`cargo check --workspace --all-targets --locked`、`cargo clippy --workspace --all-targets --locked -- -D warnings`、`cargo test --workspace --all-targets --locked`。
- trainer 变更:`pnpm -C trainer install --frozen-lockfile`、`pnpm -C trainer build`;需要验证完整 Tauri 管线时(仅本地):`pnpm -C trainer tauri build --debug --no-bundle`。
- PR 与 main 推送由 `.github/workflows/ci.yml` 强制执行上述 Rust 与 trainer 检查;本地提交前应先跑通相同命令。
- 修改 YAML 方案或词典时:校验 YAML 语法(缩进只用空格),并在 PR 中说明验证方式。
- 行为变更必须是有意为之:在 PR 中说明变更内容、原因与验证方式。
- 治理、文档类变更不得改动输入方案与词典文件。
- 发布流程(`.github/workflows/release.yml`)会执行词典来源校验、方案完整性与隐私文件检查。
