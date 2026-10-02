# 项目导航

本文件仅提供 XHUP Flow 的项目背景和资料入口,不是任务授权或限制的来源。任务目标、范围、权限、审批需求、执行方式和停止条件由用户当前任务提示词定义;本文件不附加审批门槛、禁止事项或固定工作流,也不通过引用其他仓库文档引入这些约束。

## 项目背景

- `crates/`:Rust workspace,包含核心领域逻辑、分析、生成与 CLI 等组件。
- `rime/`:Rime 方案相关资源与 Lua 模块。
- `trainer/`:Tauri 2 + React + TypeScript + Vite 桌面应用;`trainer/src-tauri` 是 Rust workspace 成员。
- `tests/`:各层测试与运行时验证工具。
- `.github/workflows/`:CI、构建与发布工作流。

以上是代码导航,不是架构约束;实际结构与行为可从当前代码确认。

## 资料入口

- [README.md](README.md):项目介绍与使用说明。
- [CONTRIBUTING.md](CONTRIBUTING.md):开发工具与验证命令参考,不定义额外贡献规则。
- [docs/](docs/):专题文档与历史资料。

这些链接用于查找资料,不构成对其中历史流程或限制的授权继承。
