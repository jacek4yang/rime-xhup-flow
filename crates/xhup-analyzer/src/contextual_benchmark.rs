//! 版本化上下文联合解码 benchmark 格式、严格解析与 baseline runner。
//!
//! fixture 显式携带 lattice edges，使里程碑一可以在尚未接入生产候选检索前
//! 比较多分段路径。词频来自已固定数据，左上下文进入 [`RuntimeContext`]；当前
//! baseline scorer 刻意忽略上下文，供后续 n-gram scorer 做可量化 A/B。

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::num::NonZeroUsize;
use std::time::Instant;

use serde::{Deserialize, Serialize};
use xhup_core::KeySequence;
use xhup_decoder::{
    BaselineScorer, CandidateKind, EdgeCandidate, EdgeId, Lattice, RuntimeContext, Span, rank_paths,
};

pub const BENCHMARK_SCHEMA: &str = "xhup-contextual-benchmark/v1";
pub const BASELINE_SCHEMA: &str = "xhup-contextual-baseline/v1";
pub const REPORT_SCHEMA: &str = "xhup-contextual-report/v1";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BenchmarkSplit {
    Development,
    Evaluation,
}

/// 一条已校验 benchmark case。
pub struct BenchmarkCase {
    id: String,
    split: BenchmarkSplit,
    source: String,
    tags: Box<[String]>,
    context: RuntimeContext,
    lattice: Lattice,
    expected_edge_ids: Box<[EdgeId]>,
    expected_signature: String,
    expected_text: String,
}

impl BenchmarkCase {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn split(&self) -> BenchmarkSplit {
        self.split
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn tags(&self) -> &[String] {
        &self.tags
    }

    pub fn context(&self) -> &RuntimeContext {
        &self.context
    }

    pub fn lattice(&self) -> &Lattice {
        &self.lattice
    }

    pub fn expected_text(&self) -> &str {
        &self.expected_text
    }

    pub fn expected_edge_ids(&self) -> &[EdgeId] {
        &self.expected_edge_ids
    }
}

/// 一个版本化 benchmark 文档。
pub struct BenchmarkSuite {
    top_k: usize,
    max_paths_per_case: NonZeroUsize,
    cases: Vec<BenchmarkCase>,
}

impl BenchmarkSuite {
    pub fn from_json(text: &str) -> Result<Self, BenchmarkError> {
        let raw: RawSuite = serde_json::from_str(text)
            .map_err(|error| BenchmarkError(format!("benchmark JSON 非法: {error}")))?;
        if raw.schema != BENCHMARK_SCHEMA {
            return Err(BenchmarkError(format!(
                "benchmark schema 应为 {BENCHMARK_SCHEMA:?}，实际为 {:?}",
                raw.schema
            )));
        }
        if raw.top_k == 0 {
            return Err(BenchmarkError("topK 必须大于 0".to_string()));
        }
        let max_paths_per_case = NonZeroUsize::new(raw.max_paths_per_case)
            .ok_or_else(|| BenchmarkError("maxPathsPerCase 必须大于 0".to_string()))?;
        if raw.cases.is_empty() {
            return Err(BenchmarkError("benchmark cases 不能为空".to_string()));
        }

        let mut ids = BTreeSet::new();
        let mut cases = Vec::with_capacity(raw.cases.len());
        for raw_case in raw.cases {
            if raw_case.id.is_empty() {
                return Err(BenchmarkError("benchmark case id 不能为空".to_string()));
            }
            if !ids.insert(raw_case.id.clone()) {
                return Err(case_error(&raw_case.id, "id 重复"));
            }
            if raw_case.source.is_empty() {
                return Err(case_error(&raw_case.id, "source 不能为空"));
            }
            let input: KeySequence = raw_case
                .raw_input
                .parse()
                .map_err(|error| case_error(&raw_case.id, &format!("rawInput 非法: {error}")))?;
            let mut lattice = Lattice::new(input.clone());
            for edge in raw_case.edges {
                let span = Span::new(edge.start, edge.end)
                    .map_err(|error| case_error(&raw_case.id, &error.to_string()))?;
                let candidate = EdgeCandidate::new(edge.text, edge.kind.into(), edge.frequency)
                    .map_err(|error| case_error(&raw_case.id, &error.to_string()))?;
                lattice
                    .add_edge(span, candidate)
                    .map_err(|error| case_error(&raw_case.id, &error.to_string()))?;
            }
            if lattice
                .complete_paths(NonZeroUsize::new(1).expect("1 非零"))
                .paths()
                .is_empty()
            {
                return Err(case_error(&raw_case.id, "lattice 没有覆盖完整输入的路径"));
            }

            let (expected_edge_ids, expected_signature, expected_text) =
                resolve_expected(&raw_case.id, &lattice, &raw_case.expected)?;
            let context = RuntimeContext::new(raw_case.left_context, input);
            cases.push(BenchmarkCase {
                id: raw_case.id,
                split: raw_case.split.into(),
                source: raw_case.source,
                tags: raw_case.tags.into_boxed_slice(),
                context,
                lattice,
                expected_edge_ids,
                expected_signature,
                expected_text,
            });
        }

        let development = cases
            .iter()
            .any(|case| case.split == BenchmarkSplit::Development);
        let evaluation = cases
            .iter()
            .any(|case| case.split == BenchmarkSplit::Evaluation);
        if !development || !evaluation {
            return Err(BenchmarkError(
                "benchmark 必须同时包含 development 与 evaluation split".to_string(),
            ));
        }

        Ok(Self {
            top_k: raw.top_k,
            max_paths_per_case,
            cases,
        })
    }

    pub fn cases(&self) -> &[BenchmarkCase] {
        &self.cases
    }

    pub fn top_k(&self) -> usize {
        self.top_k
    }

    pub fn max_paths_per_case(&self) -> NonZeroUsize {
        self.max_paths_per_case
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BenchmarkReport {
    pub schema: &'static str,
    pub benchmark_schema: &'static str,
    pub scorer: &'static str,
    pub case_count: usize,
    pub development_cases: usize,
    pub evaluation_cases: usize,
    pub top_k: usize,
    pub top1_text_correct: usize,
    pub top1_path_correct: usize,
    pub top_k_path_correct: usize,
    pub contextual_case_count: usize,
    pub contextual_top1_correct: usize,
    pub truncated_cases: usize,
    pub total_complete_paths: usize,
    pub maximum_complete_paths: usize,
    pub mean_candidate_fanout: f64,
    pub mean_complete_paths: f64,
    pub latency_micros: LatencyPercentiles,
}

impl BenchmarkReport {
    pub fn top1_text_accuracy(&self) -> f64 {
        ratio(self.top1_text_correct, self.case_count)
    }

    pub fn segmentation_accuracy(&self) -> f64 {
        ratio(self.top1_path_correct, self.case_count)
    }

    pub fn top_k_path_accuracy(&self) -> f64 {
        ratio(self.top_k_path_correct, self.case_count)
    }

    pub fn contextual_disambiguation_accuracy(&self) -> f64 {
        ratio(self.contextual_top1_correct, self.contextual_case_count)
    }
}

#[derive(Clone, Copy, Debug, Default, Serialize)]
pub struct LatencyPercentiles {
    pub p50: u128,
    pub p95: u128,
    pub p99: u128,
}

pub struct BenchmarkRunner {
    scorer: BaselineScorer,
}

impl BenchmarkRunner {
    pub fn new(scorer: BaselineScorer) -> Self {
        Self { scorer }
    }

    pub fn run(&self, suite: &BenchmarkSuite) -> BenchmarkReport {
        let contextual_inputs = contextual_inputs(&suite.cases);
        let mut top1_text_correct = 0;
        let mut top1_path_correct = 0;
        let mut top_k_path_correct = 0;
        let mut contextual_case_count = 0;
        let mut contextual_top1_correct = 0;
        let mut truncated_cases = 0;
        let mut total_complete_paths = 0;
        let mut maximum_complete_paths = 0;
        let mut total_edges = 0;
        let mut active_positions = 0;
        let mut latencies = Vec::with_capacity(suite.cases.len());

        for case in &suite.cases {
            let started = Instant::now();
            let paths = case.lattice.complete_paths(suite.max_paths_per_case);
            let ranked = rank_paths(&self.scorer, &case.context, &case.lattice, paths.paths());
            latencies.push(started.elapsed().as_micros());

            let top = ranked.first().expect("suite 校验保证至少一条完整路径");
            let path_correct = top.path().edge_ids() == case.expected_edge_ids.as_ref();
            top1_text_correct += usize::from(top.text() == case.expected_text);
            top1_path_correct += usize::from(path_correct);
            top_k_path_correct += usize::from(
                ranked
                    .iter()
                    .take(suite.top_k)
                    .any(|path| path.path().edge_ids() == case.expected_edge_ids.as_ref()),
            );
            if contextual_inputs.contains(&case.context.composition().to_string()) {
                contextual_case_count += 1;
                contextual_top1_correct += usize::from(path_correct);
            }
            truncated_cases += usize::from(paths.truncated());
            total_complete_paths += paths.paths().len();
            maximum_complete_paths = maximum_complete_paths.max(paths.paths().len());
            total_edges += case.lattice.edges().len();
            active_positions += (0..case.lattice.input().len())
                .filter(|&position| {
                    case.lattice
                        .outgoing(position)
                        .is_some_and(|edges| !edges.is_empty())
                })
                .count();
        }

        BenchmarkReport {
            schema: REPORT_SCHEMA,
            benchmark_schema: BENCHMARK_SCHEMA,
            scorer: BaselineScorer::SCORER_ID,
            case_count: suite.cases.len(),
            development_cases: suite
                .cases
                .iter()
                .filter(|case| case.split == BenchmarkSplit::Development)
                .count(),
            evaluation_cases: suite
                .cases
                .iter()
                .filter(|case| case.split == BenchmarkSplit::Evaluation)
                .count(),
            top_k: suite.top_k,
            top1_text_correct,
            top1_path_correct,
            top_k_path_correct,
            contextual_case_count,
            contextual_top1_correct,
            truncated_cases,
            total_complete_paths,
            maximum_complete_paths,
            mean_candidate_fanout: ratio(total_edges, active_positions),
            mean_complete_paths: ratio(total_complete_paths, suite.cases.len()),
            latency_micros: percentiles(&mut latencies),
        }
    }
}

impl Default for BenchmarkRunner {
    fn default() -> Self {
        Self::new(BaselineScorer::default())
    }
}

/// 用机器可读 baseline 精确比较非计时指标；计时只报告、不跨机器设门槛。
pub fn check_baseline_json(
    report: &BenchmarkReport,
    text: &str,
) -> Result<Vec<String>, BenchmarkError> {
    let baseline: RawBaseline = serde_json::from_str(text)
        .map_err(|error| BenchmarkError(format!("contextual baseline JSON 非法: {error}")))?;
    if baseline.schema != BASELINE_SCHEMA {
        return Err(BenchmarkError(format!(
            "baseline schema 应为 {BASELINE_SCHEMA:?}，实际为 {:?}",
            baseline.schema
        )));
    }
    if baseline.benchmark_schema != BENCHMARK_SCHEMA {
        return Err(BenchmarkError(format!(
            "baseline benchmarkSchema 应为 {BENCHMARK_SCHEMA:?}"
        )));
    }
    if baseline.scorer != BaselineScorer::SCORER_ID {
        return Err(BenchmarkError(format!(
            "baseline scorer 应为 {:?}",
            BaselineScorer::SCORER_ID
        )));
    }

    let expected = baseline.metrics;
    let actual = [
        ("caseCount", report.case_count, expected.case_count),
        (
            "top1TextCorrect",
            report.top1_text_correct,
            expected.top1_text_correct,
        ),
        (
            "top1PathCorrect",
            report.top1_path_correct,
            expected.top1_path_correct,
        ),
        (
            "topKPathCorrect",
            report.top_k_path_correct,
            expected.top_k_path_correct,
        ),
        (
            "contextualCaseCount",
            report.contextual_case_count,
            expected.contextual_case_count,
        ),
        (
            "contextualTop1Correct",
            report.contextual_top1_correct,
            expected.contextual_top1_correct,
        ),
        (
            "truncatedCases",
            report.truncated_cases,
            expected.truncated_cases,
        ),
        (
            "totalCompletePaths",
            report.total_complete_paths,
            expected.total_complete_paths,
        ),
        (
            "maximumCompletePaths",
            report.maximum_complete_paths,
            expected.maximum_complete_paths,
        ),
    ];
    Ok(actual
        .into_iter()
        .filter(|(_, actual, expected)| actual != expected)
        .map(|(name, actual, expected)| format!("{name}: expected {expected}, actual {actual}"))
        .collect())
}

fn contextual_inputs(cases: &[BenchmarkCase]) -> BTreeSet<String> {
    let mut groups: BTreeMap<String, (BTreeSet<&str>, BTreeSet<&str>)> = BTreeMap::new();
    for case in cases {
        let group = groups
            .entry(case.context.composition().to_string())
            .or_default();
        group.0.insert(&case.expected_signature);
        group.1.insert(case.context.committed_left());
    }
    groups
        .into_iter()
        .filter_map(|(input, (expectations, contexts))| {
            (expectations.len() > 1 && contexts.len() > 1).then_some(input)
        })
        .collect()
}

fn resolve_expected(
    case_id: &str,
    lattice: &Lattice,
    expected: &[RawExpectedSegment],
) -> Result<(Box<[EdgeId]>, String, String), BenchmarkError> {
    if expected.is_empty() {
        return Err(case_error(case_id, "expected segments 不能为空"));
    }
    let mut position = 0;
    let mut edge_ids = Vec::with_capacity(expected.len());
    let mut signature = String::new();
    let mut text = String::new();
    for segment in expected {
        if segment.start != position || segment.start >= segment.end {
            return Err(case_error(
                case_id,
                "expected segments 必须从 0 开始、连续、非空且严格前进",
            ));
        }
        let matches: Vec<_> = lattice
            .edges()
            .iter()
            .filter(|edge| {
                edge.span().start() == segment.start
                    && edge.span().end() == segment.end
                    && edge.candidate().text() == segment.text
            })
            .collect();
        if matches.len() != 1 {
            return Err(case_error(
                case_id,
                &format!(
                    "expected segment {}..{} {:?} 应唯一命中一条 edge，实际 {}",
                    segment.start,
                    segment.end,
                    segment.text,
                    matches.len()
                ),
            ));
        }
        let edge = matches[0];
        edge_ids.push(edge.id());
        signature.push_str(&format!(
            "{}-{}:{}:{};",
            segment.start,
            segment.end,
            segment.text.len(),
            segment.text
        ));
        text.push_str(&segment.text);
        position = segment.end;
    }
    if position != lattice.input().len() {
        return Err(case_error(case_id, "expected segments 未覆盖完整 rawInput"));
    }
    Ok((edge_ids.into_boxed_slice(), signature, text))
}

fn ratio(numerator: usize, denominator: usize) -> f64 {
    numerator as f64 / denominator.max(1) as f64
}

fn percentiles(values: &mut [u128]) -> LatencyPercentiles {
    values.sort_unstable();
    LatencyPercentiles {
        p50: percentile(values, 50),
        p95: percentile(values, 95),
        p99: percentile(values, 99),
    }
}

fn percentile(values: &[u128], percentile: usize) -> u128 {
    if values.is_empty() {
        return 0;
    }
    let rank = (values.len() * percentile).div_ceil(100);
    values[rank.saturating_sub(1).min(values.len() - 1)]
}

fn case_error(id: &str, message: &str) -> BenchmarkError {
    BenchmarkError(format!("benchmark case {id:?}: {message}"))
}

#[derive(Debug)]
pub struct BenchmarkError(String);

impl fmt::Display for BenchmarkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for BenchmarkError {}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RawSuite {
    schema: String,
    top_k: usize,
    max_paths_per_case: usize,
    cases: Vec<RawCase>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RawCase {
    id: String,
    split: RawSplit,
    source: String,
    #[serde(default)]
    tags: Vec<String>,
    left_context: String,
    raw_input: String,
    edges: Vec<RawEdge>,
    expected: Vec<RawExpectedSegment>,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum RawSplit {
    Development,
    Evaluation,
}

impl From<RawSplit> for BenchmarkSplit {
    fn from(value: RawSplit) -> Self {
        match value {
            RawSplit::Development => Self::Development,
            RawSplit::Evaluation => Self::Evaluation,
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RawEdge {
    start: usize,
    end: usize,
    text: String,
    kind: RawCandidateKind,
    frequency: u64,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum RawCandidateKind {
    HotWord,
    ExtendedWord,
    Character,
    AttestedAlias,
    UserLearned,
    OovComposition,
}

impl From<RawCandidateKind> for CandidateKind {
    fn from(value: RawCandidateKind) -> Self {
        match value {
            RawCandidateKind::HotWord => Self::HotWord,
            RawCandidateKind::ExtendedWord => Self::ExtendedWord,
            RawCandidateKind::Character => Self::Character,
            RawCandidateKind::AttestedAlias => Self::AttestedAlias,
            RawCandidateKind::UserLearned => Self::UserLearned,
            RawCandidateKind::OovComposition => Self::OovComposition,
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RawExpectedSegment {
    start: usize,
    end: usize,
    text: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RawBaseline {
    schema: String,
    benchmark_schema: String,
    scorer: String,
    metrics: RawBaselineMetrics,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RawBaselineMetrics {
    case_count: usize,
    top1_text_correct: usize,
    top1_path_correct: usize,
    top_k_path_correct: usize,
    contextual_case_count: usize,
    contextual_top1_correct: usize,
    truncated_cases: usize,
    total_complete_paths: usize,
    maximum_complete_paths: usize,
}
