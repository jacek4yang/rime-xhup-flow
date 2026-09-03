//! PR #22 ZERO_REGRESSION canonical 层的候选语法审计(只读,不修改数据)。
//!
//! `word_zero_regression.tsv` 由冻结的 legacy-any-fi-v1 语法生成,其中
//! 包含大量非单调模式(如 `IF`/`IFI`/`IIF`);新 production policy
//! (monotone-suffix-initials-v2)不再生成这些形式。本审计把它们按模式
//! 分类计数,作为技术债文档:这些是 legacy-v1 冻结映射,保持支持,
//! 不是无效数据。
//!
//! 此处不引入 analyzer 依赖;单调性判定与 analyzer 的
//! `CandidateGrammar::MonotoneSuffixInitialsV2` 是同一小不变式的独立
//! 实现(generator 不依赖 analyzer)。

use std::collections::BTreeMap;

use xhup_generator::canonical_word_shortcut_entries;

/// 模式是否为单调后缀缩写 `F* I*` 且至少一个 I。
fn is_monotone_suffix(mode: &str) -> bool {
    let mut seen_initial = false;
    for c in mode.chars() {
        match c {
            'F' => {
                if seen_initial {
                    return false;
                }
            }
            'I' => seen_initial = true,
            _ => return false,
        }
    }
    seen_initial
}

/// PR #22 冻结层模式审计:总数、单调行数、非单调行数(按模式分类)。
#[test]
fn zero_regression_pattern_audit() {
    let entries = canonical_word_shortcut_entries();
    assert_eq!(entries.len(), 44_448, "PR #22 冻结行数");

    let mut monotone = 0usize;
    let mut non_monotone: BTreeMap<String, usize> = BTreeMap::new();
    for entry in entries {
        let mode = entry.mode();
        assert!(
            mode.chars().all(|c| matches!(c, 'F' | 'I')),
            "冻结层模式只含 F/I: {mode}"
        );
        if is_monotone_suffix(mode) {
            monotone += 1;
        } else {
            *non_monotone.entry(mode.to_string()).or_default() += 1;
        }
    }
    let non_monotone_total: usize = non_monotone.values().sum();
    assert_eq!(monotone + non_monotone_total, entries.len());

    // 技术债文档锚点(数据变化时人工 review):冻结 legacy-v1 层的
    // 非单调模式分布。写入断言防止无意识漂移;这是只读审计,不是数据
    // 修改计划 —— 非单调映射无限期保持支持。
    eprintln!("PR #22 legacy-v1 冻结层模式审计:");
    eprintln!("  总行数:          {}", entries.len());
    eprintln!("  单调(F* I*):     {monotone}");
    eprintln!("  非单调(冻结):    {non_monotone_total}");
    let mut rows: Vec<_> = non_monotone.into_iter().collect();
    rows.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    for (mode, count) in &rows {
        eprintln!("    {mode}: {count}");
    }
    assert_eq!(monotone, 31_167, "单调行数锚点");
    assert_eq!(non_monotone_total, 13_281, "非单调行数锚点");
    // 按模式分类(降序)。
    let expected: Vec<(&str, usize)> = vec![
        ("IIF", 4_632),
        ("IFI", 3_625),
        ("IF", 2_981),
        ("IFF", 1_538),
        ("IIIF", 446),
        ("FIF", 40),
        ("IFII", 14),
        ("IIFI", 5),
    ];
    let actual: Vec<(String, usize)> = rows;
    assert_eq!(
        actual,
        expected
            .into_iter()
            .map(|(m, c)| (m.to_string(), c))
            .collect::<Vec<_>>(),
        "非单调模式分布锚点"
    );
}
