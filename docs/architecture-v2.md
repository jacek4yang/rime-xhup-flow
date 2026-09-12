# XHUP Flow 2.0 架构

状态：**里程碑 1 基础与里程碑 2 知识库已合并；规则模型独立交付，生产行为未改变**。umbrella 追踪见
[GitHub #83](https://github.com/jacek4yang/rime-xhup-flow/issues/83)。v1 当前实现与
发布契约仍以 [architecture.md](architecture.md) 为准；本文件定义后续 PR 的目标
边界和可执行顺序。

## 目标数据流

```text
XHUP Knowledge Base
  Unicode identity / linguistic readings / XHUP sound / shape
  attested + generated codes / frequency / variants / evidence
        ↓
XHUP Flow Compiler (Rust, deterministic + explainable)
  prefix-closed trie / candidate slots (code, rank)
  XHUP rules + lexical/corpus evidence + migration costs
        ↓
compiled deterministic artifacts
  native Rime tables + compact model/index metadata
        ↓
librime + mandatory librime-lua (main xhup_flow)
        ↓
Contextual Joint Decoder
  raw keys + committed left context + composition + local user state
  joint key/text lattice → bounded Beam/Viterbi → deterministic Top-K
        ↓
local user learning (versioned, resettable, private)
```

`xhup_flow_static` 永久保留为静态兼容、调试与应急回退方案。2.0 主方案缺少
librime-lua 时必须明确失败并给安装诊断，不能继续以“完整 Flow”名义静默降级。

## 不变量

1. `xhup-flow-v1.0.0` 是不可变兼容输入。`xhup_flow_static` 永久冻结精确映射、
   菜单顺序/top1 与行为；智能 `xhup_flow` 保留接受的 legacy/XHUP 码和文本可达性，
   可在明确政策、基准与回归测试下动态排序。`xhup_flow_user` 身份与迁移语义不变。
2. 编译器决定码位质量，不决定有效文本是否可达。词库影响排名，不定义可达语言：

   ```text
   hot static lexicon ⊂ extended lexicon ⊂ open-composition reachable text
   ```

3. `CoreStandardHanzi` 只是规范语言学子集；`InputHanzi` 是证据驱动、可扩展的
   生产集合。`linguistic reading != XHUP input sound code` 是类型边界，禁止为可
   编码性伪造语言学事实。
4. 生产 build/test/deploy/runtime 全离线。所有上下文评分与学习留在本地；普通
   Debug/日志不得含 committed text。canonical 输出不含时间、机器路径、主机名或
   随机顺序。
5. 解码必须显式有界。Lua 不承载巨型词典、不做每键磁盘 I/O、不物化无界候选，
   弱证据向 v1 确定性顺序回退。
6. `valid prefix != commit boundary`。例如 `uior` 的 `u/ui/uio/uior` 都是合法查询
   状态，节点是有序候选空间而非某个词的专属码；短码首选不自动提交。

机器可读的 v1 锚点在
[`data/benchmarks/v1-static-baseline.json`](../data/benchmarks/v1-static-baseline.json)，
由 Rust 集成测试从真实 canonical API 重建，不依赖本文件中的手写计数。

## crate 与运行时边界

多源字符证据、确定性来源与冲突审计见 [knowledge-base.md](knowledge-base.md)。
官方规则、历史实现、Flow 编译政策/扩展的边界和独立夹具见 [xhup-rules.md](xhup-rules.md)。

| 层 | 当前/目标职责 |
| --- | --- |
| `xhup-core` | XHUP 键、音节、音形、字符身份等底层事实；继续不包含候选排序 |
| `xhup-decoder` | 平台无关的 runtime context、lattice/path、固定点 scorer 契约与后续 bounded decoder |
| `xhup-generator` | 知识库解析、证据决议、Flow Compiler 与确定性 artifact 生成 |
| `xhup-analyzer` | differential audit、benchmark、replay、参数扫描与解释报告 |
| native Rime tables | 大规模字符/词汇 lookup；不把百万条目复制进 Lua |
| `lua/xhup_flow/` | 真实 committed context 获取、策略/分段编排、bounded decoding、候选融合、调试解释 |
| Trainer/Tauri | 展示解释与平台诊断；Tauri 仍是薄平台层，不复制领域逻辑 |

里程碑 1 新增的 `xhup-decoder` 不链接 librime，也不改变生成包。它是 Rust 参考语义，
用于先把 span、路径、分数与确定性决胜做成可测契约；后续 Lua 实现必须用共享 fixture
逐项对齐该契约。

## runtime context

```text
RuntimeContext {
  committed_left: local text (Debug redacted),
  composition: non-empty validated KeySequence
}
```

当前只保存 committed left text 与未提交按键，足以让后续 scorer 接入真实 librime-lua
API。用户模型、recency 等信号应以独立版本化只读快照加入，不能写回 Lua 源文件。

## lattice 与路径

`Lattice` 绑定一条原始按键序列。edge 是严格前进的半开按键 span：

```text
LatticeEdge {
  span: [start, end),
  candidate: { text, kind, frequency }
}
```

同一 span、重叠 span 与不同长度 span 可同时存在，候选来源类别不决定可达性。
生产适配将接收 prefix-space 合同：同一位置可同时有 `u/ui/uio/uior` 的变长 key spans，
每条边连接字符、词、扩展词、用户词、别名或 OOV 原语。目标是联合最大化
`P(text, word_segmentation, key_segmentation | raw_keys, left_context, user_state)`，
不能先固定按键分段再只优化文本。
完整路径必须从 `0` 连续覆盖到 `raw_input.len`，同时表达 surface text 和 segmentation。
里程碑 1 的 `complete_paths(limit)` 仅用于小 fixture，调用必须传非零硬上限并返回
`truncated`；生产实现不得先全量枚举，将改为按 position/state 合并的 bounded
Beam/Viterbi。

## scorer 契约

`DeterministicScorer` 输入 immutable context + lattice + path，输出固定点 `i64` 分数与
breakdown。相同状态必须得到相同分数；最终决胜不依赖 HashMap 顺序或浮点比较。

当前 `word-frequency-segmentation/v1` baseline：

```text
score = Σ fixed_point_log2(word_frequency + 1)
      - segment_count × segment_penalty
```

频率为零表示“没有证据”，绝不表示不可达。baseline 刻意不消费 left context；提交的
歧义 fixture 因而只有 50% contextual segmentation accuracy。这是可见的待改进基线，
不是“上下文解码已完成”的声明。

后续 breakdown 在不改变 fixed-point/确定性约束下加入：word bigram/trigram、character
n-gram backoff、static compatibility、ambiguity/confidence、user selection/recency 与
安全回退项。

## benchmark v1

格式与来源见 [data/benchmarks/README.md](../data/benchmarks/README.md)。关键约束：

- schema 固定为 `xhup-contextual-benchmark/v1`，未知字段与错误版本直接拒绝；
- development 与 evaluation 必须同时存在并保持分离；
- 每例显式给出 left context、同一 raw input 上的全部 fixture edges 与 expected path；
- 同 raw input + 不同 left context + 不同期望分段是 contextual case；
- `topK` 与 `maxPathsPerCase` 都是文档内硬界限；
- CLI 报告 top1 text、top1/top-k path、segmentation/context accuracy、fanout、路径展开、
  truncated case 与 p50/p95/p99；计时不作跨机器硬阈值。

seed fixture 来自已固定万象词频与 KdConv 派生存在性证据，先证明多路径结构和基线
缺口。它不是完整 2.0 评测集；后续必须在不泄漏 evaluation 的前提下扩展口语、粒子、
生僻/OOV、技术词、长句与真实同音异词。

## 静态与动态层策略

当前 v1 运行时仍优先保护静态候选，本规则 PR 不改它。该实现不是智能 Flow 的永久
排名限制：产品政策已允许上下文改变主方案首选，必须有界、可解释、可测量，并保留
接受码与开放可达性。`xhup_flow_static` 则始终保持 v1 精确顺序与行为。
正常菜单只显示最小 ASCII 注释；丰富 `[CTX]/[USR]/[OOV]` 信息限显式 debug/Trainer。

## 交付顺序

1. **已建立**：v1 机器基线、本文、context benchmark v1、runtime context、lattice/path、
   deterministic scorer、frequency+segmentation baseline 与 CI 门禁；生产行为零变化。
2. **已合并 #85**：知识库 evidence schema、冲突决议、来源/许可与“为何支持”审计；无新字符硬上限。
3. XHUP 双拼/形码/全码/简码规则形式化与 v1 compatibility fixture。
4. Prefix-Space Compiler v3：前缀闭合 trie、多位置候选槽位、全局目标与 v2 对照基准；
   先评估 assignment/min-cost-flow 加拥塞/熵项迭代重加权，不只调 v2 系数或优化 KSPC。
   频率、击键/rank/collision、前缀/邻域外部性、分段歧义、XHUP 偏离及迁移成本都需解释。
5. 独立候选注释清理；`xhup_flow` mandatory Lua 合同、模块加载诊断、包装与 librime E2E。
6. native lookup → production key/text lattice 与候选融合；接入真实 committed context 和 n-gram scorer。
7. bounded Beam/Viterbi 联合 key/text Top-K、低置信度回退、解释 breakdown 与性能基准。
8. 本地用户 context/segmentation 学习、版本/迁移/损坏回退/重置/导入导出。
9. Trainer explanation、首次 deploy 优化、Windows/Linux/macOS/Android 验证和 2.0 发布。

在第 6–8 步通过真实 librime-lua、同输入不同上下文不同 top path、OOV 可达、v1 static
差异 0、学习重启持久化及 p95/p99/内存门禁前，不得宣称 contextual decoder 完成。
