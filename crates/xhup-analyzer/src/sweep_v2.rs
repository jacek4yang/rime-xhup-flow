//! v2 参数扫描与运行点指标(docs/optimizer-v2.md §5)。
//!
//! 对 CostModelV2 × EvidenceWeights 的可配置网格(编译期常量,代表性
//! 采样优先于大而全)逐运行点:产出 v2 映射(mapping_v2),并计算五类
//! 指标:
//!
//! - **期望输入成本**:语料回放(replay.rs)KSPC / rank1 / top3 / 期望成本。
//!   原始句子语料不入库(data/corpus/README.md),因此默认用**聚合统计
//!   近似回放**(每个词形视作独立一句按计数加权,无跨词分词交互,报告
//!   标注为近似);提供真实句子语料时以其为准;
//! - **XHUP 兼容率**:提供参考映射 TSV 时按 1/2/3 键分层保持率(compat);
//!   未提供时指标显式缺失(TSV 中 `NA`),绝不静默填 0;
//! - **top-100/top-1000 可达 rank 分布**(频率口径:万象聚合分数降序,
//!   与 evidence/frequency 一致);
//! - **候选 fanout 统计**(v2 词在码位上的分布,含共享码数);
//! - **稳定性**:相邻运行点(网格序)间映射变动率。
//!
//! 输出:机器可读 TSV([`render_tsv`])+ 人类可读摘要([`render_summary`]),
//! 两者同输入字节确定。本模块不声称任何"最优参数",只呈现运行点面,
//! 选型是 Pareto 前沿上的人工评审(文档 §5)。

use std::collections::{BTreeMap, BTreeSet};

use crate::candidates::WordTarget;
use crate::compat::{self, ReferenceEntry, TierCompat};
use crate::corpus::CorpusStats;
use crate::evidence::LexicalEvidence;
use crate::mapping_v2::{BaselineMassView, FanoutStats, MappingV2, produce_mapping};
use crate::optimizer_v2::{CostModelV2, EvidenceWeights};
use crate::replay::{ReplayCostModel, ReplayMapping, Replayer};
use crate::xhup_prior::XhupStylePrior;

/// 一个 v2 扫描运行点(成本参数 + 证据权重组合)。
#[derive(Clone, Debug)]
pub struct SweepV2Point {
    /// 报告用稳定标签(由参数组合派生,确定性)。
    pub label: String,
    /// 成本模型参数。
    pub cost: CostModelV2,
    /// 证据权重参数。
    pub weights: EvidenceWeights,
}

/// 编译期扫描网格:2 rank 曲线 × 3 歧义 × 3 扰动 × 3 XHUP 偏离
/// × 2 证据权重 = 108 个运行点。
///
/// key_cost 固定 1.0(成本尺度归一);认知/长尾系数固定在默认代表点
/// (扫描维度优先给文档 §5 点名的选择/扰动/先验/证据四类)。
pub fn grid() -> Vec<SweepV2Point> {
    const RANK_PROFILES: [(&str, [f64; 4]); 2] = [
        ("rk-std", [0.0, 0.5, 1.0, 2.0]),
        ("rk-steep", [0.0, 1.0, 2.0, 4.0]),
    ];
    const AMBIGUITY: [f64; 3] = [0.25, 0.5, 1.0];
    const DISRUPTION: [f64; 3] = [0.5, 1.0, 2.0];
    const XHUP_DEVIATION: [f64; 3] = [0.0, 1.0, 2.0];
    let evidence_variants = [
        ("e-default", EvidenceWeights::default()),
        (
            "e-conversation",
            EvidenceWeights {
                global_share: 0.3,
                conversation_share: 0.4,
                sentence_coverage_weight: 0.2,
                context_diversity_weight: 0.1,
            },
        ),
    ];

    let mut points = Vec::new();
    for (rank_label, rank_cost) in RANK_PROFILES {
        for ambiguity_coeff in AMBIGUITY {
            for disruption_coeff in DISRUPTION {
                for xhup_deviation_coeff in XHUP_DEVIATION {
                    for (ev_label, weights) in evidence_variants {
                        points.push(SweepV2Point {
                            label: format!(
                                "{rank_label}|a{ambiguity_coeff}|d{disruption_coeff}|x{xhup_deviation_coeff}|{ev_label}"
                            ),
                            cost: CostModelV2 {
                                rank_cost,
                                ambiguity_coeff,
                                disruption_coeff,
                                xhup_deviation_coeff,
                                ..CostModelV2::default()
                            },
                            weights,
                        });
                    }
                }
            }
        }
    }
    points
}

/// 回放语料来源。
pub enum ReplaySource<'a> {
    /// 聚合统计近似(无句子上下文;原始语料不入库时的默认路径)。
    AggregateStats(&'a CorpusStats),
    /// 真实句子语料(每行一句)。
    Sentences(&'a [String]),
}

/// 一次扫描的全部输入(构建一次,各运行点复用)。
pub struct SweepV2Input<'a> {
    /// 候选词集合(candidates.rs 管线)。
    pub targets: &'a [WordTarget],
    /// 词 → 多信号词汇证据。
    pub evidence: &'a BTreeMap<String, LexicalEvidence>,
    /// 固定层码位占用质量视图。
    pub baseline: &'a BaselineMassView,
    /// 回放语料来源。
    pub replay: ReplaySource<'a>,
    /// 参考映射(本地提供,绝不入库);`None` 时兼容率指标显式缺失、
    /// XHUP 先验全部中立。
    pub reference: Option<&'a [ReferenceEntry]>,
}

/// 回放指标(聚合统计近似回放时语义为近似,见模块文档)。
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ReplayMetrics {
    /// 键/字。
    pub kspc: f64,
    /// rank-1 命中率(token 级)。
    pub rank1_rate: f64,
    /// rank ≤ 3 命中率(token 级)。
    pub top3_rate: f64,
    /// 每字期望成本(键 + 选择)。
    pub expected_cost_per_char: f64,
}

/// 可达 rank 分布(top-N 词窗)。
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RankDistribution {
    /// rank 1 词数。
    pub rank1: usize,
    /// rank 2 词数。
    pub rank2: usize,
    /// rank 3 词数。
    pub rank3: usize,
    /// rank ≥ 4 词数。
    pub rank4_plus: usize,
    /// 未分配 shortcut(留全码)词数。
    pub unassigned: usize,
}

/// 一个运行点的完整指标行。
pub struct SweepV2Row {
    /// 运行点。
    pub point: SweepV2Point,
    /// 分配词数。
    pub assigned: usize,
    /// 回放指标。
    pub replay: ReplayMetrics,
    /// 兼容率分层(码长 → 桶;1/2/3 键 + 全码桶);`None` = 未提供参考映射。
    pub compat: Option<BTreeMap<usize, TierCompat>>,
    /// top-100 可达 rank 分布。
    pub top100: RankDistribution,
    /// top-1000 可达 rank 分布。
    pub top1000: RankDistribution,
    /// 候选 fanout 统计。
    pub fanout: FanoutStats,
    /// 与前一运行点(网格序)的映射变动率;首个点为 `None`。
    pub change_rate: Option<f64>,
}

/// 证据视图索引:词 → 证据(构建一次,扫描复用)。
pub fn evidence_by_word(
    set: &crate::evidence::LexicalEvidenceSet,
) -> BTreeMap<String, LexicalEvidence> {
    set.entries()
        .iter()
        .map(|e| (e.word().to_string(), e.clone()))
        .collect()
}

/// 词频排序口径:万象聚合分数降序,词形升序兜底(与 evidence/frequency
/// 现有口径一致;确定性)。
fn frequency_order(targets: &[WordTarget]) -> Vec<String> {
    let mut ordered: Vec<(u64, &str)> = targets
        .iter()
        .map(|t| (t.frequency_score(), t.word()))
        .collect();
    ordered.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(b.1)));
    ordered.into_iter().map(|(_, w)| w.to_string()).collect()
}

/// top-N 可达 rank 分布。
fn rank_distribution(order: &[String], mapping: &MappingV2, n: usize) -> RankDistribution {
    let mut dist = RankDistribution::default();
    for word in order.iter().take(n) {
        match mapping.get(word) {
            None => dist.unassigned += 1,
            Some(entry) => match entry.rank {
                1 => dist.rank1 += 1,
                2 => dist.rank2 += 1,
                3 => dist.rank3 += 1,
                _ => dist.rank4_plus += 1,
            },
        }
    }
    dist
}

/// 相邻运行点映射变动率:分配状态或 (码, rank) 不同的词数 /
/// 两次运行分配词数并集。
fn mapping_change_rate(prev: &MappingV2, current: &MappingV2) -> f64 {
    let words: BTreeSet<&str> = prev
        .entries()
        .map(|e| e.word.as_str())
        .chain(current.entries().map(|e| e.word.as_str()))
        .collect();
    if words.is_empty() {
        return 0.0;
    }
    let changed = words
        .iter()
        .filter(|word| match (prev.get(word), current.get(word)) {
            (None, None) => false, // 并集成员不会双缺
            (Some(a), Some(b)) => a.code != b.code || a.rank != b.rank,
            _ => true,
        })
        .count();
    changed as f64 / words.len() as f64
}

/// 执行扫描:逐运行点产出映射并计算全部指标(网格序即输出序)。
pub fn run_sweep_v2(input: &SweepV2Input, points: &[SweepV2Point]) -> Vec<SweepV2Row> {
    let prior = input
        .reference
        .map(|r| XhupStylePrior::from_entries(r.to_vec()));
    // 兼容率 baseline 索引与参数无关,构建一次(仅在有参考映射时)。
    let compat_index = input.reference.map(|_| compat::build_baseline_index());
    let order = frequency_order(input.targets);

    let mut rows = Vec::with_capacity(points.len());
    let mut previous: Option<MappingV2> = None;
    for point in points {
        let mapping = produce_mapping(
            input.targets,
            input.evidence,
            input.baseline,
            &point.cost,
            &point.weights,
            prior.as_ref(),
        );

        // (a) 期望输入成本:回放(运行点自己的键/选择成本假设)。
        let replay_cost = ReplayCostModel {
            key_cost: point.cost.key_cost,
            rank_cost: point.cost.rank_cost,
        };
        let replayer = Replayer::with_mapping(ReplayMapping::build_with_plans(
            &replay_cost,
            &mapping.replay_plans(),
        ));
        let report = match input.replay {
            ReplaySource::AggregateStats(stats) => replayer
                .replay_weighted_words(stats.words.iter().map(|(w, s)| (w.as_str(), s.count))),
            ReplaySource::Sentences(sentences) => {
                replayer.replay_corpus(sentences.iter().map(String::as_str))
            }
        };
        let replay = ReplayMetrics {
            kspc: report.kspc(),
            rank1_rate: report.rank1_rate(),
            top3_rate: report.top3_rate(),
            expected_cost_per_char: report.totals.expected_cost
                / (report.totals.chars.max(1)) as f64,
        };

        // (b) XHUP 兼容率:baseline 层 + v2 映射叠加(不含已入库简码层)。
        let compat = input.reference.map(|reference| {
            let mut index = compat_index
                .as_ref()
                .expect("有参考映射时索引必然已构建")
                .clone();
            for entry in mapping.entries() {
                index.entry(entry.code.to_string()).or_default().push((
                    entry.word.clone(),
                    compat::CurrentHit {
                        layer: "v2-sweep",
                        rank: entry.rank,
                    },
                ));
            }
            compat::compare_with_index(reference, &index).tiers
        });

        // (e) 稳定性:与前一运行点的映射变动率。
        let change_rate = previous
            .as_ref()
            .map(|prev| mapping_change_rate(prev, &mapping));

        rows.push(SweepV2Row {
            fanout: mapping.fanout_stats(),
            assigned: mapping.len(),
            top100: rank_distribution(&order, &mapping, 100),
            top1000: rank_distribution(&order, &mapping, 1000),
            point: point.clone(),
            replay,
            compat,
            change_rate,
        });
        previous = Some(mapping);
    }
    rows
}

/// TSV 列(表头与数据行共用此顺序)。
const TSV_COLUMNS: [&str; 41] = [
    "point",
    "key_cost",
    "rank1_cost",
    "rank2_cost",
    "rank3_cost",
    "rank4_cost",
    "ambiguity",
    "disruption",
    "xhup_deviation",
    "cognitive",
    "rare_pollution",
    "w_global",
    "w_conversation",
    "w_coverage",
    "w_diversity",
    "assigned",
    "kspc",
    "rank1_rate",
    "top3_rate",
    "cost_per_char",
    "compat1_preserved",
    "compat1_first",
    "compat2_preserved",
    "compat2_first",
    "compat3_preserved",
    "compat3_first",
    "top100_r1",
    "top100_r2",
    "top100_r3",
    "top100_r4plus",
    "top100_unassigned",
    "top1000_r1",
    "top1000_r2",
    "top1000_r3",
    "top1000_r4plus",
    "top1000_unassigned",
    "codes_used",
    "shared_codes",
    "fanout_mean",
    "fanout_max",
    "change_rate",
];

/// 机器可读 TSV(每运行点一行;缺失指标显式 `NA`)。
pub fn render_tsv(rows: &[SweepV2Row]) -> String {
    let mut out = TSV_COLUMNS.join("\t") + "\n";
    for row in rows {
        let c = &row.point.cost;
        let w = &row.point.weights;
        let compat = |tier: usize, f: fn(TierCompat) -> f64| {
            row.compat
                .as_ref()
                .and_then(|tiers| tiers.get(&tier))
                .map_or("NA".to_string(), |t| format!("{:.6}", f(*t)))
        };
        let dist = |d: &RankDistribution| {
            [
                d.rank1.to_string(),
                d.rank2.to_string(),
                d.rank3.to_string(),
                d.rank4_plus.to_string(),
                d.unassigned.to_string(),
            ]
        };
        let fields: Vec<String> = [
            vec![
                row.point.label.clone(),
                c.key_cost.to_string(),
                c.rank_cost[0].to_string(),
                c.rank_cost[1].to_string(),
                c.rank_cost[2].to_string(),
                c.rank_cost[3].to_string(),
                c.ambiguity_coeff.to_string(),
                c.disruption_coeff.to_string(),
                c.xhup_deviation_coeff.to_string(),
                c.cognitive_complexity_coeff.to_string(),
                c.rare_pollution_coeff.to_string(),
                w.global_share.to_string(),
                w.conversation_share.to_string(),
                w.sentence_coverage_weight.to_string(),
                w.context_diversity_weight.to_string(),
                row.assigned.to_string(),
                format!("{:.6}", row.replay.kspc),
                format!("{:.6}", row.replay.rank1_rate),
                format!("{:.6}", row.replay.top3_rate),
                format!("{:.6}", row.replay.expected_cost_per_char),
                compat(1, |t| t.preservation_rate()),
                compat(1, |t| t.first_rate()),
                compat(2, |t| t.preservation_rate()),
                compat(2, |t| t.first_rate()),
                compat(3, |t| t.preservation_rate()),
                compat(3, |t| t.first_rate()),
            ],
            dist(&row.top100).to_vec(),
            dist(&row.top1000).to_vec(),
            vec![
                row.fanout.codes_used.to_string(),
                row.fanout.shared_codes.to_string(),
                format!("{:.6}", row.fanout.mean),
                row.fanout.max.to_string(),
                row.change_rate
                    .map_or("NA".to_string(), |r| format!("{r:.6}")),
            ],
        ]
        .concat();
        debug_assert_eq!(fields.len(), TSV_COLUMNS.len(), "TSV 列数应与表头一致");
        out.push_str(&fields.join("\t"));
        out.push('\n');
    }
    out
}

/// 人类可读摘要(每运行点一行,指标列齐)。
pub fn render_summary(rows: &[SweepV2Row]) -> String {
    let mut out = String::from(
        "运行点 | 分配 | KSPC | rank1 | top3 | 兼容2键 | top100首选 | fanout均值 | 变动率\n",
    );
    for row in rows {
        out.push_str(&format!(
            "{} | {} | {:.3} | {:.1}% | {:.1}% | {} | {}/100 | {:.2} | {}\n",
            row.point.label,
            row.assigned,
            row.replay.kspc,
            row.replay.rank1_rate * 100.0,
            row.replay.top3_rate * 100.0,
            row.compat
                .as_ref()
                .and_then(|tiers| tiers.get(&2))
                .map_or("NA".to_string(), |t| {
                    format!("{:.1}%", t.preservation_rate() * 100.0)
                }),
            row.top100.rank1,
            row.fanout.mean,
            row.change_rate
                .map_or("NA".to_string(), |r| format!("{:.1}%", r * 100.0)),
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    //! 端到端测试用合成小词表(canonical 骨干词 + 合成候选码,回放分词
    //! 可命中)与合成语料/参考映射;参考数据全部合成,不触碰真实官方映射。
    use super::*;
    use crate::candidates::ShortcutCandidate;

    fn word_evidence(word: &str, code: &str, normalized: f64) -> LexicalEvidence {
        LexicalEvidence::for_test(
            word,
            code.parse().unwrap(),
            1000,
            normalized,
            None,
            None,
            None,
        )
    }

    fn target(word: &str, full: &str, score: u64, candidates: &[&str]) -> WordTarget {
        WordTarget::with_candidates_for_test(
            word,
            full.parse().unwrap(),
            score,
            candidates
                .iter()
                .map(|c| ShortcutCandidate::for_test(c.parse().unwrap()))
                .collect(),
        )
    }

    /// 合成输入:三个 canonical 词各带一个 2 键候选;空 baseline。
    fn fixture() -> (
        Vec<WordTarget>,
        BTreeMap<String, LexicalEvidence>,
        BaselineMassView,
    ) {
        let targets = vec![
            target("我们", "womf", 1000, &["wm"]),
            target("时间", "uijm", 900, &["uj"]),
            target("什么", "ufme", 800, &["uf"]),
        ];
        let evidence = [
            word_evidence("我们", "womf", 1e-4),
            word_evidence("时间", "uijm", 8e-5),
            word_evidence("什么", "ufme", 6e-5),
        ]
        .into_iter()
        .map(|e| (e.word().to_string(), e))
        .collect();
        (targets, evidence, BaselineMassView::for_test(Vec::new()))
    }

    fn two_points() -> Vec<SweepV2Point> {
        vec![
            SweepV2Point {
                label: "test-a".to_string(),
                cost: CostModelV2::default(),
                weights: EvidenceWeights::default(),
            },
            SweepV2Point {
                label: "test-b".to_string(),
                cost: CostModelV2 {
                    ambiguity_coeff: 1.0,
                    ..CostModelV2::default()
                },
                weights: EvidenceWeights::default(),
            },
        ]
    }

    #[test]
    fn grid_size_and_labels_are_stable() {
        let points = grid();
        assert_eq!(points.len(), 108, "网格应为 108 个运行点");
        let labels: BTreeSet<&str> = points.iter().map(|p| p.label.as_str()).collect();
        assert_eq!(labels.len(), points.len(), "标签应唯一");
        assert_eq!(points[0].label, "rk-std|a0.25|d0.5|x0|e-default");
    }

    #[test]
    fn end_to_end_synthetic_produces_all_metrics() {
        let (targets, evidence, baseline) = fixture();
        let sentences = vec!["我们时间".to_string(), "什么时间".to_string()];
        let input = SweepV2Input {
            targets: &targets,
            evidence: &evidence,
            baseline: &baseline,
            replay: ReplaySource::Sentences(&sentences),
            reference: None,
        };
        let rows = run_sweep_v2(&input, &two_points());
        assert_eq!(rows.len(), 2);
        for row in &rows {
            assert!(row.assigned >= 1, "高频词应被分配");
            assert!(row.replay.kspc > 0.0 && row.replay.kspc <= 4.0);
            assert!(
                (0.0..=1.0).contains(&row.replay.rank1_rate),
                "rank1 率应 ∈ [0,1],实际 {}",
                row.replay.rank1_rate
            );
            assert!((0.0..=1.0).contains(&row.replay.top3_rate));
            assert!(row.compat.is_none(), "无参考映射时兼容率显式缺失");
            let top = row.top100;
            assert_eq!(
                top.rank1 + top.rank2 + top.rank3 + top.rank4_plus + top.unassigned,
                3,
                "top-100 分布应覆盖全部合成词"
            );
            assert!(row.fanout.codes_used >= 1);
        }
        assert!(rows[0].change_rate.is_none(), "首个运行点无变动率");
        let rate = rows[1].change_rate.expect("第二运行点应有变动率");
        assert!((0.0..=1.0).contains(&rate));
        // 分配词都拿到 2 键首选 → 合成句回放 KSPC 应优于全码(2.0)。
        assert!(rows[0].replay.kspc <= 2.0, "KSPC 应不劣于全码");
    }

    #[test]
    fn sweep_is_byte_deterministic() {
        let (targets, evidence, baseline) = fixture();
        let sentences = vec!["我们时间".to_string()];
        let input = SweepV2Input {
            targets: &targets,
            evidence: &evidence,
            baseline: &baseline,
            replay: ReplaySource::Sentences(&sentences),
            reference: None,
        };
        let first = render_tsv(&run_sweep_v2(&input, &two_points()));
        let second = render_tsv(&run_sweep_v2(&input, &two_points()));
        assert_eq!(first, second, "同输入同参数 → TSV 字节一致");
    }

    #[test]
    fn missing_reference_marks_compat_na() {
        let (targets, evidence, baseline) = fixture();
        let sentences = vec!["我们".to_string()];
        let input = SweepV2Input {
            targets: &targets,
            evidence: &evidence,
            baseline: &baseline,
            replay: ReplaySource::Sentences(&sentences),
            reference: None,
        };
        let tsv = render_tsv(&run_sweep_v2(&input, &two_points()));
        let data_line = tsv.lines().nth(1).expect("应有数据行");
        let fields: Vec<&str> = data_line.split('\t').collect();
        assert_eq!(fields.len(), TSV_COLUMNS.len(), "数据行列数应与表头一致");
        let compat_col = TSV_COLUMNS
            .iter()
            .position(|c| *c == "compat1_preserved")
            .unwrap();
        assert_eq!(fields[compat_col], "NA", "缺失参考映射应显式 NA");
    }

    #[test]
    fn aggregate_stats_replay_path_produces_metrics() {
        // 默认回放路径(无 --input):聚合统计近似,逐词形按计数加权。
        let (targets, evidence, baseline) = fixture();
        let stats = CorpusStats {
            words: [(
                "我们".to_string(),
                crate::corpus::WordCorpusStats {
                    count: 5,
                    sentence_count: 5,
                    left_contexts: 1,
                    right_contexts: 1,
                },
            )]
            .into_iter()
            .collect(),
            sentences: 5,
            tokens: 5,
        };
        let input = SweepV2Input {
            targets: &targets,
            evidence: &evidence,
            baseline: &baseline,
            replay: ReplaySource::AggregateStats(&stats),
            reference: None,
        };
        let rows = run_sweep_v2(&input, &two_points());
        assert_eq!(rows.len(), 2);
        assert!(rows[0].replay.kspc > 0.0);
        assert!((0.0..=1.0).contains(&rows[0].replay.rank1_rate));
    }

    #[test]
    fn synthetic_reference_yields_compat_rates() {
        let (targets, evidence, baseline) = fixture();
        let sentences = vec!["我们".to_string()];
        // 合成参考(非真实数据):我们 → wm 首选,与产出映射一致。
        let reference = compat::parse_reference_tsv("# 合成参考\n我们\twm\t1\n什么\tuf\t2\n")
            .expect("合成参考可解析");
        let input = SweepV2Input {
            targets: &targets,
            evidence: &evidence,
            baseline: &baseline,
            replay: ReplaySource::Sentences(&sentences),
            reference: Some(&reference),
        };
        let rows = run_sweep_v2(&input, &two_points());
        for row in &rows {
            let tiers = row.compat.as_ref().expect("有参考映射时兼容率应存在");
            let tier2 = tiers.get(&2).expect("应有 2 键桶");
            assert_eq!(tier2.total, 2);
            for rate in [tier2.preservation_rate(), tier2.first_rate()] {
                assert!((0.0..=1.0).contains(&rate), "兼容率应 ∈ [0,1]");
            }
            assert_eq!(
                tier2.preservation_rate(),
                1.0,
                "合成参考与 v2 分配一致,应全部保留"
            );
        }
    }
}
