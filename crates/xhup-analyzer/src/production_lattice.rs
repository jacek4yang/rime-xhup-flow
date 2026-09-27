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
use xhup_decoder::{
    BuildStats, BuiltLattice, CandidateKind, LatticeBuilder, LatticeError, SourceCandidate,
};
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

    match builder.build(input, path_limit) {
        Ok(built) => built,
        // #150/#151:逐键 prefix-walk 下,单键前缀(如 `j`)没有任何码
        // 事实(最短码 2 键),build 返回 EmptyCandidateText —— 转为
        // 零边 lattice(空菜单但可用),不得 panic。
        Err(LatticeError::EmptyCandidateText) => production_empty(input, path_limit),
        Err(error) => panic!("生成器候选与输入按键串必须自洽: {error:?}"),
    }
}

/// 输入串可解析但没有任何码事实时的空构造(#150/#151 逐键 prefix-walk)。
///
/// 首个按键尚无任何码事实(最短码 2 键)时返回**零边 lattice** 而非
/// 错误 —— 空 lattice 的完整路径空间为空,排序输出为空菜单,语义 =
/// 「该前缀尚无可用候选」,与「崩溃/拒绝构建」有本质区别;下一个按键
/// 到达后事实流非空,恢复正常构建。
fn production_empty(input: &str, path_limit: NonZeroUsize) -> BuiltLattice {
    let composition: KeySequence = input
        .parse()
        .expect("空事实流仅发生在输入可解析但无码事实的场景");
    let lattice = xhup_decoder::Lattice::new(composition);
    BuiltLattice::from_parts(lattice, path_limit)
}

/// 融合统计快捷访问(测试与基准用)。
pub fn production_build_stats(input: &str, path_limit: NonZeroUsize) -> BuildStats {
    *build_production_lattice(input, path_limit).stats()
}
