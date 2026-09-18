//! 跨切分(码级)评测通路(Issue #83 §20/§24,§25 第 6 步收益验收)。
//!
//! # 为什么需要这条通路
//!
//! [`crate::context_replay`] 度量的是**同码词消歧**(候选菜单 = 同一 canonical
//! 词码下的词),token 已由分词器给定,不存在「按码切分」的选择。而第二转移
//! 证据源(如 PTT)补充的是**码级/跨切分**歧义证据 —— 两者是**不同的总体**。
//! 实测表明用 `context_replay` 度量第二证据源的收益会得到 ≈0(甚至轻微负值),
//! 因此必须有这条通路才能回答「第二证据源是否真的有用」。
//!
//! # 两个已实测的工程约束(本模块的设计依据)
//!
//! 1. **路径爆炸**:真实句构造出的生产 lattice 几乎必然超过
//!    [`Lattice::complete_paths`] 的参考枚举上限(实测 14/14 句撞 64 路径上限)。
//!    因此本模块使用 [`decode_beam_adaptive`] —— 它在不截断时与穷举排序
//!    **逐路径一致**,在仍截断时显式报告(`truncated_at_max`),不会把截断
//!    悄悄当成正确答案。
//! 2. **期望分段未必可达**:分词器与 lattice 构造器口径不同,语料的真实分段
//!    不一定作为完整路径存在于 lattice 中(实测仅 1/14 句存在)。本模块把
//!    「期望分段不在 lattice 中」显式计入 [`CrossSegmentationMetrics::skipped_expected_path_absent`]
//!    并**排除出分母**,避免把「不可达」误计为「排序错误」。
//!
//! # 语义边界
//!
//! - 候选检索使用生产桥 [`crate::production_lattice`](真实 generator 数据);
//! - 输入按键串由语料分词的各 token canonical 词码**拼接**得到,即「用户完整
//!   输入该句」的按键序列;
//! - 期望分段 = 语料分词结果本身,不引入人工语言学判断;
//! - 全离线、确定性:有序容器 + 固定点整数评分。

use std::num::NonZeroUsize;

use serde::Serialize;
use xhup_core::KeySequence;
use xhup_decoder::{
    BaselineScorer, DecodeConfig, DeterministicScorer, Lattice, RuntimeContext,
    decode_beam_adaptive,
};

use crate::corpus::Segmenter;
use crate::production_lattice::build_production_lattice;

/// 报告 schema 标识。
pub const CROSS_SEGMENTATION_SCHEMA: &str = "xhup-cross-segmentation/v1";

/// 单句输入的长度上限(按键数)。超过则跳过并计数,避免超长输入拖垮评测。
pub const MAX_INPUT_KEYS: usize = 8;

/// 有界解码的起始 beam 宽度。
const BEAM_FLOOR: usize = 8;

/// 有界解码的自适应上限(与 `xhup_decoder::DEFAULT_MAX_BEAM_WIDTH` 同值)。
const BEAM_CEILING: usize = 32;

/// 参考路径枚举上限(`production_lattice` 构建时使用)。
const PATH_LIMIT: NonZeroUsize = NonZeroUsize::new(64).expect("64 != 0");

/// 跨切分评测的显式排除分类与有效性计数。
///
/// 排除分类必须显式记录:分母只含**有效样本**,否则「不可达」会被误计为
/// 「排序错误」,指标失去意义。
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CrossSegmentationMetrics {
    /// 语料中参与尝试的句子数(非空行)。
    pub sentences: usize,
    /// 因某个 token 无 canonical 词码而跳过。
    pub skipped_no_word_code: usize,
    /// 因拼接后按键串过长/为空而跳过。
    pub skipped_input_bounds: usize,
    /// 因期望分段不在 lattice 中而跳过(可复现性判据)。
    pub skipped_expected_path_absent: usize,
    /// **有效样本数**(= 分母)。
    pub evaluated: usize,
    /// 有效样本中 top1 **文本**等于语料真实文本的句子数(产品口径)。
    ///
    /// 这是最主要的指标:用户要的是「打出的字对不对」,而不是「内部切分是否
    /// 与语料分词器逐 span 相同」。同一文本可由多种合法切分产生(如
    /// `["好的"]` 与 `["好","的"]`),按 span 严格比较会把正确结果误判为错误。
    pub top1_text_correct: usize,
    /// 有效样本中 top1 切分与语料分词**逐 span 相同**的句子数(严格口径)。
    pub top1_span_exact: usize,
    /// 有效样本中期望文本落在 top-k 内(按文本比较)。
    pub in_top_k: usize,
    /// 有界解码顶到上限仍截断的有效样本数(诚实报告,不冒充等价)。
    pub truncated_at_max: usize,
    /// 以「已提交前文 + 当前窗口」形式评测的窗口数(仅开启 committed
    /// context 且句子有多个 token 时计数;这些句子不再走单句路径)。
    pub contextual_windows: usize,
}

impl CrossSegmentationMetrics {
    /// top1 文本正确率(产品口径;分母为有效样本)。
    pub fn top1_text_rate(&self) -> f64 {
        ratio(self.top1_text_correct, self.evaluated)
    }

    /// top1 逐 span 严格一致的比率(诊断口径;通常低于文本正确率)。
    pub fn top1_span_exact_rate(&self) -> f64 {
        ratio(self.top1_span_exact, self.evaluated)
    }

    /// top-k 命中率(分母为有效样本)。
    pub fn top_k_rate(&self) -> f64 {
        ratio(self.in_top_k, self.evaluated)
    }

    /// 被排除的句子占比(诊断数据可用性;分母为尝试过的句子)。
    pub fn exclusion_rate(&self) -> f64 {
        let excluded = self.skipped_no_word_code
            + self.skipped_input_bounds
            + self.skipped_expected_path_absent;
        ratio(excluded, self.sentences)
    }
}

/// 一次跨切分评测的完整报告。
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CrossSegmentationReport {
    pub schema: &'static str,
    pub scorer: &'static str,
    pub top_k: usize,
    pub beam_floor: usize,
    pub beam_ceiling: usize,
    /// 本次评测是否使用 committed context(在 token 边界切出前文窗口)。
    pub with_committed_context: bool,
    pub metrics: CrossSegmentationMetrics,
}

/// 语料分词结果对应的按键 span `(start, end)` 与文本。
type ExpectedSegment = (usize, usize, String);

/// 用真实语料跑一次跨切分评测。
///
/// `scorer` 为任意确定性 scorer(例如 baseline 或 KDConv bigram);`scorer_id`
/// 写入报告便于溯源。语料为每行一句的纯文本。
pub fn evaluate_cross_segmentation<S: DeterministicScorer>(
    sentences_text: &str,
    scorer: &S,
    scorer_id: &'static str,
    top_k: usize,
    with_committed_context: bool,
) -> CrossSegmentationReport {
    let menus = WordCodeMenus::build();
    let segmenter = Segmenter::build();
    let mut metrics = CrossSegmentationMetrics::default();

    for line in sentences_text.lines() {
        let sentence = line.trim();
        if sentence.is_empty() {
            continue;
        }
        metrics.sentences += 1;

        let tokens = segmenter.segment(sentence);
        // (1) 每个 token 都要有 canonical 词码,否则无法拼出按键串。
        let mut codes = Vec::with_capacity(tokens.len());
        let mut missing = false;
        for token in &tokens {
            match menus.code_of(token) {
                Some(code) => codes.push(code.to_string()),
                None => {
                    missing = true;
                    break;
                }
            }
        }
        if missing {
            metrics.skipped_no_word_code += 1;
            continue;
        }

        // 在多 token 句子上按 token 边界切出「前文 + 当前输入」的窗口:
        // 这是用户真实输入节奏的近似(having committed 前面几个字之后,
        // 再输入下一段)。单 token 句子退化为空上下文(仍被计数)。
        if codes.len() > 1 && with_committed_context {
            let split = codes.len() - 1;
            let committed_text: String = tokens[..split].concat();
            let committed_codes: Vec<String> = codes[..split].to_vec();
            let tail_tokens = &tokens[split..];
            let tail_codes = &codes[split..];
            metrics.contextual_windows += 1;
            evaluate_window(
                &menus,
                &committed_text,
                &committed_codes,
                tail_tokens,
                tail_codes,
                scorer,
                top_k,
                &mut metrics,
            );
            continue;
        }

        // (2) 按键串长度边界。
        let input: String = codes.concat();
        if input.is_empty() || input.len() > MAX_INPUT_KEYS {
            metrics.skipped_input_bounds += 1;
            continue;
        }

        // (3) 期望分段 = 语料分词结果的 (span, text) 序列。
        let mut expected: Vec<ExpectedSegment> = Vec::with_capacity(tokens.len());
        let mut position = 0usize;
        for (token, code) in tokens.iter().zip(codes.iter()) {
            expected.push((position, position + code.len(), token.clone()));
            position += code.len();
        }

        let expected_text: String = tokens.join("");

        // (4) 生产 lattice(真实 generator 候选)。
        let built = build_production_lattice(&input, PATH_LIMIT);
        let lattice = built.lattice();
        // 可复现性判据:期望分段必须作为一条完整路径存在。
        if !path_exists(lattice, &expected) {
            metrics.skipped_expected_path_absent += 1;
            continue;
        }

        // (5) 有界解码(自适应扩宽;仍截断时诚实计数)。
        let Ok(composition) = input.parse::<KeySequence>() else {
            metrics.skipped_input_bounds += 1;
            continue;
        };
        let context = RuntimeContext::new("", composition);
        score_lattice(
            &context,
            lattice,
            expected_text,
            &expected,
            scorer,
            top_k,
            &mut metrics,
        );
    }

    CrossSegmentationReport {
        schema: CROSS_SEGMENTATION_SCHEMA,
        scorer: scorer_id,
        top_k,
        beam_floor: BEAM_FLOOR,
        beam_ceiling: BEAM_CEILING,
        with_committed_context,
        metrics,
    }
}

/// 对一段「已提交前文 + 当前输入」窗口构造 lattice 并评分。
///
/// 前文的按键只用于构造 [`RuntimeContext`] 的 `committed_left`(提供转移证据),
/// 当前窗口才是 lattice 的输入。期望分段按当前窗口的 token 计算。
#[allow(clippy::too_many_arguments)]
fn evaluate_window<S: DeterministicScorer>(
    menus: &WordCodeMenus,
    committed_text: &str,
    committed_codes: &[String],
    tail_tokens: &[String],
    tail_codes: &[String],
    scorer: &S,
    top_k: usize,
    metrics: &mut CrossSegmentationMetrics,
) {
    // 窗口按键串边界(前文按键数不影响窗口,但窗口本身仍受上限约束)。
    let _ = (menus, committed_codes);
    let input: String = tail_codes.concat();
    if input.is_empty() || input.len() > MAX_INPUT_KEYS {
        metrics.skipped_input_bounds += 1;
        return;
    }

    let mut expected: Vec<ExpectedSegment> = Vec::with_capacity(tail_tokens.len());
    let mut position = 0usize;
    for (token, code) in tail_tokens.iter().zip(tail_codes.iter()) {
        expected.push((position, position + code.len(), token.clone()));
        position += code.len();
    }
    let expected_text: String = tail_tokens.concat();

    let built = build_production_lattice(&input, PATH_LIMIT);
    let lattice = built.lattice();
    if !path_exists(lattice, &expected) {
        metrics.skipped_expected_path_absent += 1;
        return;
    }
    let Ok(composition) = input.parse::<KeySequence>() else {
        metrics.skipped_input_bounds += 1;
        return;
    };
    let context = RuntimeContext::new(committed_text, composition);
    score_lattice(
        &context,
        lattice,
        expected_text,
        &expected,
        scorer,
        top_k,
        metrics,
    );
}

/// 有界解码 + 三种口径计数(单句与窗口两条路径共用)。
fn score_lattice<S: DeterministicScorer>(
    context: &RuntimeContext,
    lattice: &Lattice,
    expected_text: String,
    expected: &[ExpectedSegment],
    scorer: &S,
    top_k: usize,
    metrics: &mut CrossSegmentationMetrics,
) {
    let config = DecodeConfig::new(
        NonZeroUsize::new(BEAM_FLOOR).expect("beam floor != 0"),
        NonZeroUsize::new(top_k.max(1)).expect("top_k != 0"),
        0,
    );
    let (decoded, outcome) = decode_beam_adaptive(
        lattice,
        context,
        scorer,
        config,
        NonZeroUsize::new(BEAM_CEILING).expect("beam ceiling != 0"),
    );
    metrics.evaluated += 1;
    if outcome.truncated_at_max {
        metrics.truncated_at_max += 1;
    }
    let ranked = decoded.ranked();
    // 产品口径:top1 文本是否正确。
    if ranked
        .first()
        .is_some_and(|path| path.text() == expected_text)
    {
        metrics.top1_text_correct += 1;
    }
    // 严格口径:top1 切分是否与语料分词逐 span 相同。
    if ranked
        .first()
        .is_some_and(|path| path_matches(lattice, path.path().edge_ids(), expected))
    {
        metrics.top1_span_exact += 1;
    }
    if ranked
        .iter()
        .take(top_k)
        .any(|path| path.text() == expected_text)
    {
        metrics.in_top_k += 1;
    }
}

/// 便捷入口:用 baseline(忽略上下文)跑跨切分评测,作为上下文 scorer 的对照。
pub fn evaluate_cross_segmentation_baseline(
    sentences_text: &str,
    top_k: usize,
) -> CrossSegmentationReport {
    evaluate_cross_segmentation(
        sentences_text,
        &BaselineScorer::default(),
        BaselineScorer::SCORER_ID,
        top_k,
        false,
    )
}

/// 语料分词量级下的词 → 码查表(按进程构建一次)。
struct WordCodeMenus {
    word_code: std::collections::BTreeMap<String, String>,
}

impl WordCodeMenus {
    fn build() -> Self {
        let mut word_code = std::collections::BTreeMap::new();
        for entry in xhup_generator::canonical_word_code_entries() {
            // 同一词多码时保留首条(数据层有序,确定性)。
            word_code
                .entry(entry.word().to_string())
                .or_insert_with(|| entry.code().to_string());
        }
        Self { word_code }
    }

    fn code_of(&self, word: &str) -> Option<&str> {
        self.word_code.get(word).map(String::as_str)
    }
}

/// 给定边序列是否精确匹配期望分段(按 (span, text) 比较,不依赖 EdgeId)。
fn path_matches(
    lattice: &Lattice,
    edge_ids: &[xhup_decoder::EdgeId],
    expected: &[ExpectedSegment],
) -> bool {
    if edge_ids.len() != expected.len() {
        return false;
    }
    edge_ids
        .iter()
        .zip(expected.iter())
        .all(|(&id, (start, end, text))| {
            lattice.edge(id).is_some_and(|edge| {
                edge.span().start() == *start
                    && edge.span().end() == *end
                    && edge.candidate().text() == text
            })
        })
}

/// lattice 中是否存在一条完整路径恰好等于期望分段。
fn path_exists(lattice: &Lattice, expected: &[ExpectedSegment]) -> bool {
    // 按位置递推:可达位置集合 + 「该位置由哪个期望段覆盖」。
    // 期望分段是连续覆盖,故只需检查每个期望段是否恰有一条对应边。
    expected.iter().all(|(start, end, text)| {
        lattice.outgoing(*start).unwrap_or(&[]).iter().any(|&id| {
            lattice.edge(id).is_some_and(|edge| {
                edge.span().start() == *start
                    && edge.span().end() == *end
                    && edge.candidate().text() == text
            })
        })
    })
}

fn ratio(numerator: usize, denominator: usize) -> f64 {
    if denominator == 0 {
        return 0.0;
    }
    numerator as f64 / denominator as f64
}
