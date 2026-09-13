use std::collections::BTreeMap;
use std::fmt;
use xhup_core::KeySequence;

use crate::lattice::{CandidateKind, EdgeCandidate, Lattice, LatticeError, Span};

/// 运行时可观察的候选来源分类。
///
/// 与 [`CandidateKind`] 的区别:CandidateKind 是编译时的精确概念分类,
/// CandidateSource 是运行时从 native Rime translator 可真实区分的来源。
/// 例如 hot 和 extended 词条在同一 native table 中无法区分,统一为 `StaticTable`。
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum CandidateSource {
    /// 主静态词典 (translator namespace: `translator`).
    StaticTable,
    /// Flow 组句词典 (translator namespace: `flow`).
    FlowTable,
    /// 学习词典/用户词条 (translator namespace: `learn`, or user_table type).
    UserLearned,
    /// 组句合成候选 (sentence type from flow translator).
    SentenceComposition,
    /// 离线编译器元数据 (prefix-space compiler 提供的精确分类).
    CompilerMetadata,
}

/// 来自单一来源的候选证据。
#[derive(Clone, Eq, PartialEq)]
pub struct SourceEvidence {
    source: CandidateSource,
    /// 来源内部的质量/排名指标;语义取决于 source 类型。
    /// 零表示"无质量证据"。
    native_quality: i64,
    /// 来源在所属 translator 菜单中的位置(1-based);
    /// 零表示"未知或不适用"。
    source_rank: u32,
}

impl SourceEvidence {
    pub fn new(source: CandidateSource, native_quality: i64, source_rank: u32) -> Self {
        Self {
            source,
            native_quality,
            source_rank,
        }
    }

    pub fn source(&self) -> CandidateSource {
        self.source
    }

    pub fn native_quality(&self) -> i64 {
        self.native_quality
    }

    pub fn source_rank(&self) -> u32 {
        self.source_rank
    }
}

impl fmt::Debug for SourceEvidence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SourceEvidence")
            .field("source", &self.source)
            .field("native_quality", &self.native_quality)
            .field("source_rank", &self.source_rank)
            .finish()
    }
}

/// 融合后的候选:同一 (span, text) 对可能来自多个来源。
///
/// 保留所有证据以供后续评分器消费。Debug 输出不包含候选文本。
#[derive(Clone, Eq, PartialEq)]
pub struct FusedCandidate {
    text: Box<str>,
    evidence: Vec<SourceEvidence>,
    /// 编译器提供的精确分类(如果 compiler metadata 可用)。
    compiler_kind: Option<CandidateKind>,
    /// 所有证据中的最大频率。
    max_frequency: u64,
}

impl FusedCandidate {
    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn evidence(&self) -> &[SourceEvidence] {
        &self.evidence
    }

    pub fn compiler_kind(&self) -> Option<CandidateKind> {
        self.compiler_kind
    }

    pub fn max_frequency(&self) -> u64 {
        self.max_frequency
    }
}

impl fmt::Debug for FusedCandidate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FusedCandidate")
            .field("evidence", &self.evidence)
            .field("compiler_kind", &self.compiler_kind)
            .field("max_frequency", &self.max_frequency)
            .finish_non_exhaustive()
    }
}

/// 融合后的 lattice 边 id
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct FusedEdgeId(usize);

impl FusedEdgeId {
    pub fn index(self) -> usize {
        self.0
    }
}

/// 融合后的 lattice 边:span + 融合候选。
#[derive(Clone, Eq, PartialEq)]
pub struct FusedEdge {
    id: FusedEdgeId,
    span: Span,
    candidate: FusedCandidate,
}

impl FusedEdge {
    pub fn id(&self) -> FusedEdgeId {
        self.id
    }

    pub fn span(&self) -> Span {
        self.span
    }

    pub fn candidate(&self) -> &FusedCandidate {
        &self.candidate
    }
}

impl fmt::Debug for FusedEdge {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FusedEdge")
            .field("id", &self.id)
            .field("span", &self.span)
            .field("candidate", &self.candidate)
            .finish()
    }
}

/// Lattice 构建的结构化统计;用于诊断与性能监控。
/// 所有字段均为安全计数/度量,不含用户文本。
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LatticeBuildStats {
    pub input_key_count: usize,
    pub positions: usize,
    pub queried_spans: usize,
    pub accepted_spans: usize,
    pub raw_candidates: usize,
    pub fused_candidates: usize,
    pub edges_per_position: Vec<usize>,
    pub max_fanout: usize,
    pub total_edges: usize,
    pub truncated_spans: usize,
    pub truncated_candidates: usize,
    pub fallback_edges: usize,
}

/// 生产 lattice 构建的显式约束。
///
/// 默认值基于 XHUP 四码编码分布与 Android 实用性考量:
/// - max_key_span: 20 (10 个汉字 × 2 键音码,覆盖最长实用句子)
/// - max_candidates_per_span: 32
/// - max_outgoing_per_position: 64
/// - max_total_edges: 2048
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LatticeBounds {
    pub max_key_span: usize,
    pub max_candidates_per_span: usize,
    pub max_outgoing_per_position: usize,
    pub max_total_edges: usize,
}

impl Default for LatticeBounds {
    fn default() -> Self {
        Self {
            max_key_span: 20,
            max_candidates_per_span: 32,
            max_outgoing_per_position: 64,
            max_total_edges: 2048,
        }
    }
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
struct FusionKey {
    span: Span,
    text: Box<str>,
}

impl FusionKey {
    fn new(span: Span, text: Box<str>) -> Self {
        Self { span, text }
    }
}

pub struct LatticeFusionBuilder {
    input: KeySequence,
    bounds: LatticeBounds,
    raw_edges: Vec<(Span, Box<str>, SourceEvidence, u64)>,
}

impl LatticeFusionBuilder {
    pub fn new(input: KeySequence, bounds: LatticeBounds) -> Self {
        Self {
            input,
            bounds,
            raw_edges: Vec::new(),
        }
    }

    pub fn add_candidate(
        &mut self,
        span: Span,
        text: impl Into<Box<str>>,
        source: CandidateSource,
        native_quality: i64,
        source_rank: u32,
        frequency: u64,
    ) -> Result<(), LatticeError> {
        let text = text.into();
        if text.is_empty() {
            return Err(LatticeError::EmptyCandidateText);
        }
        if span.end() > self.input.len() {
            return Err(LatticeError::SpanOutOfBounds {
                start: span.start(),
                end: span.end(),
                input_len: self.input.len(),
            });
        }
        self.raw_edges.push((
            span,
            text,
            SourceEvidence::new(source, native_quality, source_rank),
            frequency,
        ));
        Ok(())
    }

    pub fn build(self) -> (Lattice, LatticeBuildStats) {
        let input_len = self.input.len();
        let mut stats = LatticeBuildStats {
            input_key_count: input_len,
            positions: input_len + 1,
            edges_per_position: vec![0; input_len + 1],
            raw_candidates: self.raw_edges.len(),
            ..Default::default()
        };

        let mut grouped: BTreeMap<FusionKey, Vec<(SourceEvidence, u64)>> = BTreeMap::new();
        for (span, text, evidence, freq) in self.raw_edges {
            if span.len() > self.bounds.max_key_span {
                continue;
            }
            stats.queried_spans += 1;
            let key = FusionKey::new(span, text);
            grouped.entry(key).or_default().push((evidence, freq));
        }

        let mut span_candidates: BTreeMap<Span, Vec<FusedCandidate>> = BTreeMap::new();
        for (key, evidences) in grouped {
            stats.fused_candidates += 1;
            let mut fused_evidences = Vec::with_capacity(evidences.len());
            let mut max_freq = 0;
            let mut compiler_kind = None;
            for (ev, freq) in evidences {
                max_freq = max_freq.max(freq);
                if ev.source() == CandidateSource::CompilerMetadata {
                    // 编译器元数据可携带精确分类;此处预留接口。
                    compiler_kind = None; // 未来从 evidence payload 提取
                }
                fused_evidences.push(ev);
            }
            // 确定性排序:按 source 枚举顺序(Ord derive)。
            fused_evidences.sort_by_key(|ev| ev.source());

            let fused = FusedCandidate {
                text: key.text,
                evidence: fused_evidences,
                compiler_kind,
                max_frequency: max_freq,
            };

            span_candidates.entry(key.span).or_default().push(fused);
        }
        stats.accepted_spans = span_candidates.len();

        let mut lattice = Lattice::new(self.input);

        for (span, mut candidates) in span_candidates {
            if candidates.len() > self.bounds.max_candidates_per_span {
                stats.truncated_spans += 1;
                stats.truncated_candidates +=
                    candidates.len() - self.bounds.max_candidates_per_span;

                candidates.sort_by_key(|c| {
                    let is_fallback = c.text.chars().count() == 1;
                    let priority = if is_fallback { 0 } else { 1 };
                    (priority, std::cmp::Reverse(c.max_frequency))
                });

                candidates.truncate(self.bounds.max_candidates_per_span);
            }

            if stats.edges_per_position[span.start()] + candidates.len()
                > self.bounds.max_outgoing_per_position
            {
                let allowed = self
                    .bounds
                    .max_outgoing_per_position
                    .saturating_sub(stats.edges_per_position[span.start()]);
                stats.truncated_candidates += candidates.len() - allowed;
                candidates.truncate(allowed);
            }

            for fused in candidates {
                if stats.total_edges >= self.bounds.max_total_edges {
                    break;
                }

                if fused.text.chars().count() == 1 {
                    stats.fallback_edges += 1;
                }

                let legacy_kind = fused.compiler_kind.unwrap_or(CandidateKind::ExtendedWord);
                let legacy_candidate =
                    EdgeCandidate::new(fused.text, legacy_kind, fused.max_frequency).unwrap();

                if lattice.add_edge(span, legacy_candidate).is_ok() {
                    stats.total_edges += 1;
                    stats.edges_per_position[span.start()] += 1;
                }
            }
        }

        stats.max_fanout = stats.edges_per_position.iter().copied().max().unwrap_or(0);

        (lattice, stats)
    }
}
