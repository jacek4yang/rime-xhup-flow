# Optimizer v2:候选位资源、XHUP 先验与参数扫描

状态:**设计中**。前置阅读:docs/input-model-v2.md(目标与证据模型)、
docs/data-pipeline.md(证据来源)、docs/lua-runtime.md(运行时边界)。
现有实现:crates/xhup-analyzer(v1 优化器,保留并扩展,不重写)。

## 0. v1 的缺口

v1 优化器(deterministic heuristic)已产出 ZR/FF/二码 canonical 层,但:

- 成本模型近似「节省键数 + 简单扰动」,不建模候选位(rank)价值;
- 频率证据只有万象聚合分数一个信号;
- 无传统小鹤设计先验,优化结果可能偏离用户肌肉记忆;
- 参数来自直觉 + 局部 sensitivity,没有系统扫描与选型记录。

v2 的目标:**期望真实输入成本**最小化(docs/input-model-v2.md §1),
证据来自 LexicalEvidence(多信号、缺失显式),过程可解释、可复现。

## 1. 候选位是资源

码位 = 有序候选位序列(rank 1/2/3/…),不是空闲/占用二值:

- `CandidateSlot { code, rank, occupant, occupant_utility }`;
- 占用已有码的 rank 2 可能值得;新码 rank 1 与旧码 rank 2 的成本不同;
- 选择成本随 rank 递增(默认:rank1 < rank2 ≪ rank3 ≪ 翻页);
- 扰动成本 = 被挤后排的既有候选的效用损失 × 其频率。

## 2. 成本模型 v2(全部参数集中、可扫描)

```rust
CostModelV2 {
    // 击键与选择
    key_cost: f64,               // 每键基础成本
    rank_cost: [f64; 4],         // rank1..rank4+ 选择成本
    pagination_cost: f64,        // 翻页
    // 歧义与扰动
    ambiguity_coeff: f64,        // 同码竞争强度系数
    disruption_coeff: f64,       // 扰动既有候选系数
    // XHUP 先验与认知
    xhup_deviation_coeff: f64,   // 偏离传统映射的代价
    cognitive_complexity_coeff: f64, // 码可记忆性(与读音/模式的一致性)
    // 数据质量
    rare_pollution_coeff: f64,   // 长尾词污染惩罚
    context_failure_coeff: f64,  // 语境失效
    domain_balance_coeff: f64,   // 域均衡
}
```

频率/语言侧:

```rust
EvidenceWeights {
    global_share: f64,           // 万象归一化频率权重
    conversation_share: f64,     // 会话域权重
    formal_share: f64,           // 正式域(待来源)
    sentence_coverage_weight: f64,
    context_diversity_weight: f64,
}
```

缺失信号的处理规则:权重重归一化到已测量信号(绝不把 None 当 0
计入);EvidenceCoverage 审计报告每个信号的有效权重。

## 3. XHUP 风格先验

`XhupStylePrior(word, code, rank)` 特征:

- 与传统参考映射一致(兼容率工具 compat 模块提供度量);
- 简码长度与传统一致;音码前缀一致;形码一致;
- 传统候选位;码可记忆性;约定俗成度;迁移成本。

参考数据**本地提供、绝不入库**(许可红线,docs/data-pipeline.md);
prior 的强度是参数(`xhup_deviation_coeff`),扫描决定。
优化器可以偏离传统映射 —— 但偏离必须有真实证据支撑且被记录到
对照报告(兼容率 × 期望成本改善,供评审)。

## 4. 分层策略

| 层 | 键数 | 策略要点 |
|---|---|---|
| 一级简码 | 1 | 极稀缺;强先验;默认冻结传统映射,偏离需显著证据 |
| 二码 | 2 | 候选位建模主战场之一;rank 价值显式 |
| 三码 | 3 | 空间大;系统化优化主战场;认知约束防混乱 |
| 全码 | 4/6/8 | 机械推导,非优化对象 |

各层独立参数子集,扫描按层进行。

## 5. 扫描与选型

扩展现有 sweep 框架:网格/随机采样参数空间,对每个工作点产出:

- Expected Input Cost(语料回放,见 benchmark 方法论文档);
- XHUP 兼容率(1/2/3 键分层 + 候选序相似度);
- top-100/top-1000 常用词质量(可达 rank 分布);
- 候选 fanout、简码稳定性(相邻工作点间映射变动率)。

选型 = Pareto 前沿上的人工评审点(成本 × 兼容性 × 稳定性),
记录选型理由到报告。不允许「跑一次取默认」。

## 6. 可解释性(发布门禁)

每个 production 简码决策可输出理由卡:

```text
词 / 码 / 预期 rank
global frequency / conversation frequency / sentence coverage
XHUP prior 得分 / 节省键数 / fanout / 期望成本增量
选中理由(主导项)
```

## 7. 不变量与门禁

- 碰撞共存:词汇存在性不受碰撞影响(已有守卫);
- 常用词 P0:top-100 100% 在库、可达 rank ≤3(已有守卫);
- 优化输出确定性:同输入同参数 → 字节一致;
- 映射冻结:v1.0.0 起 core 静态简码为 1.x 兼容契约;
- 冻结前的映射变更必须有对照报告(compat)+ 语料回放数据。
