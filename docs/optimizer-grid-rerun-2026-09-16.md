# Optimizer v2 全网格复跑记录(2026-09-16,§25 第 4 步)

## 背景

Production 选型(见 docs/optimizer-v2.md「Production 选型记录」)早于
MultiSourceFrequencyEvidence daily-prior(#98)与 Production Shortcut
Audit(#99)落地。#83 §25 第 4 步要求:production optimizer 全网格在
**真实生产数据**上对照复跑,确认现选点在多源证据下的位置。

## 复跑条件(完全真实生产数据,§24 增补 C)

- 词表:canonical v2 全量(evidence by sweep_v2::v2_targets);
- 证据:LexicalEvidenceSet(含 #98 daily_prior 全量注入);
- 回放:KdConv 聚合统计近似(无 --reference;兼容率列为 NA,合规
  docs/data-pipeline.md §0 不入库原则);
- 网格:2 rank profile × 3 ambiguity × 3 disruption × 3 xhup ×
  2 evidence variant = **108 点 + canonical 基线行**;
- 复现命令:

  ```bash
  cargo run --locked -p xhup-analyzer --bin sweep-v2 -- \
    --output <本地路径>
  ```

  (同输入字节级确定;TSV 不入库 —— 研究产物规范,与 --reference
  对照一样由维护者本地生成。)

## 结果摘要

| 行 | kspc | rank1_rate | top3_rate |
|---|---|---|---|
| canonical 基线(含 #98 daily_prior) | 1.900488 | 94.7502% | 99.8264% |
| 现选点 `rk-steep\|a0.25\|d0.5\|x1\|e-conversation` | 1.903393 | 95.4946% | 99.9017% |
| 网格 rank1 最优 `rk-steep\|a0.25\|d1\|x2\|e-conversation` | 1.910648 | 95.7463% | 99.9186% |

## 结论

1. **现选点在多源证据下依然合理**:rank1 95.49% 高于基线 +0.74pp,
   kspc 仅 +0.003(§1 的 ExpectedHumanInputCost 多目标权衡);
2. 网格内存在 rank1 更高的工作点(+0.25pp),代价 kspc +0.007 ——
   位于「disruption 翻倍 + xhup 偏离翻倍」象限,偏离传统映射的
   认知代价(xhup_deviation)在成本模型中正是为抑制此类跳跃而设;
   是否迁移属**映射冻结**约束下的维护者决策(§7 不变量),代理不自行改选;
3. **oracle 交叉验证**(#100):单码实例上贪心 vs 精确 gap 0.0024%
   —— 网格内任何点切换运行参数不涉及接纳顺序损失,选型差异来自
   成本模型权重本身;
4. Shortcut Audit 门禁(#99)与 oracle(#100)在 CI 每次运行,
   任何映射变更都会同时通过 §3 合同与 gap 报告验证。

## 与 §25 第 4 步完成定义的关系

- [x] 全网格真实数据复跑 + 选点位置确认;
- [x] oracle 交叉验证接入(#100);
- [x] 多目标权衡量化(rank1 vs kspc vs xhup_deviation);
- [ ] 迁移工作点(如维护者决定):需 --reference 对照报告 + 语料回放,
      按 §7 冻结前变更流程执行。
