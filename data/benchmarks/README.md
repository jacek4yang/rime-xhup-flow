# XHUP Flow benchmarks

本目录只保存可再分发、可审计、确定性的 benchmark fixture 与基线；不保存
原始用户输入、运行时学习状态或许可不允许再分发的语料。

## `v1-static-baseline.json`

`xhup-v1-static-baseline/v1` 是 `xhup-flow-v1.0.0`（`778bfa5`）的兼容快照。
`crates/xhup-analyzer/tests/v1_baseline_snapshot.rs` 从真实 canonical API 重建字符、
词汇、静态码位、Rime 源包与 replay 数值并逐项比较。机器相关的 deploy/RSS
观测只作带环境来源的参考，不作为跨机器门槛。

## `contextual-v1.json`

`xhup-contextual-benchmark/v1` 的顶层字段：

- `topK`：路径 top-k 命中统计的 k；
- `maxPathsPerCase`：参考枚举器的硬上限；生产 Beam/Viterbi 不复用全路径枚举；
- `cases`：每条 case 明确 `development` / `evaluation` split、来源、左上下文、
  原始按键、lattice edges 与期望分段。

edge span 是原始 ASCII 按键的半开区间 `[start,end)`。`frequency=0` 表示缺少
证据，不表示不可达。候选类别当前支持 `hot-word`、`extended-word`、
`character`、`attested-alias`、`user-learned`、`oov-composition`。

首批 fixture 只锁定联合分段基础，不冒充完整 2.0 评测集：两组同音节序列均有
两个完整分段，并各自以不同 left context 期望不同路径。词频来自 pinned 万象
base，字符频率来自 pinned 万象读音分数；会话存在性由 KdConv 派生统计交叉验证。
`contextual-baseline-v1.json` 刻意记录 context-free baseline 只有 50% 上下文分段
命中，防止后续把“表内组句”误报为上下文改进。

运行：

```bash
cargo run --locked -p xhup-analyzer --bin contextual-bench -- \
  --input data/benchmarks/contextual-v1.json \
  --baseline data/benchmarks/contextual-baseline-v1.json
```

计时 p50/p95/p99 会报告但不进入跨机器硬门槛；准确率、路径数、截断数必须与
基线精确一致。fixture 扩展时应保持 development/evaluation 分离，避免用
evaluation case 调参。
