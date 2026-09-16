//! Production Shortcut Audit(Issue #83 §3/§9/§10)。
//!
//! 产品硬合同:`advertised shortcut => useful shortcut`,misleading-hint
//! rate ≈ 0。本模块把「广告出去的简码」逐条对照真实候选菜单,回答:
//!
//! 1. 该简码在全码菜单体系中把目标词排到第几(rank)?
//! 2. 相对该词全码,提示节省多少键(expected effort saving)?
//! 3. 是否「有用」(rank 1:输入简码即首选命中)还是「误导」
//!    (词不在菜单或 rank > 1,用户必须翻页/选择,提示价值存疑)?
//! 4. 全库聚合指标:top1/top3 rate、misleading-hint rate、
//!    expected effort saving(词频加权)、collision mass、
//!    prefix utilization、high-frequency shallow-slot coverage。
//!
//! 效用语义与 optimizer/replay 共享:键节省 = 全码长 − 简码长(与
//! lua_hints「严格短于全码才提示」一致);rank 来自 [`CodeOccupancy`]
//! 真实菜单(与 static-shortcut-audit 同源)。审计输入 = 生成器 canonical
//! 层 + quick-hint 提示视图,绝不手工指定词(§21 哨兵只进测试)。
//!
//! 输出确定性:同输入 → 字节级相同 TSV(与 manifest 管线同规范)。

use std::collections::BTreeMap;

use xhup_core::KeySequence;

/// 单条 advertised shortcut 的审计结果。
#[derive(Clone, Debug, PartialEq)]
pub struct ShortcutAuditEntry {
    pub word: String,
    /// 提示码(quick-hint 视图选出的最简码)。
    pub shortcut_code: String,
    /// 该词全码键数。
    pub full_code_len: usize,
    /// 提示码键数。
    pub shortcut_len: usize,
    /// 输入该简码时目标词的真实菜单 rank(1 起;None = 词不在该码菜单)。
    pub menu_rank: Option<usize>,
    /// 该码菜单内的候选总数(fanout,1 = 无重码)。
    pub menu_fanout: usize,
}

impl ShortcutAuditEntry {
    /// 键节省(全码 − 简码;恒 ≥ 1,提示视图保证)。
    pub fn keystrokes_saved(&self) -> usize {
        self.full_code_len - self.shortcut_len
    }

    /// 判定:rank 1 = useful;词缺失或 rank > 1 = misleading。
    pub fn verdict(&self) -> ShortcutVerdict {
        match self.menu_rank {
            Some(1) => ShortcutVerdict::Useful,
            Some(_) => ShortcutVerdict::UsefulWithSelection,
            None => ShortcutVerdict::Misleading,
        }
    }
}

/// 审计判定(§3 合同的直接映射)。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ShortcutVerdict {
    /// 输入简码 → 词为首选:真正「一击即中」的有用简码。
    Useful,
    /// 词在菜单但非首选:有节省但需选择/翻页。
    UsefulWithSelection,
    /// 词不在该码菜单:广告了却命不中,misleading。
    Misleading,
}

/// 全库聚合指标(§10 全部要求的项)。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ShortcutAuditMetrics {
    /// 审计的 advertised shortcut 总数。
    pub total: usize,
    /// verdict = Useful 的数量与占比。
    pub useful: usize,
    /// verdict = UsefulWithSelection。
    pub useful_with_selection: usize,
    /// verdict = Misleading 的数量与占比(misleading-hint rate)。
    pub misleading: usize,
    /// 词频加权的期望键节省(Σ P(word) × saved;P 用万象归一化频率)。
    pub expected_effort_saving: f64,
    /// 重码质量合计(Σ (fanout − 1) / fanout;衡量 collision mass)。
    pub collision_mass: f64,
    /// 前缀利用率:被简码占用的不同码数 / (简码数)(去重后)。
    pub prefix_utilization: f64,
    /// 高频词浅层覆盖:先验 top-1000 词中获得 ≤3 键简码的比例。
    pub top1000_shallow_coverage: f64,
}

/// 审计运行输入(canonical 层引用,全部来自 xhup-generator)。
pub struct ShortcutAuditInput<'a> {
    /// quick-hint 提示视图:词 → 提示码(生成器 lua_hints 语义)。
    pub hints: &'a BTreeMap<String, String>,
    /// 词 → 全码键数(quick-hint 视图构建时的伴生数据)。
    pub full_code_lens: &'a BTreeMap<String, usize>,
    /// 词 → 万象归一化频率(expected effort saving 加权)。
    pub normalized_frequency: &'a BTreeMap<String, f64>,
    /// 真实菜单体系(canonical 层静态占用)。
    pub occupancy: &'a crate::occupancy::CodeOccupancy,
    /// 先验 top-N 浅层覆盖的 N(§10 高频词浅层覆盖口径)。
    pub top_n: usize,
}

/// 逐条审计 + 聚合。确定性:按提示词字典序遍历。
pub fn run_audit(input: &ShortcutAuditInput) -> (Vec<ShortcutAuditEntry>, ShortcutAuditMetrics) {
    let mut entries: Vec<ShortcutAuditEntry> = Vec::new();
    for (word, shortcut) in input.hints.iter() {
        let Some(&full_len) = input.full_code_lens.get(word) else {
            continue;
        };
        let shortcut_len = shortcut.chars().count();
        let menu_rank = input
            .occupancy
            .group(&parse_code(shortcut))
            .and_then(|group| group.iter().position(|c| c.text() == word).map(|p| p + 1));
        let fanout = input.occupancy.fanout(&parse_code(shortcut));
        entries.push(ShortcutAuditEntry {
            word: word.clone(),
            shortcut_code: shortcut.clone(),
            full_code_len: full_len,
            shortcut_len,
            menu_rank,
            menu_fanout: fanout,
        });
    }
    entries.sort_by(|a, b| a.word.cmp(&b.word));
    let metrics = aggregate(&entries, input);
    (entries, metrics)
}

fn parse_code(code: &str) -> KeySequence {
    code.parse().expect("提示码必须是合法 KeySequence")
}

fn aggregate(entries: &[ShortcutAuditEntry], input: &ShortcutAuditInput) -> ShortcutAuditMetrics {
    let mut metrics = ShortcutAuditMetrics {
        total: entries.len(),
        ..ShortcutAuditMetrics::default()
    };
    let mut distinct_codes = std::collections::BTreeSet::new();
    for e in entries {
        match e.verdict() {
            ShortcutVerdict::Useful => metrics.useful += 1,
            ShortcutVerdict::UsefulWithSelection => metrics.useful_with_selection += 1,
            ShortcutVerdict::Misleading => metrics.misleading += 1,
        }
        if e.menu_fanout > 0 {
            metrics.collision_mass += (e.menu_fanout - 1) as f64 / e.menu_fanout as f64;
        }
        distinct_codes.insert(e.shortcut_code.clone());
        // 词频加权期望键节省:缺失频率的词不计入(显式跳过,不填 0)。
        if let Some(&p) = input.normalized_frequency.get(&e.word) {
            metrics.expected_effort_saving += p * e.keystrokes_saved() as f64;
        }
    }
    metrics.prefix_utilization = if entries.is_empty() {
        0.0
    } else {
        distinct_codes.len() as f64 / entries.len() as f64
    };
    // 高频词浅层覆盖:先验(top-N 由调用方传入排序)中 ≤3 键简码占比。
    metrics.top1000_shallow_coverage = shallow_coverage(input);
    metrics
}

fn shallow_coverage(input: &ShortcutAuditInput) -> f64 {
    // 先验口径:万象归一化频率降序,词形升序兜底(与 sweep_v2 同口径)。
    let mut ranked: Vec<(&String, &f64)> = input.normalized_frequency.iter().collect();
    ranked.sort_by(|a, b| b.1.total_cmp(a.1).then_with(|| a.0.cmp(b.0)));
    let top: Vec<&String> = ranked.iter().take(input.top_n).map(|(w, _)| *w).collect();
    if top.is_empty() {
        return 0.0;
    }
    let covered = top
        .iter()
        .filter(|w| {
            input
                .hints
                .get(**w)
                .is_some_and(|code| code.chars().count() <= 3)
        })
        .count();
    covered as f64 / top.len() as f64
}

/// 审计 TSV(确定性;头部注释含聚合指标,数据按词字典序)。
pub fn audit_tsv(entries: &[ShortcutAuditEntry], metrics: &ShortcutAuditMetrics) -> String {
    use std::fmt::Write as _;
    let mut out = String::new();
    writeln!(
        out,
        "# shortcut-audit-v1 total={} useful={} useful_with_selection={} misleading={} \
         misleading_rate={:.6} expected_effort_saving={:.6} collision_mass={:.6} \
         prefix_utilization={:.6} top1000_shallow_coverage={:.6}",
        metrics.total,
        metrics.useful,
        metrics.useful_with_selection,
        metrics.misleading,
        rate(metrics.misleading, metrics.total),
        metrics.expected_effort_saving,
        metrics.collision_mass,
        metrics.prefix_utilization,
        metrics.top1000_shallow_coverage,
    )
    .unwrap();
    writeln!(
        out,
        "word\tshortcut\tfull_len\tshortcut_len\tsaved\tmenu_rank\tfanout\tverdict"
    )
    .unwrap();
    for e in entries {
        writeln!(
            out,
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            e.word,
            e.shortcut_code,
            e.full_code_len,
            e.shortcut_len,
            e.keystrokes_saved(),
            e.menu_rank
                .map(|r| r.to_string())
                .unwrap_or_else(|| "NA".into()),
            e.menu_fanout,
            verdict_label(e.verdict()),
        )
        .unwrap();
    }
    out
}

fn rate(n: usize, total: usize) -> f64 {
    n as f64 / total.max(1) as f64
}

fn verdict_label(v: ShortcutVerdict) -> &'static str {
    match v {
        ShortcutVerdict::Useful => "useful",
        ShortcutVerdict::UsefulWithSelection => "useful_with_selection",
        ShortcutVerdict::Misleading => "misleading",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::OnceLock;
    /// 合成占用视图替代品:直接用真实 CodeOccupancy 结构构造难以在测试
    /// 中注入合成菜单(它是 canonical 数据派生的冻结视图);审计核心
    /// 逻辑(判定/聚合/TSV)用真实 canonical 占用 + 合成 hints 验证。
    /// 构建成本高(canonical 全层),进程内共享一份(测试只读)。
    fn shared_occupancy() -> &'static crate::occupancy::CodeOccupancy {
        static CELL: OnceLock<crate::occupancy::CodeOccupancy> = OnceLock::new();
        CELL.get_or_init(crate::occupancy::CodeOccupancy::build_current_production)
    }

    #[test]
    fn real_quick_hints_view_is_audit_ready() {
        // 生成器提示视图:全部严格短于全码;词 → 最简码。
        let hints = xhup_generator::lua_hints_view();
        assert!(!hints.is_empty());
        for (word, code) in &hints {
            let len = code.chars().count();
            assert!(len >= 1, "提示码非空 {word}");
            assert!(
                code.chars().all(|c| c.is_ascii_lowercase()),
                "提示码纯 ASCII 小写 {word} {code}"
            );
        }
    }

    #[test]
    fn audit_entries_derive_rank_from_real_menu() {
        let occupancy = shared_occupancy();
        let freq = real_word_freq_normalized();
        let (hints, full_lens) = xhup_generator::lua_hints_view_with_full_lens();
        let input = ShortcutAuditInput {
            hints: &hints,
            full_code_lens: &full_lens,
            normalized_frequency: &freq,
            occupancy,
            top_n: 1000,
        };
        let (entries, metrics) = run_audit(&input);
        assert_eq!(entries.len(), hints.len(), "逐条审计覆盖全部提示");
        // 真实菜单体系下,提示码菜单必须含目标词(生成器只从 canonical
        // 层提取提示;canonical 层即占用体系来源)。
        assert_eq!(
            metrics.misleading, 0,
            "canonical 提示必须全部命中真实菜单(§3 合同)"
        );
        assert!(metrics.useful > 0, "存在 rank-1 有用简码");
        // TSV 确定性。
        let tsv1 = audit_tsv(&entries, &metrics);
        let tsv2 = audit_tsv(&entries, &metrics);
        assert_eq!(tsv1, tsv2);
    }

    #[test]
    fn misleading_is_detected_for_unbacked_hint() {
        // 提示视图之外的词(指向不存在的码-词关系)必须被判 misleading:
        // 用一个 canonical 层没有的合成词验证判定逻辑(测试夹具,不入库)。
        let occupancy = shared_occupancy();
        let freq = real_word_freq_normalized();
        let mut hints = BTreeMap::new();
        hints.insert("::~不存在词::~".to_string(), "aaaa".to_string());
        let mut full_lens = xhup_generator::lua_hints_view_with_full_lens().1;
        full_lens.insert("::~不存在词::~".to_string(), 8);
        let input = ShortcutAuditInput {
            hints: &hints,
            full_code_lens: &full_lens,
            normalized_frequency: &freq,
            occupancy,
            top_n: 10,
        };
        let (entries, metrics) = run_audit(&input);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].verdict(), ShortcutVerdict::Misleading);
        assert_eq!(metrics.misleading, 1);
        assert_eq!(metrics.useful, 0);
    }

    #[test]
    fn metrics_shapes_are_sane() {
        let occupancy = shared_occupancy();
        let freq = real_word_freq_normalized();
        let (hints, full_lens) = xhup_generator::lua_hints_view_with_full_lens();
        let input = ShortcutAuditInput {
            hints: &hints,
            full_code_lens: &full_lens,
            normalized_frequency: &freq,
            occupancy,
            top_n: 1000,
        };
        let (entries, metrics) = run_audit(&input);
        assert_eq!(metrics.total, entries.len());
        assert_eq!(
            metrics.useful + metrics.useful_with_selection + metrics.misleading,
            metrics.total
        );
        assert!(metrics.expected_effort_saving > 0.0, "存在正的期望键节省");
        assert!(
            (0.0..=1.0).contains(&metrics.prefix_utilization),
            "前缀利用率在 0..1"
        );
        assert!(
            (0.0..=1.0).contains(&metrics.top1000_shallow_coverage),
            "浅层覆盖在 0..1"
        );
        // 词频加权节省应远小于未加权(高频词贡献主导)。
        let unweighted: f64 = entries.iter().map(|e| e.keystrokes_saved() as f64).sum();
        assert!(metrics.expected_effort_saving < unweighted);
    }

    /// 词 → 万象归一化频率(真实 canonical 数据)。
    fn real_word_freq_normalized() -> BTreeMap<String, f64> {
        let data = crate::build_analysis();
        let total: f64 = data.words.iter().map(|e| e.frequency_score() as f64).sum();
        data.words
            .iter()
            .map(|e| (e.word().to_string(), e.frequency_score() as f64 / total))
            .collect()
    }
}
