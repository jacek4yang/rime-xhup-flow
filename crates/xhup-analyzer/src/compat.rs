//! 兼容性对照:当前 canonical 映射 vs 外部参考映射(如传统/官方小鹤)。
//!
//! 用途(docs/input-model-v2.md §6 / 发布流程):任何大规模重映射之前,
//! 必须用本模块产出对照报告 —— 兼容率、候选序相似度、逐条差异,供
//! 评审选择工作点。参考映射由使用方本地提供(TSV),**绝不入库**
//! (许可证红线:官方派生数据只能本地参考,见 docs/research 调研)。
//!
//! 参考 TSV 格式(UTF-8,`#` 起注释行):`文本<TAB>码<TAB>参考排名`
//! (排名 1 = 首选)。码长即层:1 键一级简码、2 键、3 键、≥4 键全码。

use std::collections::BTreeMap;

use xhup_generator::{
    canonical_fixed_first_shortcut_entries, canonical_level1_shortcuts,
    canonical_two_key_shortcut_entries, canonical_word_shortcut_entries,
};

use crate::{CharCodeAnalysisEntry, WordCodeAnalysisEntry};

/// 当前映射中的一条命中:所在层 + 该层内排名(1 = 首选)。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CurrentHit {
    /// 层标签(稳定字符串,报告用)。
    pub layer: &'static str,
    /// 该层内同码排名(1 = 首选)。
    pub rank: usize,
}

/// 一条参考映射的对照结果。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompatRow {
    /// 参考文本。
    pub text: String,
    /// 参考码。
    pub code: String,
    /// 参考排名。
    pub reference_rank: u32,
    /// 当前映射在同码上的最佳命中(跨层取最小 rank);None = 同码无此文本。
    pub current: Option<CurrentHit>,
}

impl CompatRow {
    /// 同码映射保留(任意层命中)。
    pub fn preserved(&self) -> bool {
        self.current.is_some()
    }

    /// 同码且当前为首选(最佳命中 rank 1)。
    pub fn preserved_as_first(&self) -> bool {
        matches!(self.current, Some(hit) if hit.rank == 1)
    }

    /// 同码且当前在前二。
    pub fn preserved_within_top2(&self) -> bool {
        matches!(self.current, Some(hit) if hit.rank <= 2)
    }
}

/// 单码长桶的兼容率统计。
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TierCompat {
    /// 参考条目数。
    pub total: usize,
    /// 同码映射保留数。
    pub preserved: usize,
    /// 同码且当前首选数。
    pub first: usize,
    /// 同码且当前前二数。
    pub top2: usize,
}

impl TierCompat {
    fn rate(self, count: usize) -> f64 {
        count as f64 / self.total.max(1) as f64
    }

    /// 保留率(0..=1)。
    pub fn preservation_rate(self) -> f64 {
        self.rate(self.preserved)
    }

    /// 首选一致率(0..=1)。
    pub fn first_rate(self) -> f64 {
        self.rate(self.first)
    }

    /// 前二一致率(0..=1)。
    pub fn top2_rate(self) -> f64 {
        self.rate(self.top2)
    }
}

/// 兼容性对照报告:按参考码长分桶 + 全量。
#[derive(Clone, Debug, PartialEq)]
pub struct CompatibilityReport {
    /// 码长 → 桶统计(1/2/3/4 键;4 键桶含 ≥4 键全码)。
    pub tiers: BTreeMap<usize, TierCompat>,
    /// 全量合计。
    pub overall: TierCompat,
    /// 逐条对照(报告/TSV 转储用)。
    pub rows: Vec<CompatRow>,
}

/// 参考映射条目(TSV 解析结果)。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReferenceEntry {
    /// 参考文本。
    pub text: String,
    /// 参考码(小写 ASCII)。
    pub code: String,
    /// 参考排名(1 = 首选)。
    pub rank: u32,
}

/// 解析参考 TSV(`文本<TAB>码<TAB>排名`;`#` 注释;空行跳过)。
pub fn parse_reference_tsv(text: &str) -> Result<Vec<ReferenceEntry>, String> {
    let mut entries = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let line = line.trim_end();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let fields: Vec<&str> = line.split('\t').collect();
        let [text, code, rank] = fields.as_slice() else {
            return Err(format!(
                "第 {} 行应为三列 文本<TAB>码<TAB>排名: {line:?}",
                index + 1
            ));
        };
        if !code.bytes().all(|b| b.is_ascii_lowercase()) {
            return Err(format!("第 {} 行码应为小写 ASCII: {code:?}", index + 1));
        }
        let rank: u32 = rank
            .parse()
            .map_err(|_| format!("第 {} 行排名应为正整数: {rank:?}", index + 1))?;
        if rank == 0 {
            return Err(format!("第 {} 行排名应 ≥1", index + 1));
        }
        entries.push(ReferenceEntry {
            text: (*text).to_string(),
            code: (*code).to_string(),
            rank,
        });
    }
    Ok(entries)
}

/// 当前 canonical 映射的统一视图:码 → 各层 (文本, 层内排名) 列表。
///
/// 层内排名:静态全码层按显式 Rime 权重降序(merged_ranking 已保证
/// 碰撞码字词跨表统一);各简码层(一级/ZR/FF/二码)的码位在各自词典
/// 内唯一映射一个文本,排名恒 1。
fn build_current_index() -> BTreeMap<String, Vec<(String, CurrentHit)>> {
    let mut index = build_baseline_index();

    // 词语简码层(ZR / FIXED_FIRST / 二码零冲突):码位唯一映射,rank 恒 1。
    let shortcut_layers: [(&str, Vec<(String, String)>); 3] = [
        (
            "zero-regression",
            canonical_word_shortcut_entries()
                .iter()
                .map(|e| (e.shortcut_code().to_string(), e.word().to_string()))
                .collect(),
        ),
        (
            "fixed-first",
            canonical_fixed_first_shortcut_entries()
                .iter()
                .map(|e| (e.shortcut_code().to_string(), e.word().to_string()))
                .collect(),
        ),
        (
            "two-key",
            canonical_two_key_shortcut_entries()
                .iter()
                .map(|e| (e.shortcut_code().to_string(), e.word().to_string()))
                .collect(),
        ),
    ];
    for (layer, entries) in shortcut_layers {
        for (code, text) in entries {
            index
                .entry(code)
                .or_default()
                .push((text, CurrentHit { layer, rank: 1 }));
        }
    }
    index
}

/// baseline 索引:静态全码层(单字 2/3/4 码 + 词语 4/6/8 码,同码按权重
/// 降序)+ 一级简码层,**不含**已入库的词语简码层。
///
/// 供 v2 扫描等「任意映射」对照:在 baseline 上叠加调用方的简码层后
/// 传入 [`compare_with_index`]。
pub fn build_baseline_index() -> BTreeMap<String, Vec<(String, CurrentHit)>> {
    let mut index: BTreeMap<String, Vec<(String, CurrentHit)>> = BTreeMap::new();

    // 静态全码层:单字(2/3/4 码)+ 词(4/6/8 码),同码按权重降序排名。
    let mut full_by_code: BTreeMap<String, Vec<(u32, String)>> = BTreeMap::new();
    let chars: Vec<CharCodeAnalysisEntry> = xhup_generator::char_code_analysis_entries();
    for entry in &chars {
        full_by_code
            .entry(entry.code().to_string())
            .or_default()
            .push((entry.rime_weight(), entry.hanzi().as_char().to_string()));
    }
    let words: Vec<WordCodeAnalysisEntry> = xhup_generator::word_code_analysis_entries();
    for entry in &words {
        full_by_code
            .entry(entry.code().to_string())
            .or_default()
            .push((entry.rime_weight(), entry.word().to_string()));
    }
    for (code, mut entries) in full_by_code {
        entries.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        for (rank, (_, text)) in entries.into_iter().enumerate() {
            index.entry(code.clone()).or_default().push((
                text,
                CurrentHit {
                    layer: "full",
                    rank: rank + 1,
                },
            ));
        }
    }

    // 一级简码层(1 键,一键一字,rank 恒 1)。
    for entry in canonical_level1_shortcuts() {
        index
            .entry(entry.key().as_char().to_string())
            .or_default()
            .push((
                entry.hanzi().as_char().to_string(),
                CurrentHit {
                    layer: "level1",
                    rank: 1,
                },
            ));
    }
    index
}

/// 参考码长分桶键:1/2/3 键各自成桶,≥4 键合并为全码桶。
fn tier_bucket(code: &str) -> usize {
    code.chars().count().min(4)
}

/// 对照参考映射与当前 canonical 映射,产出兼容率报告。
pub fn compare(reference: &[ReferenceEntry]) -> CompatibilityReport {
    compare_with_index(reference, &build_current_index())
}

/// 对照参考映射与调用方提供的映射索引(v2 扫描等任意映射评估)。
///
/// 索引语义与 `build_current_index` 一致:码 → 各层 (文本, 层内排名);
/// 同一 (码, 文本) 跨层命中取最小 rank。
pub fn compare_with_index(
    reference: &[ReferenceEntry],
    index: &BTreeMap<String, Vec<(String, CurrentHit)>>,
) -> CompatibilityReport {
    let mut tiers: BTreeMap<usize, TierCompat> = BTreeMap::new();
    let mut overall = TierCompat::default();
    let mut rows = Vec::with_capacity(reference.len());

    for entry in reference {
        let current = index.get(&entry.code).and_then(|hits| {
            hits.iter()
                .filter(|(text, _)| *text == entry.text)
                .map(|(_, hit)| *hit)
                .min_by_key(|hit| hit.rank)
        });
        let row = CompatRow {
            text: entry.text.clone(),
            code: entry.code.clone(),
            reference_rank: entry.rank,
            current,
        };
        let tier = tiers.entry(tier_bucket(&entry.code)).or_default();
        for stats in [&mut *tier, &mut overall] {
            stats.total += 1;
            stats.preserved += row.preserved() as usize;
            stats.first += row.preserved_as_first() as usize;
            stats.top2 += row.preserved_within_top2() as usize;
        }
        rows.push(row);
    }
    CompatibilityReport {
        tiers,
        overall,
        rows,
    }
}

/// 渲染人类可读报告(确定性文本)。
pub fn render_report(report: &CompatibilityReport) -> String {
    let mut out = String::new();
    out.push_str("兼容率对照(当前 canonical 映射 vs 参考映射)\n");
    out.push_str("码长\t条目\t保留%\t首选%\t前二%\n");
    for (len, tier) in &report.tiers {
        let label = if *len == 4 { "≥4(全码)" } else { "键" };
        out.push_str(&format!(
            "{}{}\t{}\t{:.1}\t{:.1}\t{:.1}\n",
            len,
            label,
            tier.total,
            tier.preservation_rate() * 100.0,
            tier.first_rate() * 100.0,
            tier.top2_rate() * 100.0,
        ));
    }
    out.push_str(&format!(
        "合计\t{}\t{:.1}\t{:.1}\t{:.1}\n",
        report.overall.total,
        report.overall.preservation_rate() * 100.0,
        report.overall.first_rate() * 100.0,
        report.overall.top2_rate() * 100.0,
    ));
    out
}

/// 转储逐条差异 TSV:`文本<TAB>参考码<TAB>参考排名<TAB>当前层<TAB>当前排名`。
/// 未命中行为 `MISSING`(供 diff review 聚焦流失映射)。
pub fn dump_diff_tsv(report: &CompatibilityReport) -> String {
    let mut out = String::new();
    out.push_str("text\tref_code\tref_rank\tcurrent_layer\tcurrent_rank\n");
    for row in &report.rows {
        let (layer, rank) = match &row.current {
            Some(hit) => (hit.layer.to_string(), hit.rank.to_string()),
            None => ("MISSING".to_string(), "-".to_string()),
        };
        out.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\n",
            row.text, row.code, row.reference_rank, layer, rank
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    //! 用真实 canonical 映射构造合成参考条目,验证对照语义。
    use super::*;

    #[test]
    fn parse_reference_tsv_validates_shape() {
        let ok = parse_reference_tsv("# 注释\n我们\twomf\t1\n\n时间\tuij\t3\n").unwrap();
        assert_eq!(ok.len(), 2);
        assert_eq!(ok[0].text, "我们");
        assert!(parse_reference_tsv("我们\twomf\n").is_err(), "缺列应报错");
        assert!(
            parse_reference_tsv("我们\tWOMF\t1\n").is_err(),
            "大写码应报错"
        );
        assert!(
            parse_reference_tsv("我们\twomf\t0\n").is_err(),
            "排名 0 应报错"
        );
    }

    #[test]
    fn identical_reference_scores_perfect_compatibility() {
        // 参考 = 当前 ZR 简码层自身 → 该层全部条目必须 100% 首选保留。
        let reference: Vec<ReferenceEntry> = canonical_word_shortcut_entries()
            .iter()
            .take(200)
            .map(|e| ReferenceEntry {
                text: e.word().to_string(),
                code: e.shortcut_code().to_string(),
                rank: 1,
            })
            .collect();
        let report = compare(&reference);
        assert_eq!(report.overall.total, 200);
        assert_eq!(report.overall.preservation_rate(), 1.0);
        assert_eq!(report.overall.first_rate(), 1.0);
    }

    #[test]
    fn missing_reference_entry_is_reported() {
        // 一个必然不存在的虚构映射 → MISSING,兼容率下降可见。
        let reference = parse_reference_tsv("我\twomf\t1\n").unwrap();
        let report = compare(&reference);
        assert_eq!(report.rows.len(), 1);
        assert!(!report.rows[0].preserved(), "单字不应占用词全码 womf");
        let tsv = dump_diff_tsv(&report);
        assert!(tsv.contains("我\twomf\t1\tMISSING\t-"));
    }

    #[test]
    fn fixed_first_layer_hit_reports_layer_identity() {
        // 层身份哨兵:FF 简码「时间 uij」必须在 fixed-first 层命中
        // (runtime 审计锚点:uij 铈→鼫→时间,FF 词典内唯一)。
        let reference = parse_reference_tsv("时间\tuij\t1\n").unwrap();
        let report = compare(&reference);
        let hit = report.rows[0].current.expect("时间 uij 应命中");
        assert_eq!(hit.layer, "fixed-first");
        assert_eq!(hit.rank, 1);
    }

    #[test]
    fn full_code_collision_ranking_is_visible() {
        // 碰撞共存哨兵:参考「什么 ufme」首选 → 当前 full 层 rank 1
        // (merged_ranking:什么 3 > 甚么 2 > 𬳽 1)。
        let reference = parse_reference_tsv("什么\tufme\t1\n").unwrap();
        let report = compare(&reference);
        let hit = report.rows[0].current.expect("什么 ufme 应命中");
        assert_eq!(hit.layer, "full");
        assert_eq!(hit.rank, 1);
    }

    #[test]
    fn tier_buckets_split_by_code_length() {
        let reference =
            parse_reference_tsv("我们\twomf\t1\n时间\tuij\t1\n什么\tufme\t1\n").unwrap();
        let report = compare(&reference);
        assert!(report.tiers.contains_key(&3), "3 键桶应存在(uij)");
        assert!(report.tiers.contains_key(&4), "全码桶应存在(womf/ufme)");
        assert_eq!(report.overall.total, 3);
    }
}
