//! 生产级 Joint Key/Text Lattice 构造桥:generator 真实候选 → decoder 融合 lattice。
//!
//! 里程碑一(Issue #83 §11)。本模块把 `xhup-generator` 的确定性候选
//! 投影(单字全码、hot 词码、扩展词码含搜狗聚合增量)翻译为
//! `xhup-decoder::SourceCandidate` 事实流,经 [`LatticeBuilder`] 的
//! 多源融合契约收拢为单一边集合。不访问网络、不读取外部文件。
//!
//! span 映射:XHUP 双拼逐字两键,`span = (0, 2 * 词字数)`,起始恒为 0
//! (composition 从头输入;跨词拼接由 decoder 的路径枚举承担)。

use std::num::NonZeroUsize;

use xhup_decoder::{BuildStats, BuiltLattice, CandidateKind, LatticeBuilder, SourceCandidate};
use xhup_generator::{
    canonical_extended_word_code_entries, canonical_input_char_code_entries,
    canonical_word_code_entries,
};

/// 从生成器真实候选构建指定输入串的融合 lattice。
///
/// 来源映射:
/// - 单字全码 → `Character`(携带万象单字频率分数);
/// - hot 词码(4/6/8 键) → `HotWord`(hot 聚合分数);
/// - extended/搜狗词码 → `ExtendedWord`(聚合分数;搜狗增量为 1)。
pub fn build_production_lattice(input: &str, path_limit: NonZeroUsize) -> BuiltLattice {
    let mut builder = LatticeBuilder::new();

    // 单字原语:全码(2 键或 4 键)逐条注入;span 覆盖整条码。
    for entry in canonical_input_char_code_entries() {
        let code = entry.code();
        let len = code.len();
        builder.push(SourceCandidate::new(
            (0, len),
            entry.hanzi().as_char().to_string(),
            CandidateKind::Character,
            entry.frequency_score(),
        ));
    }

    // hot 静态词层:
    for entry in canonical_word_code_entries() {
        let code = entry.code();
        let len = code.len();
        builder.push(SourceCandidate::new(
            (0, len),
            entry.word().to_string(),
            CandidateKind::HotWord,
            entry.frequency_score(),
        ));
    }

    // 扩展词层(万象 extended + 搜狗聚合增量):
    for entry in canonical_extended_word_code_entries() {
        let code = entry.code();
        let len = code.len();
        builder.push(SourceCandidate::new(
            (0, len),
            entry.word().to_string(),
            CandidateKind::ExtendedWord,
            entry.frequency_score(),
        ));
    }

    builder
        .build(input, path_limit)
        .expect("生成器候选与输入按键串必须自洽")
}

/// 融合统计快捷访问(测试与基准用)。
pub fn production_build_stats(input: &str, path_limit: NonZeroUsize) -> BuildStats {
    *build_production_lattice(input, path_limit).stats()
}
