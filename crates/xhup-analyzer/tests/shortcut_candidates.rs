//! 候选枚举测试:时间哨兵(P0)、候选合法性、确定性。

use std::collections::BTreeSet;

use xhup_analyzer::candidates::{Mode, enumerate_targets};
use xhup_core::KeySequence;
use xhup_generator::word_code_analysis_entries;

/// 从完整码与模式重投影,验证候选码确实只由 F/I 投影构成。
fn reproject(full_code: &KeySequence, modes: &[Mode]) -> Vec<xhup_core::Key> {
    let mut keys = Vec::new();
    let (chunks, _) = full_code.as_slice().as_chunks::<2>();
    for (index, chunk) in chunks.iter().enumerate() {
        match modes[index] {
            Mode::Full => keys.extend_from_slice(chunk),
            Mode::Initial => keys.push(chunk[0]),
        }
    }
    keys
}

#[test]
fn time_sentinel_candidates_are_exactly_fi_and_if() {
    // P0:「时间」(时 ui + 间 jm,full = uijm)的候选必须恰好是
    // uij(FI)与 ujm(IF)——FF 是完整码,II(uj)长度 2 保留给单字。
    let words = word_code_analysis_entries();
    let (targets, _) = enumerate_targets(&words);
    let target = targets
        .iter()
        .find(|t| t.word() == "时间")
        .expect("时间必然在 production 词表中");
    assert_eq!(target.full_code().to_string(), "uijm");
    let candidates: BTreeSet<(String, String)> = target
        .candidates()
        .iter()
        .map(|c| (c.shortcut_code().to_string(), c.mode().pattern()))
        .collect();
    assert_eq!(
        candidates,
        BTreeSet::from([
            ("uij".to_string(), "FI".to_string()),
            ("ujm".to_string(), "IF".to_string())
        ]),
        "时间的 shortcut 候选必须恰好是 uij(FI) 与 ujm(IF)"
    );
}

#[test]
fn every_candidate_is_a_legal_fi_projection() {
    let words = word_code_analysis_entries();
    let (targets, stats) = enumerate_targets(&words);
    assert!(stats.actual > 0);
    let mut candidate_count = 0usize;
    for target in &targets {
        let char_count = target.word().chars().count();
        let full_len = target.full_code().len();
        assert_eq!(full_len, char_count * 2, "词码长度应为字数两倍");
        let mut seen_codes: BTreeSet<&KeySequence> = BTreeSet::new();
        for candidate in target.candidates() {
            candidate_count += 1;
            let code = candidate.shortcut_code();
            // 长度:≥ 3 且严格小于完整码
            assert!(code.len() >= 3, "{} {} 长度 < 3", target.word(), code);
            assert!(
                code.len() < full_len,
                "{} {} 不短于完整码",
                target.word(),
                code
            );
            // 只含小写 a-z
            assert!(
                code.to_string().chars().all(|c| c.is_ascii_lowercase()),
                "{} 含非小写字母",
                code
            );
            // 模式长度 = 字数,且码与模式逐字 F/I 重投影一致(保持原字序)
            let modes = candidate.mode().modes();
            assert_eq!(modes.len(), char_count);
            assert_eq!(
                reproject(target.full_code(), modes),
                code.as_slice(),
                "{} {} 与模式 {} 的重投影不一致",
                target.word(),
                code,
                candidate.mode().pattern()
            );
            // (词, 码) 去重
            assert!(seen_codes.insert(code), "{} 内码重复", target.word());
        }
        // 每词候选数上界:2 字 2 个、3 字 7 个、4 字 15 个
        let upper = match char_count {
            2 => 2,
            3 => 7,
            4 => 15,
            other => panic!("词字数应为 2/3/4: {other}"),
        };
        assert!(target.candidates().len() <= upper);
    }
    assert_eq!(candidate_count, stats.actual);
    assert!(stats.actual <= stats.theoretical);
}

#[test]
fn enumeration_is_deterministic() {
    let words = word_code_analysis_entries();
    let (targets_a, stats_a) = enumerate_targets(&words);
    let (targets_b, stats_b) = enumerate_targets(&words);
    assert_eq!(stats_a.actual, stats_b.actual);
    assert_eq!(stats_a.theoretical, stats_b.theoretical);
    assert_eq!(targets_a.len(), targets_b.len());
    for (a, b) in targets_a.iter().zip(targets_b.iter()) {
        assert_eq!(a.word(), b.word());
        assert_eq!(a.full_code(), b.full_code());
        assert_eq!(a.candidates().len(), b.candidates().len());
        for (ca, cb) in a.candidates().iter().zip(b.candidates().iter()) {
            assert_eq!(ca.shortcut_code(), cb.shortcut_code());
            assert_eq!(ca.mode().pattern(), cb.mode().pattern());
        }
    }
}
