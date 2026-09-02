//! 报告渲染:text 报告与 TSV 转储。
//!
//! 报告如实呈现计算结果:不预设任何候选更优,不声称"最优参数",成本数值是
//! 无量纲优化目标而非真实耗时。`ZERO_REGRESSION` 的准确含义是
//! **zero exact-code regression**(shortcut 码在当前固定 exact 空间完全空闲,
//! 不重排任何已有候选组);真正接入 production 仍需 librime runtime acceptance。

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::time::Duration;

use crate::candidates::WordTarget;
use crate::frequency::{CharCodeUsage, FrequencyScale};
use crate::occupancy::CandidateSource;
use crate::optimize::{OptimizationOutcome, OptimizationProfile, evaluate_target};
use crate::sweep::{Robustness, SweepRun, classify, operating_points, robustness_map};
use crate::{AnalysisData, CostModel};
use xhup_core::KeySequence;

/// 各阶段耗时(CLI 计时后传入)。
#[derive(Clone, Copy, Debug, Default)]
pub struct Timings {
    /// 加载 generator 证据投影。
    pub load_evidence: Duration,
    /// 构建码位占用。
    pub build_occupancy: Duration,
    /// 候选枚举。
    pub enumeration: Duration,
    /// 单次优化运行(balanced)。
    pub single_run: Duration,
    /// 完整 sensitivity sweep。
    pub sweep: Duration,
}

/// balanced 频率尺度(50:50 混合、3 码归属保守假设)。
pub fn balanced_scale() -> FrequencyScale {
    FrequencyScale::Normalized {
        char_share: 0.5,
        usage: CharCodeUsage::Conservative,
    }
}

/// balanced operating point 的成本模型。
pub fn balanced_cost() -> CostModel {
    operating_points()[2].cost_model()
}

/// 在 sweep 结果中定位某 profile 的 balanced 主运行(50:50 / conservative)。
fn find_balanced_run(runs: &[SweepRun], profile: OptimizationProfile) -> Option<&SweepRun> {
    runs.iter().find(|run| {
        run.profile == profile
            && run.point == "balanced"
            && !run.diagnostic
            && matches!(
                run.scale,
                FrequencyScale::Normalized {
                    char_share,
                    usage: CharCodeUsage::Conservative,
                } if char_share == 0.5
            )
    })
}

/// u64 分数分布摘要(count/min/p50/p90/p99/max/sum)。
fn score_distribution(scores: &mut [u64]) -> (usize, u64, u64, u64, u64, u64, u64) {
    scores.sort_unstable();
    let n = scores.len();
    if n == 0 {
        return (0, 0, 0, 0, 0, 0, 0);
    }
    let percentile = |p: usize| scores[(p * n).div_ceil(100).max(1) - 1];
    (
        n,
        scores[0],
        percentile(50),
        percentile(90),
        percentile(99),
        scores[n - 1],
        scores.iter().sum(),
    )
}

fn fmt_seconds(d: Duration) -> String {
    format!("{:.3}s", d.as_secs_f64())
}

/// 渲染完整 text 报告。
pub fn render_report(data: &AnalysisData, runs: &[SweepRun], timings: &Timings) -> String {
    let mut out = String::new();
    section_overview(&mut out, data, timings);
    section_occupancy(&mut out, data);
    section_frequency_audit(&mut out, data);
    section_topics(&mut out, data);
    for profile in OptimizationProfile::all() {
        section_profile(&mut out, data, runs, profile);
    }
    section_operating_points(&mut out, runs);
    section_robustness(&mut out, runs);
    section_sentinels(&mut out, data, runs);
    section_raw_diagnostic(&mut out, runs);
    out
}

fn section_overview(out: &mut String, data: &AnalysisData, timings: &Timings) {
    let mut universe: BTreeMap<usize, usize> = BTreeMap::new();
    for target in &data.targets {
        *universe.entry(target.word().chars().count()).or_default() += 1;
    }
    writeln!(out, "# XHUP Flow 词语简码优化分析报告").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "## 概览").unwrap();
    writeln!(out, "word universe(production 100k words):").unwrap();
    for (chars, count) in &universe {
        writeln!(out, "  {chars}-char: {count}").unwrap();
    }
    writeln!(out, "  total targets: {}", data.targets.len()).unwrap();
    writeln!(out).unwrap();
    writeln!(out, "shortcut candidates:").unwrap();
    writeln!(
        out,
        "  theoretical(pre-dedup): {}",
        data.enumeration.theoretical
    )
    .unwrap();
    writeln!(out, "  actual(post-dedup):     {}", data.enumeration.actual).unwrap();
    writeln!(
        out,
        "  dedup removed:          {}",
        data.enumeration.dedup_removed()
    )
    .unwrap();
    writeln!(out, "  by length:").unwrap();
    for length in 3..=7 {
        writeln!(
            out,
            "    {length}: {}",
            data.enumeration
                .by_length
                .get(&length)
                .copied()
                .unwrap_or(0)
        )
        .unwrap();
    }
    writeln!(out).unwrap();
    writeln!(out, "timings:").unwrap();
    writeln!(
        out,
        "  load evidence:    {}",
        fmt_seconds(timings.load_evidence)
    )
    .unwrap();
    writeln!(
        out,
        "  build occupancy:  {}",
        fmt_seconds(timings.build_occupancy)
    )
    .unwrap();
    writeln!(
        out,
        "  enumeration:      {}",
        fmt_seconds(timings.enumeration)
    )
    .unwrap();
    writeln!(
        out,
        "  single run:       {}",
        fmt_seconds(timings.single_run)
    )
    .unwrap();
    writeln!(out, "  sweep:            {}", fmt_seconds(timings.sweep)).unwrap();
    writeln!(out).unwrap();
}

fn section_occupancy(out: &mut String, data: &AnalysisData) {
    let audit = data.occupancy.layer_audit();
    writeln!(out, "## 当前 code-space 占用(从 canonical data 现算)").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "分层行数:").unwrap();
    writeln!(
        out,
        "  1-key level1 shortcuts: {}",
        audit.level1_shortcut_rows
    )
    .unwrap();
    writeln!(out, "  2-key chars:            {}", audit.char_2key_rows).unwrap();
    writeln!(out, "  3-key chars:            {}", audit.char_3key_rows).unwrap();
    writeln!(out, "  4-key chars(FullCode): {}", audit.char_4key_rows).unwrap();
    writeln!(out, "  4-key words:            {}", audit.word_4key_rows).unwrap();
    writeln!(out, "  6-key words:            {}", audit.word_6key_rows).unwrap();
    writeln!(out, "  8-key words:            {}", audit.word_8key_rows).unwrap();
    writeln!(out, "  total rows:             {}", audit.total_rows()).unwrap();
    writeln!(out).unwrap();
    writeln!(
        out,
        "| len | distinct codes | rows | mean | median | P90 | P95 | P99 | max |"
    )
    .unwrap();
    for stats in data.occupancy.length_stats() {
        writeln!(
            out,
            "| {} | {} | {} | {:.2} | {} | {} | {} | {} | {} |",
            stats.length(),
            stats.distinct_codes(),
            stats.rows(),
            stats.mean_fanout(),
            stats.median_fanout(),
            stats.p90_fanout(),
            stats.p95_fanout(),
            stats.p99_fanout(),
            stats.max_fanout(),
        )
        .unwrap();
    }
    writeln!(out).unwrap();
    let stats = data.occupancy.length_stats();
    let five = &stats[4];
    let seven = &stats[6];
    if five.rows() == 0 && seven.rows() == 0 {
        writeln!(
            out,
            "5-key 与 7-key 当前为整体空闲的 exact-code 空间(0 行)。"
        )
        .unwrap();
    } else {
        writeln!(
            out,
            "注意:5-key 现有 {} 行,7-key 现有 {} 行(非空闲)。",
            five.rows(),
            seven.rows()
        )
        .unwrap();
    }
    writeln!(out).unwrap();
}

fn section_frequency_audit(out: &mut String, data: &AnalysisData) {
    let mut word_scores: Vec<u64> = data.words.iter().map(|e| e.frequency_score()).collect();
    let mut char_scores: Vec<u64> = data.chars.iter().map(|e| e.frequency_score()).collect();
    let words = score_distribution(&mut word_scores);
    let chars = score_distribution(&mut char_scores);
    writeln!(out, "## 频率尺度审计").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "| domain | n | min | p50 | p90 | p99 | max | sum |").unwrap();
    writeln!(
        out,
        "| word | {} | {} | {} | {} | {} | {} | {} |",
        words.0, words.1, words.2, words.3, words.4, words.5, words.6
    )
    .unwrap();
    writeln!(
        out,
        "| char | {} | {} | {} | {} | {} | {} | {} |",
        chars.0, chars.1, chars.2, chars.3, chars.4, chars.5, chars.6
    )
    .unwrap();
    writeln!(out).unwrap();
    writeln!(
        out,
        "词分数与字分数不保证处于相同绝对尺度。主模型在两个 domain 内分别归一化\
         (P_word / P_char),经 char:word 混合比(25:75 / 50:50 / 75:25)合并;\
         3 码单字多码归属提供 conservative / split 两种假设。raw-score 模型仅作\
         诊断(见文末),不作为推荐依据。"
    )
    .unwrap();
    writeln!(out).unwrap();

    // ── 单字 domain 归一化审计 ──────────────────────────────────
    // Normalized 单字 domain 只含 3 码单字关系(OPTIMIZED 中唯一可重排的单字
    // 层;4 码规范全码硬保护、1/2 键 shortcut 不可达,不参与归一化)。
    let three_key: Vec<&crate::CharCodeAnalysisEntry> =
        data.chars.iter().filter(|e| e.code().len() == 3).collect();
    writeln!(
        out,
        "单字 domain 归一化审计(normalized 模型仅以 3 码单字关系为 domain):"
    )
    .unwrap();
    writeln!(out, "  3-key char relation count:  {}", three_key.len()).unwrap();
    writeln!(
        out,
        "  3-key conservative raw total: {}",
        data.frequency.char_total_conservative()
    )
    .unwrap();
    writeln!(
        out,
        "  3-key split raw total:        {:.1}",
        data.frequency.char_total_split()
    )
    .unwrap();
    for usage in [CharCodeUsage::Conservative, CharCodeUsage::Split] {
        let sum = char_domain_mass(
            data,
            &FrequencyScale::Normalized {
                char_share: 1.0,
                usage,
            },
        );
        writeln!(out, "  normalized sum({}): {:.12}", usage.label(), sum).unwrap();
    }
    writeln!(out).unwrap();
    writeln!(
        out,
        "| char:word | conservative char mass | split char mass |"
    )
    .unwrap();
    for char_share in [0.25, 0.50, 0.75] {
        let conservative = char_domain_mass(
            data,
            &FrequencyScale::Normalized {
                char_share,
                usage: CharCodeUsage::Conservative,
            },
        );
        let split = char_domain_mass(
            data,
            &FrequencyScale::Normalized {
                char_share,
                usage: CharCodeUsage::Split,
            },
        );
        writeln!(
            out,
            "| {:.0}:{:.0} | {conservative:.6} | {split:.6} |",
            char_share * 100.0,
            (1.0 - char_share) * 100.0
        )
        .unwrap();
    }
    writeln!(out).unwrap();
    writeln!(
        out,
        "混合后 char 侧总质量恒等于设定的 char_share(上表),domain 不被 2/4 码关系稀释。"
    )
    .unwrap();
    writeln!(out).unwrap();
}

/// 全部 3 码单字关系在指定尺度下的归一化权重合计(频率审计用)。
fn char_domain_mass(data: &AnalysisData, scale: &FrequencyScale) -> f64 {
    let mut buf = [0u8; 4];
    data.chars
        .iter()
        .filter(|e| e.code().len() == 3)
        .map(|e| {
            data.frequency.candidate_weight(
                scale,
                CandidateSource::CharCode,
                e.hanzi().as_char().encode_utf8(&mut buf),
                e.code(),
                e.frequency_score(),
            )
        })
        .sum()
}

fn section_topics(out: &mut String, data: &AnalysisData) {
    // ── 二字词专题 ──────────────────────────────────────────────
    let (mut both, mut one, mut none) = (0usize, 0usize, 0usize);
    let (mut w_both, mut w_one, w_none) = (0.0f64, 0.0f64, 0.0f64);
    let mut w_none_acc = w_none;
    let mut two_total_weight = 0.0f64;
    for target in data
        .targets
        .iter()
        .filter(|t| t.word().chars().count() == 2)
    {
        let probability = data.frequency.word_probability(target.frequency_score());
        two_total_weight += probability;
        let empty = target
            .candidates()
            .iter()
            .filter(|c| data.occupancy.fanout(c.shortcut_code()) == 0)
            .count();
        let total = target.candidates().len();
        if total == 2 && empty == 2 {
            both += 1;
            w_both += probability;
        } else if empty >= 1 {
            one += 1;
            w_one += probability;
        } else {
            none += 1;
            w_none_acc += probability;
        }
    }
    writeln!(out, "## 二字词专题(50,000 词,每词最多 FI/IF 两个 3 键候选)").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "| 情况 | 词数 | 占比 | 频率加权占比 |").unwrap();
    let pct = |n: usize| 100.0 * n as f64 / (both + one + none).max(1) as f64;
    let wpct = |w: f64| 100.0 * w / two_total_weight.max(f64::MIN_POSITIVE);
    writeln!(
        out,
        "| FI/IF 两码都空闲 | {both} | {:.2}% | {:.2}% |",
        pct(both),
        wpct(w_both)
    )
    .unwrap();
    writeln!(
        out,
        "| 只有一个空闲 | {one} | {:.2}% | {:.2}% |",
        pct(one),
        wpct(w_one)
    )
    .unwrap();
    writeln!(
        out,
        "| 两个都已占用 | {none} | {:.2}% | {:.2}% |",
        pct(none),
        wpct(w_none_acc)
    )
    .unwrap();
    writeln!(out).unwrap();
    writeln!(
        out,
        "(若 FI/IF 因相邻键相同去重为单码,该词按「只有一个」计。)"
    )
    .unwrap();
    writeln!(out).unwrap();

    // ── 三/四字词专题 ────────────────────────────────────────────
    for (char_count, lengths) in [(3usize, vec![3usize, 4, 5]), (4, vec![4, 5, 6, 7])] {
        let group: Vec<&WordTarget> = data
            .targets
            .iter()
            .filter(|t| t.word().chars().count() == char_count)
            .collect();
        let total_weight: f64 = group
            .iter()
            .map(|t| data.frequency.word_probability(t.frequency_score()))
            .sum();
        writeln!(
            out,
            "## {char_count} 字词专题({} 词,full length {})",
            group.len(),
            char_count * 2
        )
        .unwrap();
        writeln!(out).unwrap();
        writeln!(
            out,
            "| shortcut len | candidates | 空码候选占比 | 频率加权可得性* |"
        )
        .unwrap();
        for length in &lengths {
            let mut candidates = 0usize;
            let mut empty = 0usize;
            let mut available_weight = 0.0f64;
            for target in &group {
                let probability = data.frequency.word_probability(target.frequency_score());
                let mut has_empty_at_length = false;
                for candidate in target
                    .candidates()
                    .iter()
                    .filter(|c| c.shortcut_code().len() == *length)
                {
                    candidates += 1;
                    if data.occupancy.fanout(candidate.shortcut_code()) == 0 {
                        empty += 1;
                        has_empty_at_length = true;
                    }
                }
                if has_empty_at_length {
                    available_weight += probability;
                }
            }
            writeln!(
                out,
                "| {length} | {candidates} | {:.2}% | {:.2}% |",
                100.0 * empty as f64 / candidates.max(1) as f64,
                100.0 * available_weight / total_weight.max(f64::MIN_POSITIVE),
            )
            .unwrap();
        }
        writeln!(out).unwrap();
        writeln!(
            out,
            "* 可得性 = 至少有一个该长度空码候选的词所占的频率加权比例。"
        )
        .unwrap();
        writeln!(out).unwrap();
    }
}

// ── profile 章节 ─────────────────────────────────────────────────

/// 模式统计行:(模式, 候选数, 推荐数, 频率加权节省, 平均现有扇出, 零碰撞比例)。
type PatternRow = (String, usize, usize, f64, f64, f64);

fn pattern_stats(data: &AnalysisData, outcome: &OptimizationOutcome) -> Vec<PatternRow> {
    #[derive(Default)]
    struct Accumulator {
        candidates: usize,
        assigned: usize,
        weighted_saving: f64,
        fanout_sum: usize,
        zero_collision: usize,
    }
    let assigned: BTreeMap<&str, (&KeySequence, f64)> = outcome
        .assignments
        .iter()
        .map(|a| {
            (
                a.word.as_str(),
                (
                    &a.evaluation.shortcut_code,
                    a.evaluation.breakdown.frequency_mass * a.keys_saved as f64,
                ),
            )
        })
        .collect();
    let mut rows: BTreeMap<String, Accumulator> = BTreeMap::new();
    for target in &data.targets {
        for candidate in target.candidates() {
            let pattern = candidate.mode().pattern();
            let acc = rows.entry(pattern.clone()).or_default();
            acc.candidates += 1;
            let fanout = data.occupancy.fanout(candidate.shortcut_code());
            acc.fanout_sum += fanout;
            if fanout == 0 {
                acc.zero_collision += 1;
            }
            if let Some((code, saving)) = assigned.get(target.word())
                && *code == candidate.shortcut_code()
            {
                acc.assigned += 1;
                acc.weighted_saving += saving;
            }
        }
    }
    rows.into_iter()
        .map(|(pattern, acc)| {
            (
                pattern,
                acc.candidates,
                acc.assigned,
                acc.weighted_saving,
                acc.fanout_sum as f64 / acc.candidates.max(1) as f64,
                acc.zero_collision as f64 / acc.candidates.max(1) as f64,
            )
        })
        .collect()
}

fn section_profile(
    out: &mut String,
    data: &AnalysisData,
    runs: &[SweepRun],
    profile: OptimizationProfile,
) {
    writeln!(out, "## Profile {}", profile.label()).unwrap();
    writeln!(out).unwrap();
    match profile {
        OptimizationProfile::EmptyLengthOnly => writeln!(
            out,
            "audit profile:只允许 len 5/7 的 shortcut(当前整体空闲码长层),\
             exact code 必须空闲。用于回答「仅利用整个未使用码长层,收益有多少」。"
        )
        .unwrap(),
        OptimizationProfile::ZeroRegression => writeln!(
            out,
            "ZERO EXACT-CODE REGRESSION:shortcut 码在当前 fixed exact 空间中完全空闲,\
             不重排任何已有 exact 候选组。注意:这不证明 Rime runtime prefix/composition\
             绝对零行为变化,接入 production 仍需 librime runtime acceptance。"
        )
        .unwrap(),
        OptimizationProfile::FixedFirst => writeln!(
            out,
            "shortcut 追加到所有现有固定候选之后;既有排名扰动恒为 0,\
             shortcut 自己承担较差名次的 selection cost。"
        )
        .unwrap(),
        OptimizationProfile::Optimized => writeln!(
            out,
            "纯分析模拟:shortcut 按混合频率参与排名。硬保护:1 键 immutable、\
             2 键保留(shortcut 长度 ≥3 不可达)、4 键规范全码 top 不被词语挤掉。\
             本 profile 不实际修改任何 production 排名。"
        )
        .unwrap(),
    }
    writeln!(out).unwrap();
    let Some(run) = find_balanced_run(runs, profile) else {
        writeln!(out, "(sweep 未包含该 profile)").unwrap();
        return;
    };
    let outcome = &run.outcome;
    let stats = &outcome.stats;
    writeln!(out, "balanced 运行(normalized 50:50 conservative):").unwrap();
    writeln!(out, "  targets:            {}", stats.targets).unwrap();
    writeln!(out, "  scored candidates:  {}", stats.scored_candidates).unwrap();
    writeln!(out, "  eligible:           {}", stats.eligible_candidates).unwrap();
    writeln!(out, "  assigned words:     {}", stats.assigned_words).unwrap();
    writeln!(
        out,
        "  weighted keys before: {:.6e}",
        stats.weighted_keys_before
    )
    .unwrap();
    writeln!(
        out,
        "  weighted keys after:  {:.6e}",
        stats.weighted_keys_after
    )
    .unwrap();
    writeln!(
        out,
        "  weighted keys saved:  {:.6e}",
        stats.weighted_keys_saved()
    )
    .unwrap();
    writeln!(
        out,
        "  saving:              {:.2}%",
        stats.saving_percentage()
    )
    .unwrap();
    writeln!(
        out,
        "  mean shortcut length: {:.2}",
        stats.mean_shortcut_length
    )
    .unwrap();
    writeln!(
        out,
        "  exact-code collisions: {}",
        stats.exact_code_collisions
    )
    .unwrap();
    if profile == OptimizationProfile::Optimized {
        writeln!(out, "  existing top1 changes: {}", stats.top1_changes).unwrap();
        writeln!(
            out,
            "  3-key char top1 changes: {}",
            stats.threekey_top1_changes
        )
        .unwrap();
        writeln!(
            out,
            "  weighted displaced mass: {:.6e}",
            stats.weighted_displaced_mass
        )
        .unwrap();
        writeln!(
            out,
            "  weighted disruption cost: {:.6e}",
            stats.weighted_disruption_cost
        )
        .unwrap();
    }
    writeln!(out).unwrap();

    // 模式统计。
    writeln!(out, "模式统计:").unwrap();
    writeln!(
        out,
        "| pattern | candidates | assigned | weighted saving | mean fanout | zero-collision |"
    )
    .unwrap();
    for (pattern, candidates, assigned, saving, mean_fanout, zero_ratio) in
        pattern_stats(data, outcome)
    {
        writeln!(
            out,
            "| {pattern} | {candidates} | {assigned} | {saving:.4e} | {mean_fanout:.2} | {:.2}% |",
            zero_ratio * 100.0
        )
        .unwrap();
    }
    writeln!(out).unwrap();

    // Top 30 推荐。
    writeln!(out, "Top 30 recommendations:").unwrap();
    writeln!(
        out,
        "| # | word | code | mode | frequency | keys saved | existing fanout | projected rank | net utility |"
    )
    .unwrap();
    for (index, assignment) in outcome.assignments.iter().take(30).enumerate() {
        let e = &assignment.evaluation;
        writeln!(
            out,
            "| {} | {} | {} | {} | {} | {} | {} | {} | {:.4e} |",
            index + 1,
            assignment.word,
            e.shortcut_code,
            e.mode,
            assignment.frequency_score,
            assignment.keys_saved,
            e.existing_fanout,
            e.projected_rank,
            e.breakdown.net_utility,
        )
        .unwrap();
    }
    writeln!(out).unwrap();

    // Top 30 被拒绝的高频词。
    let mut rejected: Vec<&WordTarget> = data
        .targets
        .iter()
        .filter(|t| !outcome.assignments.iter().any(|a| a.word == t.word()))
        .collect();
    rejected.sort_by(|a, b| {
        b.frequency_score()
            .cmp(&a.frequency_score())
            .then(a.word().cmp(b.word()))
    });
    writeln!(out, "Top 30 rejected high-frequency words(无合格候选):").unwrap();
    writeln!(
        out,
        "| # | word | frequency | full code | baseline fanout | baseline rank |"
    )
    .unwrap();
    for (index, target) in rejected.into_iter().take(30).enumerate() {
        let group = data
            .occupancy
            .group(target.full_code())
            .expect("完整码组存在");
        let rank = group
            .iter()
            .find(|c| c.source() == CandidateSource::FixedWord && c.text() == target.word())
            .expect("词占用其完整码")
            .rank();
        writeln!(
            out,
            "| {} | {} | {} | {} | {} | {} |",
            index + 1,
            target.word(),
            target.frequency_score(),
            target.full_code(),
            group.len(),
            rank,
        )
        .unwrap();
    }
    writeln!(out).unwrap();

    if profile == OptimizationProfile::Optimized {
        let mut disruptions = outcome.disruptions.clone();
        disruptions.sort_by(|a, b| {
            b.weighted_cost
                .total_cmp(&a.weighted_cost)
                .then(a.code.cmp(&b.code))
                .then(a.text.cmp(&b.text))
        });
        writeln!(out, "Top 30 most expensive disruptions:").unwrap();
        writeln!(
            out,
            "| # | code | text | source | rank | delta cost | weighted cost |"
        )
        .unwrap();
        for (index, d) in disruptions.iter().take(30).enumerate() {
            writeln!(
                out,
                "| {} | {} | {} | {} | {}→{} | {:.3} | {:.4e} |",
                index + 1,
                d.code,
                d.text,
                d.source.label(),
                d.old_rank,
                d.new_rank,
                d.delta_cost,
                d.weighted_cost,
            )
            .unwrap();
        }
        writeln!(out).unwrap();
    }

    if profile == OptimizationProfile::EmptyLengthOnly {
        for char_count in [3usize, 4] {
            let group: Vec<&WordTarget> = data
                .targets
                .iter()
                .filter(|t| t.word().chars().count() == char_count)
                .collect();
            let total_weight: f64 = group
                .iter()
                .map(|t| data.frequency.word_probability(t.frequency_score()))
                .sum();
            let covered: Vec<&&WordTarget> = group
                .iter()
                .filter(|t| outcome.assignments.iter().any(|a| a.word == t.word()))
                .collect();
            let covered_weight: f64 = covered
                .iter()
                .map(|t| data.frequency.word_probability(t.frequency_score()))
                .sum();
            writeln!(
                out,
                "{char_count}-char 覆盖:{}/{} 词({:.2}%),频率加权 {:.2}%",
                covered.len(),
                group.len(),
                100.0 * covered.len() as f64 / group.len().max(1) as f64,
                100.0 * covered_weight / total_weight.max(f64::MIN_POSITIVE),
            )
            .unwrap();
        }
        writeln!(
            out,
            "2-char 词的 shortcut 为 3 键,不在本 profile 的 5/7 键范围内。"
        )
        .unwrap();
        writeln!(out).unwrap();
    }
}

fn section_operating_points(out: &mut String, runs: &[SweepRun]) {
    writeln!(out, "## Operating points(50:50 conservative 运行)").unwrap();
    writeln!(out).unwrap();
    writeln!(
        out,
        "五个代表性参数组合;不构成「最优参数」结论,供后续产品决策参考。"
    )
    .unwrap();
    writeln!(out).unwrap();
    for profile in OptimizationProfile::primary() {
        writeln!(out, "### {}", profile.label()).unwrap();
        writeln!(
            out,
            "| point | assigned | weighted saving | saving % | collisions | top1 changes | weighted disruption | avg len |"
        )
        .unwrap();
        for point in operating_points() {
            let run = runs.iter().find(|r| {
                r.profile == profile
                    && r.point == point.name
                    && !r.diagnostic
                    && matches!(
                        r.scale,
                        FrequencyScale::Normalized {
                            char_share,
                            usage: CharCodeUsage::Conservative,
                        } if char_share == 0.5
                    )
            });
            if let Some(run) = run {
                let s = &run.outcome.stats;
                writeln!(
                    out,
                    "| {} | {} | {:.4e} | {:.2}% | {} | {} | {:.4e} | {:.2} |",
                    point.name,
                    s.assigned_words,
                    s.weighted_keys_saved(),
                    s.saving_percentage(),
                    s.exact_code_collisions,
                    s.top1_changes,
                    s.weighted_disruption_cost,
                    s.mean_shortcut_length,
                )
                .unwrap();
            }
        }
        writeln!(out).unwrap();
    }
}

fn section_robustness(out: &mut String, runs: &[SweepRun]) {
    writeln!(out, "## Robustness(主网格 30 次运行/profile)").unwrap();
    writeln!(out).unwrap();
    writeln!(
        out,
        "分级按同码稳定度(得票最多的码的选中比例):HIGH ≥ 80%,MEDIUM ≥ 40%,LOW < 40%。\
         「该词应简化」与「应固定成哪个码」是两个不同问题,故同时记录选中率与同码稳定度。"
    )
    .unwrap();
    writeln!(out).unwrap();
    for profile in OptimizationProfile::primary() {
        let map = robustness_map(runs, profile);
        let Some(run) = find_balanced_run(runs, profile) else {
            continue;
        };
        let (mut high, mut medium, mut low) = (0usize, 0usize, 0usize);
        for assignment in &run.outcome.assignments {
            let stability = map
                .get(&assignment.word)
                .map_or(0.0, |r| r.same_code_stability());
            match classify(stability) {
                Robustness::High => high += 1,
                Robustness::Medium => medium += 1,
                Robustness::Low => low += 1,
            }
        }
        writeln!(
            out,
            "{}: balanced 推荐 {} 条 — HIGH {high} / MEDIUM {medium} / LOW {low}",
            profile.label(),
            run.outcome.assignments.len(),
        )
        .unwrap();
    }
    writeln!(out).unwrap();
    // 时间 稳健性明细。
    writeln!(out, "时间 的 sweep 投票(各 profile 主网格):").unwrap();
    for profile in OptimizationProfile::primary() {
        let map = robustness_map(runs, profile);
        match map.get("时间") {
            Some(record) => {
                let votes: Vec<String> = record
                    .code_votes
                    .iter()
                    .map(|(code, n)| format!("{code}×{n}"))
                    .collect();
                writeln!(
                    out,
                    "  {}: selected {}/{}, votes: {}",
                    profile.label(),
                    record.selected_runs,
                    record.total_runs,
                    votes.join(" "),
                )
                .unwrap();
            }
            None => writeln!(out, "  {}: 从未被选中", profile.label()).unwrap(),
        }
    }
    writeln!(out).unwrap();
}

// ── 哨兵章节 ─────────────────────────────────────────────────

const SENTINELS: [&str; 7] = ["时间", "我们", "输入法", "社会主义", "中国", "可以", "一直"];

fn section_sentinels(out: &mut String, data: &AnalysisData, runs: &[SweepRun]) {
    let scale = balanced_scale();
    let cost = balanced_cost();
    writeln!(out, "## 哨兵详查(balanced 模型)").unwrap();
    writeln!(out).unwrap();

    // ── 时间:强制详细报告 ──
    let target = data
        .targets
        .iter()
        .find(|t| t.word() == "时间")
        .expect("哨兵:时间必然在 production 词表中");
    let (chunk_pairs, _) = target.full_code().as_slice().as_chunks::<2>();
    let chunks: Vec<String> = chunk_pairs
        .iter()
        .map(|chunk| chunk.iter().map(|k| k.as_char()).collect())
        .collect();
    writeln!(out, "### 时间").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "- frequency score: {}", target.frequency_score()).unwrap();
    writeln!(
        out,
        "- full code: {}({})",
        target.full_code(),
        chunks.join(" + ")
    )
    .unwrap();
    let baseline_group = data
        .occupancy
        .group(target.full_code())
        .expect("完整码组存在");
    let baseline_rank = baseline_group
        .iter()
        .find(|c| c.source() == CandidateSource::FixedWord && c.text() == "时间")
        .expect("时间占用其完整码")
        .rank();
    let baseline = cost.baseline_cost(
        target.full_code().len(),
        baseline_rank,
        baseline_group.len(),
    );
    writeln!(out, "- baseline fanout: {}", baseline_group.len()).unwrap();
    writeln!(out, "- baseline rank: {baseline_rank}").unwrap();
    writeln!(out, "- baseline candidates:").unwrap();
    for c in baseline_group {
        writeln!(
            out,
            "    {}. {} [{}] weight={} score={}",
            c.rank(),
            c.text(),
            c.source().label(),
            c.rime_weight(),
            c.frequency_score()
        )
        .unwrap();
    }
    writeln!(
        out,
        "- baseline effective cost: {:.3}(typed {} + selection {:.2} + ambiguity {:.2})",
        baseline.total(),
        baseline.typed_keys,
        baseline.selection,
        baseline.ambiguity
    )
    .unwrap();
    writeln!(out).unwrap();
    for (index, candidate) in target.candidates().iter().enumerate() {
        writeln!(
            out,
            "#### candidate {}: {}({})",
            index + 1,
            candidate.shortcut_code(),
            candidate.mode().pattern()
        )
        .unwrap();
        writeln!(out, "- keys saved: {}", target.keys_saved(candidate)).unwrap();
        writeln!(
            out,
            "- existing fanout: {}",
            data.occupancy.fanout(candidate.shortcut_code())
        )
        .unwrap();
        writeln!(
            out,
            "- collision class: {}",
            data.occupancy
                .collision_class(candidate.shortcut_code())
                .label()
        )
        .unwrap();
        if let Some(group) = data.occupancy.group(candidate.shortcut_code()) {
            writeln!(out, "- existing candidates:").unwrap();
            for c in group.iter().take(5) {
                writeln!(
                    out,
                    "    {}. {} [{}] weight={} score={}",
                    c.rank(),
                    c.text(),
                    c.source().label(),
                    c.rime_weight(),
                    c.frequency_score()
                )
                .unwrap();
            }
            if group.len() > 5 {
                writeln!(out, "    …(共 {} 条)", group.len()).unwrap();
            }
        }
        for profile in OptimizationProfile::primary() {
            let evaluation = evaluate_target(
                target,
                &data.occupancy,
                &data.frequency,
                &scale,
                &cost,
                profile,
            )
            .into_iter()
            .find(|e| e.shortcut_code == *candidate.shortcut_code())
            .expect("候选评估必然存在");
            if let Some(reason) = evaluation.gate_reason {
                writeln!(out, "- {}: gate 拒绝({reason})", profile.label()).unwrap();
            } else {
                let b = &evaluation.breakdown;
                writeln!(
                    out,
                    "- {}: projected rank {}/fanout {};effective {:.3} = typed {} + sel {:.2} + amb {:.2} + mode {:.2};gross {:.4e};disruption {:.4e};net {:.4e};{}",
                    profile.label(),
                    evaluation.projected_rank,
                    evaluation.projected_fanout,
                    b.shortcut.total(),
                    b.shortcut.typed_keys,
                    b.shortcut.selection,
                    b.shortcut.ambiguity,
                    b.shortcut.mode_complexity,
                    b.gross_saving,
                    b.disruption_cost,
                    b.net_utility,
                    if evaluation.eligible { "eligible" } else { "not eligible(非正收益)" },
                )
                .unwrap();
            }
        }
        writeln!(out).unwrap();
    }
    writeln!(out, "时间的 balanced 推荐:").unwrap();
    for profile in OptimizationProfile::primary() {
        let recommendation = find_balanced_run(runs, profile).and_then(|run| {
            run.outcome
                .assignments
                .iter()
                .find(|a| a.word == "时间")
                .map(|a| a.evaluation.shortcut_code.to_string())
        });
        writeln!(
            out,
            "  {}: {}",
            profile.label(),
            recommendation.unwrap_or_else(|| "无推荐".to_string())
        )
        .unwrap();
    }
    writeln!(out).unwrap();

    // ── 其他哨兵简表 ──
    writeln!(out, "### 其他哨兵").unwrap();
    writeln!(out).unwrap();
    for word in SENTINELS.iter().skip(1) {
        let Some(target) = data.targets.iter().find(|t| t.word() == *word) else {
            writeln!(out, "- {word}: 不在 production 词表").unwrap();
            continue;
        };
        let group = data
            .occupancy
            .group(target.full_code())
            .expect("完整码组存在");
        let rank = group
            .iter()
            .find(|c| c.source() == CandidateSource::FixedWord && c.text() == *word)
            .expect("词占用其完整码")
            .rank();
        let candidates: Vec<String> = target
            .candidates()
            .iter()
            .map(|c| {
                format!(
                    "{}({} fanout={} {})",
                    c.shortcut_code(),
                    c.mode().pattern(),
                    data.occupancy.fanout(c.shortcut_code()),
                    data.occupancy.collision_class(c.shortcut_code()).label()
                )
            })
            .collect();
        let recommendations: Vec<String> = OptimizationProfile::primary()
            .iter()
            .map(|&profile| {
                let code = find_balanced_run(runs, profile).and_then(|run| {
                    run.outcome
                        .assignments
                        .iter()
                        .find(|a| a.word == *word)
                        .map(|a| a.evaluation.shortcut_code.to_string())
                });
                format!(
                    "{}={}",
                    profile.label(),
                    code.unwrap_or_else(|| "-".to_string())
                )
            })
            .collect();
        writeln!(
            out,
            "- {word} score={} full={} baseline(fanout={} rank={})",
            target.frequency_score(),
            target.full_code(),
            group.len(),
            rank,
        )
        .unwrap();
        writeln!(out, "    candidates: {}", candidates.join(" ")).unwrap();
        writeln!(out, "    recommended: {}", recommendations.join(" ")).unwrap();
    }
    writeln!(out).unwrap();

    // ── top 20 高频词 ──
    let mut by_frequency: Vec<&WordTarget> = data.targets.iter().collect();
    by_frequency.sort_by(|a, b| {
        b.frequency_score()
            .cmp(&a.frequency_score())
            .then(a.word().cmp(b.word()))
    });
    writeln!(out, "### Top 20 高频词").unwrap();
    writeln!(out).unwrap();
    writeln!(
        out,
        "| # | word | frequency | full | baseline fanout | baseline rank | ZR | FF | OPT |"
    )
    .unwrap();
    for (index, target) in by_frequency.into_iter().take(20).enumerate() {
        let group = data
            .occupancy
            .group(target.full_code())
            .expect("完整码组存在");
        let rank = group
            .iter()
            .find(|c| c.source() == CandidateSource::FixedWord && c.text() == target.word())
            .expect("词占用其完整码")
            .rank();
        let codes: Vec<String> = OptimizationProfile::primary()
            .iter()
            .map(|&profile| {
                find_balanced_run(runs, profile)
                    .and_then(|run| {
                        run.outcome
                            .assignments
                            .iter()
                            .find(|a| a.word == target.word())
                            .map(|a| a.evaluation.shortcut_code.to_string())
                    })
                    .unwrap_or_else(|| "-".to_string())
            })
            .collect();
        writeln!(
            out,
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} |",
            index + 1,
            target.word(),
            target.frequency_score(),
            target.full_code(),
            group.len(),
            rank,
            codes[0],
            codes[1],
            codes[2],
        )
        .unwrap();
    }
    writeln!(out).unwrap();
}

fn section_raw_diagnostic(out: &mut String, runs: &[SweepRun]) {
    writeln!(out, "## raw-score 诊断(非推荐依据)").unwrap();
    writeln!(out).unwrap();
    writeln!(
        out,
        "以下运行直接混用 raw 词/字分数,未做 domain 归一化;仅用于检验推荐对\
         尺度假设的敏感性,不得作为最终 recommendation 的依据。"
    )
    .unwrap();
    writeln!(out).unwrap();
    writeln!(
        out,
        "| profile | assigned | weighted saving | saving % | collisions | top1 changes |"
    )
    .unwrap();
    for profile in OptimizationProfile::primary() {
        if let Some(run) = runs.iter().find(|r| r.profile == profile && r.diagnostic) {
            let s = &run.outcome.stats;
            writeln!(
                out,
                "| {} | {} | {:.4e} | {:.2}% | {} | {} |",
                profile.label(),
                s.assigned_words,
                s.weighted_keys_saved(),
                s.saving_percentage(),
                s.exact_code_collisions,
                s.top1_changes,
            )
            .unwrap();
        }
    }
    writeln!(out).unwrap();
}

// ── TSV 转储 ─────────────────────────────────────────────────

/// 转储全部候选(balanced 模型,三个主 profile 的 utility/eligible)。
pub fn dump_candidates_tsv(data: &AnalysisData) -> String {
    let scale = balanced_scale();
    let cost = balanced_cost();
    let mut out = String::from(
        "word\tfrequency\tfull_code\tshortcut_code\tmode\tkeys_saved\tbaseline_fanout\t\
         baseline_rank\texisting_fanout\texisting_sources\tcollision_class\t\
         utility_zero_regression\tutility_fixed_first\tutility_optimized\t\
         eligible_zero_regression\teligible_fixed_first\teligible_optimized\n",
    );
    for target in &data.targets {
        let evaluations: Vec<Vec<_>> = OptimizationProfile::primary()
            .iter()
            .map(|&profile| {
                evaluate_target(
                    target,
                    &data.occupancy,
                    &data.frequency,
                    &scale,
                    &cost,
                    profile,
                )
            })
            .collect();
        for (index, candidate) in target.candidates().iter().enumerate() {
            let code = candidate.shortcut_code();
            let sources = data
                .occupancy
                .group(code)
                .map(|group| {
                    let mut labels: Vec<&str> = group.iter().map(|c| c.source().label()).collect();
                    labels.sort_unstable();
                    labels.dedup();
                    labels.join("+")
                })
                .unwrap_or_else(|| "none".to_string());
            let mut row = format!(
                "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
                target.word(),
                target.frequency_score(),
                target.full_code(),
                code,
                candidate.mode().pattern(),
                target.keys_saved(candidate),
                evaluations[0][index].baseline_fanout,
                evaluations[0][index].baseline_rank,
                data.occupancy.fanout(code),
                sources,
                data.occupancy.collision_class(code).label(),
            );
            for evaluation in &evaluations {
                let e = &evaluation[index];
                if e.gate_reason.is_some() {
                    row.push_str("\tNA\t0");
                } else {
                    write!(
                        row,
                        "\t{:.6e}\t{}",
                        e.breakdown.net_utility,
                        if e.eligible { 1 } else { 0 }
                    )
                    .unwrap();
                }
            }
            out.push_str(&row);
            out.push('\n');
        }
    }
    out
}

/// 转储推荐结果(三个主 profile 的 balanced 运行 + 稳健性)。
pub fn dump_recommendations_tsv(runs: &[SweepRun]) -> String {
    let mut out = String::from(
        "profile\tword\tfull_code\tshortcut_code\tmode\tfrequency\tkeys_saved\t\
         existing_fanout\tprojected_rank\tgross_saving\tdisruption_cost\tnet_utility\t\
         robustness\tselected_runs\ttotal_runs\ttop_code\ttop_code_votes\n",
    );
    for profile in OptimizationProfile::primary() {
        let map = robustness_map(runs, profile);
        let Some(run) = find_balanced_run(runs, profile) else {
            continue;
        };
        for assignment in &run.outcome.assignments {
            let e = &assignment.evaluation;
            let record = map.get(&assignment.word);
            let (robustness, selected, total, top_code, top_votes) = match record {
                Some(r) => {
                    let (top_code, top_votes) = r.top_code().unwrap_or(("-", 0));
                    (
                        classify(r.same_code_stability()).label(),
                        r.selected_runs,
                        r.total_runs,
                        top_code.to_string(),
                        top_votes,
                    )
                }
                None => ("-", 0, 0, "-".to_string(), 0),
            };
            writeln!(
                out,
                "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{:.6e}\t{:.6e}\t{:.6e}\t{}\t{}\t{}\t{}\t{}",
                profile.label(),
                assignment.word,
                assignment.full_code,
                e.shortcut_code,
                e.mode,
                assignment.frequency_score,
                assignment.keys_saved,
                e.existing_fanout,
                e.projected_rank,
                e.breakdown.gross_saving,
                e.breakdown.disruption_cost,
                e.breakdown.net_utility,
                robustness,
                selected,
                total,
                top_code,
                top_votes,
            )
            .unwrap();
        }
    }
    out
}
