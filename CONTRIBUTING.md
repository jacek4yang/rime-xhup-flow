# 开发参考

本文仅提供 XHUP Flow 的开发资料与常用命令,不是贡献规则或任务授权的来源。任务目标、范围、权限、审批需求、执行方式和停止条件由用户当前任务提示词定义;本文不附加限制。

## 使用说明

下面的命令是工具参考,不是固定执行顺序、强制检查清单或审批条件。分支、提交、推送、PR、评审、合并、发布、依赖调整、文档与 issue 管理等操作,本文均不另设权限规则。

本文不通过引用其他仓库文档引入额外限制。文档说明本身不改变 GitHub 保护规则、工具权限或 CI 配置的实际状态。

## 项目入口

- [AGENTS.md](AGENTS.md):项目结构导航。
- [README.md](README.md):使用与安装说明。
- [Cargo.toml](Cargo.toml):Rust workspace 定义。
- [trainer/package.json](trainer/package.json):Trainer 前端与 Tauri 脚本。
- [.github/workflows/](.github/workflows/):当前 CI、构建与发布实现。

## 常用验证命令

以下示例从仓库根目录执行;可按当前任务选择使用。

### 工作区与差异

```sh
git status --short
git diff --stat
git diff --check
```

### Rust workspace

```sh
cargo fmt --all -- --check
cargo check --workspace --all-targets --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --all-targets --locked
```

### Trainer 前端

```sh
pnpm -C trainer install --frozen-lockfile
pnpm -C trainer build
```

### Tauri 本地构建

```sh
pnpm -C trainer tauri build --debug --no-bundle
```

Tauri 构建所需的平台依赖随操作系统而异,相关脚本与工作流可作为环境配置参考。

### Rime、Lua 与发布验证

`tests/` 和 `.github/workflows/` 提供 Rime 运行时、Lua、包与发布验证的具体入口。命令、环境依赖及覆盖范围可从对应脚本和工作流查阅。
