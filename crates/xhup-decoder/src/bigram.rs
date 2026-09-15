//! Committed-Context bigram 评分器:KDConv 会话语料派生的词级转移证据。
//!
//! 证据来源与可复现性见 `data/corpus/README.md`:KdConv(Apache-2.0,commit
//! pin 653db76)92,558 句经生产同源最大匹配分词后聚合。分片文件为确定性
//! TSV(`left<TAB>right<TAB>count`,边界 token `<s>`/`</s>`,按
//! `(left, right)` 字典序),由 `CorpusStatsBuilder` 的分词流重建。
//!
//! 模型:整数 log2 bigram 转移奖励 + Q10 log2 unigram 回退,固定点 `Score`
//! 与 [`BaselineScorer`] 同标度;无证据(未见 pair)为零奖励而非拒绝,
//! 保证 OOV/open composition 可达。忽略上下文(零转移证据)时得分与
//! baseline 一致,提供干净的 A/B 对照。

use std::collections::BTreeMap;

use crate::scoring::{BaselineScoreBreakdown, BaselineScorer, Score};
use crate::{DeterministicScorer, Lattice, LatticePath, RuntimeContext};

/// 句首/句尾边界 token(与 corpus 统计层约定一致)。
pub const BOS: &str = "<s>";
pub const EOS: &str = "</s>";

/// KDConv 会话域 bigram 转移证据(不可变,BTreeMap 确定性序)。
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct BigramModel {
    /// 已知词表(含 `<s>`/`</s>` 边界与全部单字/词 token)。
    vocabulary: std::collections::BTreeSet<String>,
    /// 转移计数:left → (right → count)。
    transitions: BTreeMap<String, BTreeMap<String, u64>>,
    /// unigram 计数(含边界)。
    unigrams: BTreeMap<String, u64>,
    /// 转移对总数(审计用)。
    pair_count: usize,
}

impl BigramModel {
    /// 从确定性 TSV 构建(格式:`left<TAB>right<TAB>count`,含头部注释
    /// 与 `left right count` 列头行)。
    ///
    /// 计数为零或非法的行是数据错误,直接拒绝(审计门禁语义)。
    pub fn from_tsv(text: &str) -> Result<Self, String> {
        let mut model = Self::default();
        for (index, line) in text.lines().enumerate() {
            if line.is_empty() || line.starts_with('#') || line == "left\tright\tcount" {
                continue;
            }
            let fields: Vec<&str> = line.split('\t').collect();
            let [left, right, count] = fields.as_slice() else {
                return Err(format!("第 {} 行应为三列: {line:?}", index + 1));
            };
            let count: u64 = count
                .parse()
                .map_err(|_| format!("第 {} 行计数非法: {count:?}", index + 1))?;
            if count == 0 {
                return Err(format!("第 {} 行计数为零", index + 1));
            }
            model.observe(left, right, count);
        }
        Ok(model)
    }

    /// 记录一次转移观测(可重复调用,计数累加)。
    pub fn observe(&mut self, left: &str, right: &str, count: u64) {
        self.vocabulary.insert(left.to_string());
        self.vocabulary.insert(right.to_string());
        *self.unigrams.entry(left.to_string()).or_insert(0) += count;
        *self.unigrams.entry(right.to_string()).or_insert(0) += count;
        *self
            .transitions
            .entry(left.to_string())
            .or_default()
            .entry(right.to_string())
            .or_insert(0) += count;
        self.pair_count += 1;
    }

    pub fn is_empty(&self) -> bool {
        self.transitions.is_empty()
    }

    pub fn vocabulary_size(&self) -> usize {
        self.vocabulary.len()
    }

    /// 唯一转移对数(审计用;插入次数)。
    pub fn pair_count(&self) -> usize {
        self.pair_count
    }

    /// 查询转移计数;无证据返回 0。
    pub fn transition_count(&self, left: &str, right: &str) -> u64 {
        self.transitions
            .get(left)
            .and_then(|row| row.get(right))
            .copied()
            .unwrap_or(0)
    }

    /// unigram 计数;无证据返回 0。
    pub fn unigram_count(&self, token: &str) -> u64 {
        self.unigrams.get(token).copied().unwrap_or(0)
    }

    /// 确定性 TSV 序列化(`(left, right)` 字典序,头部注释含审计总量)。
    pub fn to_tsv(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!(
            "# kdconv-bigram pairs={} vocabulary={} tokens={}\n",
            self.pair_count,
            self.vocabulary_size(),
            self.unigrams.values().sum::<u64>(),
        ));
        for (left, row) in &self.transitions {
            for (right, &count) in row {
                out.push_str(&format!("{left}\t{right}\t{count}\n"));
            }
        }
        out
    }

    /// 提取 committed left context 的尾部 token 序列(有界窗口)。
    ///
    /// 优先匹配最长已知词(1..=4 字),否则按单字切分;未知字符切断
    /// 上下文链(与 corpus 统计层的边界语义一致:无法匹配的字符天然
    /// 切断上下文)。窗口超限时保留最近的 token。
    fn tail_tokens(&self, committed_left: &str, max_tokens: usize) -> Vec<String> {
        let chars: Vec<char> = committed_left.chars().collect();
        let mut tokens: Vec<String> = Vec::new();
        let mut pos = 0;
        while pos < chars.len() {
            let mut matched = None;
            for len in (2..=4).rev() {
                if pos + len <= chars.len() {
                    let candidate: String = chars[pos..pos + len].iter().collect();
                    if self.vocabulary.contains(&candidate) {
                        matched = Some((candidate, len));
                        break;
                    }
                }
            }
            match matched {
                Some((token, len)) => {
                    tokens.push(token);
                    pos += len;
                }
                None => {
                    let single = chars[pos].to_string();
                    if self.vocabulary.contains(&single) {
                        tokens.push(single);
                    } else {
                        // 未知字符:切断上下文链(边界语义),丢弃既有窗口。
                        tokens.clear();
                    }
                    pos += 1;
                }
            }
        }
        if tokens.len() > max_tokens {
            tokens.split_off(tokens.len() - max_tokens)
        } else {
            tokens
        }
    }
}

/// Committed-Context bigram scorer(`kdconv-bigram/v1`)。
///
/// 得分 = baseline(词频奖励 − 分段惩罚) + Σ 转移奖励。转移奖励对
/// 「committed left context 尾 token → 路径首段」与路径内相邻段的语料
/// 同现都计入;上下文窗口由 `context_window` 截断。语料中未见过的组合
/// 零奖励,不阻断 OOV/open composition 可达性。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KdconvBigramScorer {
    baseline: BaselineScorer,
    model: BigramModel,
    /// 参与打分的 committed context 尾 token 上限。
    context_window: usize,
    /// Q10 log2(count + 1) 的缩放系数(转移奖励弱于词频主项)。
    transition_weight: Score,
}

impl KdconvBigramScorer {
    pub const SCORER_ID: &'static str = "kdconv-bigram/v1";

    pub fn new(model: BigramModel) -> Self {
        Self {
            baseline: BaselineScorer::default(),
            model,
            context_window: 4,
            transition_weight: 256,
        }
    }

    pub fn model(&self) -> &BigramModel {
        &self.model
    }

    pub fn context_window(&self) -> usize {
        self.context_window
    }

    pub fn transition_weight(&self) -> Score {
        self.transition_weight
    }

    /// Q10 log2(count + 1)(与 baseline 词频奖励同标度)。
    fn log2_q10(count: u64) -> Score {
        let value = count.saturating_add(1);
        let exponent = value.ilog2();
        let base = 1_u64 << exponent;
        let fractional = (((value - base) as u128) * 1024 / base as u128) as Score;
        Score::from(exponent) * 1024 + fractional
    }

    /// 相邻 (left, right) token 的转移奖励;无证据为零。
    fn transition_reward(&self, left: &str, right: &str) -> Score {
        let count = self.model.transition_count(left, right);
        if count == 0 {
            return 0;
        }
        Self::log2_q10(count).saturating_mul(self.transition_weight)
    }
}

impl DeterministicScorer for KdconvBigramScorer {
    type Breakdown = KdconvBigramBreakdown;

    fn score(
        &self,
        context: &RuntimeContext,
        lattice: &Lattice,
        path: &LatticePath,
    ) -> (Score, Self::Breakdown) {
        let (baseline_score, baseline_breakdown) = self.baseline.score(context, lattice, path);

        let segments: Vec<&str> = path
            .edge_ids()
            .iter()
            .map(|&id| {
                lattice
                    .edge(id)
                    .expect("path edge 必须属于 lattice")
                    .candidate()
                    .text()
            })
            .collect();

        let tail = self
            .model
            .tail_tokens(context.committed_left(), self.context_window);

        let mut transition_reward = 0_i64;
        let mut transitions = Vec::new();
        // 转移奖励覆盖两段证据链:(1) committed context 尾 token → 路径
        // 首段(窗口内最近的优先);(2) 路径内相邻段的语料同现。空上下文
        // 且路径单段时无转移奖励,得分与 baseline 严格一致(干净的 A/B
        // 对照,见模块文档)。
        let seed = tail.last().map(String::as_str);
        let mut previous: Option<&str> = seed;
        for segment in &segments {
            if let Some(left) = previous {
                let reward = self.transition_reward(left, segment);
                if reward != 0 {
                    transitions.push((left.to_string(), (*segment).to_string(), reward));
                }
                transition_reward = transition_reward.saturating_add(reward);
            }
            previous = Some(segment);
        }

        let total = baseline_score.saturating_add(transition_reward);
        (
            total,
            KdconvBigramBreakdown {
                baseline: baseline_breakdown,
                transition_reward,
                transitions: transitions.into_boxed_slice(),
            },
        )
    }
}

/// bigram scorer 的可解释分数组成。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KdconvBigramBreakdown {
    /// baseline 部分总分(词频奖励 − 分段惩罚)。
    pub baseline: BaselineScoreBreakdown,
    /// 全部转移奖励之和(0 表示上下文未提供任何证据)。
    pub transition_reward: Score,
    /// 命中的转移(Trainer 解释用):left、right、Q10 奖励。
    pub transitions: Box<[(String, String, Score)]>,
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroUsize;

    use super::*;
    use crate::{CandidateKind, EdgeCandidate, Span};

    fn complete_paths(lattice: &Lattice, limit: usize) -> crate::PathSet {
        lattice.complete_paths(NonZeroUsize::new(limit).unwrap())
    }

    fn lattice_for(input: &str, edges: &[(&str, usize, usize, u64)]) -> Lattice {
        let mut lattice = Lattice::new(input.parse().expect("合法按键"));
        for (text, start, end, frequency) in edges {
            lattice
                .add_edge(
                    Span::new(*start, *end).unwrap(),
                    EdgeCandidate::new(*text, CandidateKind::HotWord, *frequency).unwrap(),
                )
                .unwrap();
        }
        lattice
    }

    fn model_with(pairs: &[(&str, &str, u64)]) -> BigramModel {
        let mut model = BigramModel::default();
        for (left, right, count) in pairs {
            model.observe(left, right, *count);
        }
        model
    }

    #[test]
    fn model_tsv_round_trip_is_byte_stable() {
        let model = model_with(&[("研究", "生命", 3), ("<s>", "研究", 7)]);
        let tsv = model.to_tsv();
        let parsed = BigramModel::from_tsv(&tsv).expect("应可解析");
        assert_eq!(parsed, model);
        assert_eq!(parsed.to_tsv(), tsv);
    }

    #[test]
    fn model_rejects_malformed_rows() {
        assert!(BigramModel::from_tsv("研究\t生命\n").is_err());
        assert!(BigramModel::from_tsv("研究\t生命\t0\n").is_err());
        assert!(BigramModel::from_tsv("研究\t生命\tx\n").is_err());
    }

    #[test]
    fn zero_evidence_transitions_score_zero() {
        let model = model_with(&[("时间", "我们", 5)]);
        assert_eq!(model.transition_count("我们", "时间"), 0);
        assert_eq!(model.unigram_count("不存在"), 0);
    }

    #[test]
    fn tail_tokens_prefers_known_words_and_cuts_at_unknown() {
        let model = model_with(&[("我们", "都", 1), ("时间", "了", 1)]);
        // 已知词「我们」整体成 token。
        assert_eq!(model.tail_tokens("我们", 4), vec!["我们"]);
        // 未知字符切断窗口:只剩「时间」「了」。
        assert_eq!(model.tail_tokens("我X时间了", 4), vec!["时间", "了"]);
        // 窗口截断保留最近 token。
        assert_eq!(model.tail_tokens("我们时间了", 1), vec!["了"]);
    }

    #[test]
    fn committed_context_changes_preferred_segmentation() {
        // 研究|生命 vs 研究生|命:词频基线偏好前者;bigram 证据应翻转后者,
        // 当且仅当存在足够的转移证据(路径内 + 上下文种子)。
        let edges: &[(&str, usize, usize, u64)] = &[
            ("研究", 0, 4, 266843),
            ("生命", 4, 8, 80039),
            ("研究生", 0, 6, 72173),
            ("命", 6, 8, 6600),
        ];
        let lattice = lattice_for("yjjqugmk", edges);
        let model = model_with(&[
            ("<s>", "研究生", 10),
            ("研究生", "命", 10),
            ("研究", "生命", 3),
        ]);
        let scorer = KdconvBigramScorer::new(model);

        let context = RuntimeContext::new("他是研究生", "yjjqugmk".parse().unwrap());
        let paths = complete_paths(&lattice, 32);
        let ranked = crate::rank_paths(&scorer, &context, &lattice, paths.paths());
        assert_eq!(
            ranked[0].segments(),
            vec!["研究生", "命"],
            "上下文证据应翻转分段"
        );

        // 空上下文:路径内同现仍然计分;排序确定且可复现。
        let empty = RuntimeContext::new("", "yjjqugmk".parse().unwrap());
        let paths = complete_paths(&lattice, 32);
        let ranked_empty = crate::rank_paths(&scorer, &empty, &lattice, paths.paths());
        assert_eq!(
            ranked_empty[0].segments(),
            vec!["研究生", "命"],
            "路径内同现证据应同样 favor 研究生|命"
        );

        // baseline 对照:无上下文且无路径内证据时,排序与 baseline 一致。
        let plain = model_with(&[("别的", "组合", 5)]);
        let scorer_plain = KdconvBigramScorer::new(plain);
        let baseline = BaselineScorer::default();
        let paths = complete_paths(&lattice, 32);
        let ranked_plain = crate::rank_paths(&scorer_plain, &empty, &lattice, paths.paths());
        let ranked_baseline = crate::rank_paths(&baseline, &empty, &lattice, paths.paths());
        assert_eq!(
            ranked_plain[0].segments(),
            ranked_baseline[0].segments(),
            "无转移证据时排序必须与 baseline 一致"
        );
    }

    #[test]
    fn breakdown_is_explainable() {
        let edges: &[(&str, usize, usize, u64)] = &[
            ("研究", 0, 4, 266843),
            ("生命", 4, 8, 80039),
            ("研究生", 0, 6, 72173),
            ("命", 6, 8, 6600),
        ];
        let lattice = lattice_for("yjjqugmk", edges);
        let model = model_with(&[("研究", "生命", 1024)]);
        let scorer = KdconvBigramScorer::new(model);
        // 上下文尾 token「研究」→ 第二段「生命」命中模型内 (研究, 生命):
        // 种子研究→研究 无证据,链内研究→生命 命中。
        let context = RuntimeContext::new("我们研究", "yjjqugmk".parse().unwrap());
        let paths = complete_paths(&lattice, 32);
        let ranked = crate::rank_paths(&scorer, &context, &lattice, paths.paths());
        let top = ranked
            .iter()
            .find(|p| p.segments() == ["研究", "生命"])
            .expect("研究|生命 路径应存在");
        let breakdown = top.breakdown();
        assert!(breakdown.transition_reward > 0);
        assert_eq!(breakdown.transitions.len(), 1);
        assert_eq!(breakdown.transitions[0].0, "研究");
        assert_eq!(breakdown.transitions[0].1, "生命");
        assert_eq!(
            breakdown.baseline.total + breakdown.transition_reward,
            top.score()
        );
    }
}
