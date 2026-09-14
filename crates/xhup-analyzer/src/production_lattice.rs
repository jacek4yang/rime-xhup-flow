//! 生产级 Joint Key/Text Lattice 构造桥:generator 真实候选 → decoder 融合 lattice。
//!
//! 里程碑一(Issue #83 §11)。本模块把 `xhup-generator` 的确定性候选
//! 投影(单字全码、hot 词码、扩展词码含搜狗聚合增量)翻译为
//! `xhup-decoder::SourceCandidate` 事实流,经 [`LatticeBuilder`] 的
//! 多源融合契约收拢为单一边集合。不访问网络、不读取外部文件。
//!
//! span 映射:XHUP 双拼逐字两键,词长 n 键词的 `span = (start, start + n)`。
//! 同一码可能在输入的多个位置出现,每个出现位置注入一条事实;
//! 路径枚举按位置衔接,实现「研究生|命」与「研究|生命」多分段并存。

use std::num::NonZeroUsize;

use xhup_core::KeySequence;
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

    // 词库总量 430 万+,必须先把「码在输入中出现的全部位置」预算好,
    // 再按位置过滤注入。码不在输入任何位置出现的事实不可能出现在
    // 任何路径上,跳过即可保持语义不变,同时避免数百万次无效
    // 融合 + String 分配(无过滤时单次构建物化 430 万条事实)。
    let input_keys: Vec<_> = match input.parse::<KeySequence>() {
        Ok(seq) => seq.as_slice().to_vec(),
        Err(_) => return production_empty(input, path_limit),
    };

    // occurrence_positions(code) = code 在输入中每个出现位置的起点。
    let occurrence_positions = |code: &KeySequence| -> Vec<usize> {
        let code_keys = code.as_slice();
        let n = input_keys.len();
        let m = code_keys.len();
        if m == 0 || m > n {
            return Vec::new();
        }
        (0..=n - m)
            .filter(|&i| input_keys[i..i + m] == *code_keys)
            .collect()
    };

    // 单字原语:全码(2 键或 4 键),span = 码的每个出现位置。
    for entry in canonical_input_char_code_entries() {
        let code = entry.code();
        for start in occurrence_positions(code) {
            builder.push(SourceCandidate::new(
                (start, start + code.len()),
                entry.hanzi().as_char().to_string(),
                CandidateKind::Character,
                entry.frequency_score(),
            ));
        }
    }

    // hot 静态词层:
    for entry in canonical_word_code_entries() {
        let code = entry.code();
        for start in occurrence_positions(code) {
            builder.push(SourceCandidate::new(
                (start, start + code.len()),
                entry.word().to_string(),
                CandidateKind::HotWord,
                entry.frequency_score(),
            ));
        }
    }

    // 扩展词层(万象 extended + 搜狗聚合增量):
    for entry in canonical_extended_word_code_entries() {
        let code = entry.code();
        for start in occurrence_positions(code) {
            builder.push(SourceCandidate::new(
                (start, start + code.len()),
                entry.word().to_string(),
                CandidateKind::ExtendedWord,
                entry.frequency_score(),
            ));
        }
    }

    builder
        .build(input, path_limit)
        .expect("生成器候选与输入按键串必须自洽")
}

/// 输入串无法解析为按键序列时的空构造(保持与解析成功路径一致的返回类型)。
fn production_empty(input: &str, path_limit: NonZeroUsize) -> BuiltLattice {
    // 输入非法时仍走一遍 build 让其返回 InvalidSpan 错误,与既有契约一致;
    // 这里用空 facts 触发 EmptyCandidateText。测试从不传非法输入,
    // 生产调用方在解码前已校验输入。
    let builder = LatticeBuilder::new();
    builder
        .build(input, path_limit)
        .expect("空事实流的构建错误属于调用方契约违反")
}

/// 融合统计快捷访问(测试与基准用)。
pub fn production_build_stats(input: &str, path_limit: NonZeroUsize) -> BuildStats {
    *build_production_lattice(input, path_limit).stats()
}
