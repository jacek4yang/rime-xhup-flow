//! 语料统计引擎(Input Model v2 §8):从真实句子语料产出优化器证据。
//!
//! 语料管线(canonical data 之外的证据层):
//!
//! ```text
//! 语料源(本地,不入库;许可与 pin 见 data/corpus/README.md)
//!   -> 预处理为纯文本句子(每行一句,UTF-8)
//!   -> 本模块:最大匹配分词(canonical 词表 + 单字,确定性)
//!   -> 统计:unigram 计数 / 句子覆盖 / 独立左右上下文数
//!   -> 派生统计 TSV(可再分发,入库,含来源哈希)
//!   -> LexicalEvidence 的语料信号字段(docs/input-model-v2.md §2)
//! ```
//!
//! 设计约束:
//!
//! - **确定性**:同输入语料 + 同 canonical 词表 → 字节级一致的输出;
//! - **隐私/许可**:只有聚合计数入库,绝不入库原句;
//! - 分词只用于统计,不声称语言学最优;最大匹配对「词是否可用、
//!   在什么上下文出现」这类证据足够,且完全可复现。

use std::collections::{BTreeMap, BTreeSet};

/// 最大匹配分词器:词表 = canonical 词(2..=4 字)+ canonical 单字。
///
/// 算法:从左到右贪心最长匹配(先查 4 字,再 3/2 字,最后单字)。
/// 无法匹配的字符(非规范汉字、ASCII、标点)作为边界跳过 —— 它们天然
/// 切断上下文,不进入任何词的上下文集。
pub struct Segmenter {
    /// 按字长分桶的词表(2/3/4 字词)。
    words_by_len: [BTreeSet<String>; 3],
    /// canonical 单字集。
    chars: BTreeSet<char>,
}

impl Segmenter {
    /// 从 generator 分析投影构建(词表与生产码表同源)。
    pub fn build() -> Self {
        let mut words_by_len: [BTreeSet<String>; 3] =
            [BTreeSet::new(), BTreeSet::new(), BTreeSet::new()];
        for entry in xhup_generator::word_code_analysis_entries() {
            let len = entry.word().chars().count();
            if (2..=4).contains(&len) {
                words_by_len[len - 2].insert(entry.word().to_string());
            }
        }
        let chars = xhup_generator::char_code_analysis_entries()
            .iter()
            .map(|e| e.hanzi().as_char())
            .collect();
        Segmenter {
            words_by_len,
            chars,
        }
    }

    /// 分词一句为 token 序列(词或单字;不可匹配字符跳过)。
    pub fn segment(&self, sentence: &str) -> Vec<String> {
        let chars: Vec<char> = sentence.chars().collect();
        let mut tokens = Vec::new();
        let mut pos = 0;
        while pos < chars.len() {
            let mut matched = None;
            for len in (2..=4).rev() {
                if pos + len <= chars.len() {
                    let candidate: String = chars[pos..pos + len].iter().collect();
                    if self.words_by_len[len - 2].contains(&candidate) {
                        matched = Some((candidate, len));
                        break;
                    }
                }
            }
            if let Some((token, len)) = matched {
                tokens.push(token);
                pos += len;
            } else {
                let ch = chars[pos];
                if self.chars.contains(&ch) {
                    tokens.push(ch.to_string());
                }
                pos += 1;
            }
        }
        tokens
    }
}

/// 单词的语料统计证据。
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct WordCorpusStats {
    /// 出现次数(unigram 计数,分词后 token 级)。
    pub count: u64,
    /// 出现于多少个不同句子(句子覆盖)。
    pub sentence_count: u64,
    /// 独立左邻 token 数(上下文多样性,句首记边界 `<s>`)。
    pub left_contexts: u64,
    /// 独立右邻 token 数(句尾记边界 `</s>`)。
    pub right_contexts: u64,
}

/// 全语料统计:词 → 证据(只含出现过的词;确定性 BTreeMap 序)。
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CorpusStats {
    /// 每词证据。
    pub words: BTreeMap<String, WordCorpusStats>,
    /// 处理的句子总数。
    pub sentences: u64,
    /// 分词产出的 token 总数。
    pub tokens: u64,
}

/// 语料统计器:喂句子,聚合证据。
pub struct CorpusStatsBuilder {
    segmenter: Segmenter,
    counts: BTreeMap<String, u64>,
    sentence_counts: BTreeMap<String, u64>,
    left: BTreeMap<String, BTreeSet<String>>,
    right: BTreeMap<String, BTreeSet<String>>,
    sentences: u64,
    tokens: u64,
}

impl Default for CorpusStatsBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl CorpusStatsBuilder {
    /// 以 canonical 词表构建。
    pub fn new() -> Self {
        CorpusStatsBuilder {
            segmenter: Segmenter::build(),
            counts: BTreeMap::new(),
            sentence_counts: BTreeMap::new(),
            left: BTreeMap::new(),
            right: BTreeMap::new(),
            sentences: 0,
            tokens: 0,
        }
    }

    /// 喂入一个句子(空句/无 token 句仍计入句子总数)。
    pub fn feed(&mut self, sentence: &str) {
        self.sentences += 1;
        let tokens = self.segmenter.segment(sentence);
        self.tokens += tokens.len() as u64;
        let mut seen_in_sentence: BTreeSet<&str> = BTreeSet::new();
        for (index, token) in tokens.iter().enumerate() {
            *self.counts.entry(token.clone()).or_insert(0) += 1;
            if seen_in_sentence.insert(token) {
                *self.sentence_counts.entry(token.clone()).or_insert(0) += 1;
            }
            let left = if index == 0 {
                "<s>"
            } else {
                tokens[index - 1].as_str()
            };
            let right = if index + 1 == tokens.len() {
                "</s>"
            } else {
                tokens[index + 1].as_str()
            };
            self.left
                .entry(token.clone())
                .or_default()
                .insert(left.to_string());
            self.right
                .entry(token.clone())
                .or_default()
                .insert(right.to_string());
        }
    }

    /// 完成聚合。
    pub fn finish(self) -> CorpusStats {
        let words = self
            .counts
            .iter()
            .map(|(word, &count)| {
                let stats = WordCorpusStats {
                    count,
                    sentence_count: self.sentence_counts.get(word).copied().unwrap_or(0),
                    left_contexts: self.left.get(word).map(|s| s.len() as u64).unwrap_or(0),
                    right_contexts: self.right.get(word).map(|s| s.len() as u64).unwrap_or(0),
                };
                (word.clone(), stats)
            })
            .collect();
        CorpusStats {
            words,
            sentences: self.sentences,
            tokens: self.tokens,
        }
    }
}

impl CorpusStats {
    /// 序列化为确定性 TSV(BTreeMap 序;无时间戳/路径)。
    ///
    /// 格式:`词<TAB>计数<TAB>句子数<TAB>左上下文数<TAB>右上下文数`,
    /// 头部注释行记录句子/token 总量(审计用)。
    pub fn to_tsv(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!(
            "# sentences={} tokens={} words={}\n",
            self.sentences,
            self.tokens,
            self.words.len()
        ));
        out.push_str("word\tcount\tsentences\tleft_contexts\tright_contexts\n");
        for (word, stats) in &self.words {
            out.push_str(&format!(
                "{}\t{}\t{}\t{}\t{}\n",
                word, stats.count, stats.sentence_count, stats.left_contexts, stats.right_contexts
            ));
        }
        out
    }

    /// 解析 [`Self::to_tsv`] 的输出(往返一致)。
    pub fn from_tsv(text: &str) -> Result<Self, String> {
        let mut words = BTreeMap::new();
        let (mut sentences, mut tokens) = (0u64, 0u64);
        for (index, line) in text.lines().enumerate() {
            if line.is_empty() {
                continue;
            }
            if let Some(header) = line.strip_prefix("# ") {
                for field in header.split(' ') {
                    if let Some((key, value)) = field.split_once('=') {
                        let parsed: u64 = value
                            .parse()
                            .map_err(|_| format!("第 {} 行头部数值非法: {field:?}", index + 1))?;
                        match key {
                            "sentences" => sentences = parsed,
                            "tokens" => tokens = parsed,
                            _ => {}
                        }
                    }
                }
                continue;
            }
            if line == "word\tcount\tsentences\tleft_contexts\tright_contexts" {
                continue;
            }
            let fields: Vec<&str> = line.split('\t').collect();
            let [word, count, sents, left, right] = fields.as_slice() else {
                return Err(format!("第 {} 行应为五列: {line:?}", index + 1));
            };
            let parse = |v: &str, what: &str| -> Result<u64, String> {
                v.parse()
                    .map_err(|_| format!("第 {} 行{what}应为非负整数: {v:?}", index + 1))
            };
            if words.contains_key(*word) {
                return Err(format!("第 {} 行词重复: {word:?}", index + 1));
            }
            words.insert(
                (*word).to_string(),
                WordCorpusStats {
                    count: parse(count, "计数")?,
                    sentence_count: parse(sents, "句子数")?,
                    left_contexts: parse(left, "左上下文数")?,
                    right_contexts: parse(right, "右上下文数")?,
                },
            );
        }
        Ok(CorpusStats {
            words,
            sentences,
            tokens,
        })
    }
}

#[cfg(test)]
mod tests {
    //! 分词与统计语义用小型合成语料验证(不依赖 canonical 数据细节,
    //! 只依赖骨干词 我们/时间 与单字 我/们 等的在库性 —— 由常用词门禁保护)。
    use super::*;

    #[test]
    fn segmentation_prefers_longest_match() {
        let segmenter = Segmenter::build();
        // 「我们」在词表 → 整体匹配;不会被拆成 我+们。
        assert_eq!(segmenter.segment("我们"), vec!["我们"]);
        // 标点/ASCII 作为边界跳过。
        let tokens = segmenter.segment("我们,时间");
        assert_eq!(tokens, vec!["我们", "时间"]);
    }

    #[test]
    fn segmentation_is_deterministic() {
        let segmenter = Segmenter::build();
        assert_eq!(
            segmenter.segment("我们时间发展"),
            segmenter.segment("我们时间发展")
        );
    }

    #[test]
    fn stats_count_coverage_and_contexts() {
        let mut builder = CorpusStatsBuilder::new();
        builder.feed("我们时间");
        builder.feed("我们时间");
        builder.feed("时间我们");
        let stats = builder.finish();
        assert_eq!(stats.sentences, 3);
        let women = &stats.words["我们"];
        assert_eq!(women.count, 3);
        assert_eq!(women.sentence_count, 3, "出现于 3 个句子");
        // 左邻:<s>(句首)×2 + 时间 ×1 = 2 种。
        assert_eq!(women.left_contexts, 2);
        // 右邻:时间 ×2 + </s>(句尾)×1 = 2 种。
        assert_eq!(women.right_contexts, 2);
    }

    #[test]
    fn tsv_round_trip_is_byte_stable() {
        let mut builder = CorpusStatsBuilder::new();
        builder.feed("我们时间");
        builder.feed("我们");
        let stats = builder.finish();
        let tsv = stats.to_tsv();
        let parsed = CorpusStats::from_tsv(&tsv).expect("应能解析");
        assert_eq!(parsed, stats);
        assert_eq!(parsed.to_tsv(), tsv, "序列化应字节级稳定");
    }

    #[test]
    fn tsv_rejects_malformed_rows() {
        assert!(CorpusStats::from_tsv("我们\t1\n").is_err());
        assert!(CorpusStats::from_tsv("我们\t1\t1\t1\t1\n我们\t2\t2\t2\t2\n").is_err());
    }
}
