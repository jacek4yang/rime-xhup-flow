# Input Model v2:XHUP Flow 输入模型重构

状态:**已实现并作为 v1.0.0 canonical v2 的输入模型**。映射选型、
语料回放与运行时门禁见 [optimizer-v2.md](optimizer-v2.md)。

本文档定义 v1.0.0 之前必须完成的输入模型重构:词汇证据、优化目标、
简码分层、候选位建模、XHUP 设计先验与验收门禁。工程平台(CI/打包/部署)
已就绪;本文档针对的是**语言数据模型**的不足。

## 0. 背景:当前模型的问题

现行管线(canonical data → xhup-core → xhup-generator → Rime 包)在工程上
确定、可复现,但语言决策依据过薄:

- 每个词只有一个孤立的万象聚合频率整数,没有语料/上下文证据;
- 简码分配把码位当「空闲/占用」二值资源,不建模候选位(rank)价值;
- 碰撞语义错误(已修,见 §3):词与单字同码时词被整体删除;
- 优化目标近似「节省键数」,不是「期望真实输入成本」;
- 缺少对传统小鹤音形设计约定的显式建模,优化结果可能偏离用户肌肉记忆。

数学优化方向本身是对的,不能退回手写简码表;要补的是**证据与目标函数**。

## 1. 优化目标:期望真实输入成本

优化器最小化期望真实输入成本,而非键数:

```text
ExpectedInputCost =
    KeyCost                    // 击键成本
  + CandidateSelectionCost     // 候选选择成本(按 rank 递增)
  + PaginationCost             // 翻页成本
  + AmbiguityCost              // 歧义成本(同码竞争强度)
  + CorrectionCost             // 改错成本
  + CognitiveCost              // 认知成本(码可记忆性/可预测性)
  + MuscleMemoryDeviationCost  // 偏离传统映射的成本
  + ContextFailureCost         // 语境失效成本
```

词条-简码的效用概念模型(不必线性,但每个信号必须显式、可测):

```text
Utility(word, shortcut) =
    α·global_frequency + β·conversational_frequency
  + γ·sentence_coverage + δ·context_diversity
  + ε·keystrokes_saved + ζ·xhup_style_prior
  + η·scenario_coverage + θ·lexical_quality
  - λ1·selection_cost - λ2·ambiguity_cost - λ3·disruption_cost
  - λ4·muscle_memory_deviation - λ5·rare_word_pollution
  - λ6·semantic_mismatch_risk
```

## 2. 词汇证据模型(LexicalEntry)

「词 + 一个频率整数」不是充分证据。逻辑模型:

```text
LexicalEntry {
    text, readings,
    source_evidence[],            // 来源与出处
    raw_frequency_by_source,      // 各来源原始频率
    normalized_frequency,         // 归一化总频率
    conversation_frequency,       // 会话/口语域
    formal_frequency,             // 正式文体域
    technical_frequency,          // 技术域
    social_frequency,             // 社交域
    sentence_coverage,            // 在真实句子中的出现覆盖
    context_diversity,            // 独立左/右上下文数量
    lexical_quality,              // 词法质量(人工筛选/共识)
    pronunciation_confidence,     // 读音置信度
    source_consensus,             // 多来源一致性
    flags,
}
```

不必全部持久化;可推导的字段由管线推导,但优化器必须拿到等价证据。

### 2.1 资格分离(替代「碰撞即删除」)

```text
LexicalEligibility    // 是否存在于词典
ShortcutEligibility   // 是否允许竞争稀缺简码位
FlowEligibility       // 是否参与组句语言模型
LearningEligibility   // 是否参与本地学习
```

示例:生僻专名 = 词典收录 ✓ / 简码 ✗ / 组句 视情况;高频口语词 = 全 ✓;
垃圾短语 = 全 ✗。资格判定显式化,每层独立审计。

## 3. 碰撞语义(已落地的不变量)

词与单字**合法同码共存**。碰撞只影响:候选排名、成本、歧义度、选择概率;
绝不影响词汇存在性。高频词不得因生僻字占用同码而消失。

回归守卫:`crates/xhup-generator/src/merged_ranking.rs`(碰撞码跨表合并
排名)+ 哨兵(什么 ufme / 但是 djui 等)+ 常用词覆盖门禁(§7)。

## 4. 候选位是资源

码位不是 FREE/OCCUPIED 二值,而是有序候选位序列(rank 1/2/3/…):

- 占用一个已有码的 rank 2 仍然可能值得;
- 建模选择概率、选择成本、候选扰动,均随 rank 变化;
- 两键码带两个好候选不自动无效。

## 5. 简码分层

| 层 | 键数 | 稀缺性 | 策略 |
|---|---|---|---|
| 一级简码 | 1 | 极稀缺,肌肉记忆成本最高 | 独立参数,强 XHUP 先验 |
| 二码 | 2 | 稀缺,高价值 | 候选位建模关键层 |
| 三码 | 3 | 空间大 | 系统化优化主战场 |
| 全码 | 4/6/8 | 词码语义 | 机械推导,非优化对象 |

各层独立参数,不用一套通用规则通吃。

## 6. XHUP 设计先验(XhupStylePrior)

传统/官方小鹤音形是**强设计先验**,不是不可变真理,也不是无关遗留:

```text
XhupStylePrior(word, code, rank) 特征:
  - 与传统简码映射一致
  - 简码长度相似性
  - 音码前缀一致性 / 形码一致性
  - 传统候选位
  - 码可记忆性 / 约定俗成度
  - 迁移成本
```

既有传统映射获得强正先验;替换广为人知的映射代价高。但优化器**可以**
在真实证据显著更优时偏离 —— 目标是「数学优化自然产出类小鹤结果」,
不是手工兼容补丁。

度量报告:1 键/2 键首选/2 键次选/3 键兼容率、传统简码保留率、候选序
相似度、相对传统基线的真实语料成本改善。

## 7. 常用词 P0 门禁(发布阻断)

- top-100 常用词:100% 词汇覆盖;
- top-1000:实际完整覆盖,每条排除必须有显式理由,禁止静默丢弃;
- 语料级哨兵集(代词/疑问词/否定/常用动词/助词/口语句/连接词),
  断言:词存在、编码可生成、未被误过滤、候选可达性合理;
- 哨兵是验收测试,不是手写简码表。

## 8. 语料证据(数据管线另文)

优化器必须从真实句子学习,不能只看孤立词频。产物:unigram/bigram/
trigram、左右上下文、句子覆盖、上下文多样性、短语能产性、域分布。
原始语料不一定入库(许可/体积),但导入脚本、来源元数据、哈希与可再
分发的派生统计必须可复现。

## 9. 丰富性 vs 混乱

要的是大量**有用**简码,不是几千条用户无法预测的三键短语。加认知/
可记忆性约束:码与读音关系、与既有小鹤模式关系、简码家族一致性、
短语形态、语境有用性。纯数学高频不得产出无意义映射。

## 10. 可解释性(发布门禁)

优化器必须能回答「为什么词 X 得到码 Y」:全局频率、会话频率、句子
覆盖、XHUP 先验、节省键数、预期 rank、fanout、期望成本增量、选中
理由 —— 逐项可查。无可解释性的复杂优化器不可维护。

## 11. 与现有 crate 的关系

- `xhup-analyzer`:保留并扩展(CostModel / FrequencyModel / 候选占用 /
  选择成本 / 歧义成本 / 扰动成本 / 参数扫描),是 optimizer v2 的家;
- `xhup-generator`:保持唯一 production 产物生产者;简码层(ZR/FF/二码)
  的输入从「单频率」升级为 LexicalEntry 证据视图;
- canonical `data/`:语义真源;v1.0.0 起冻结 core 静态简码与重要候选位
  作为 1.x 兼容契约,之后的优化默认保持兼容。

## 12. 发布就绪条件(本文档范围内)

- [ ] LexicalEntry 证据模型落地,优化器消费等价证据
- [ ] 碰撞共存不变量 + 常用词门禁全绿
- [ ] 语料统计管线可复现运行
- [ ] 候选位资源建模 + 分层参数
- [ ] XHUP 风格先验 + 兼容率报告
- [ ] 参数扫描产出 Pareto 工作点与选型依据
- [ ] 语料回放基准(KSPC/rank1/rank≤3/句成功率)作为 release gate
- [ ] 可解释性报告
- [ ] 新 canonical mapping 经对照报告评审后冻结
