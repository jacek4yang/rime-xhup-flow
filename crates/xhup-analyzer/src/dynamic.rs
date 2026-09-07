//! Flow 引擎动态候选的 typed 身份。
//!
//! 与 [`crate::policy::ShortcutPolicyId`](冻结静态层)严格分离:动态候选
//! 语义不属于任何冻结 policy,未来 Trainer / 审计报告工具经本枚举分类
//! 动态候选来源,不解析 README 文本、不混用静态层身份。
//!
//! 语义优先级契约(Flow 引擎 v1,见 README「Flow 引擎」节):
//! `FROZEN STATIC > DYNAMIC / USER-LEARNED > SENTENCE COMPOSITION`。
//! 本枚举只描述身份与优先级语义,不承载任何可变状态(学习数据只存在于
//! librime 用户词典 `xhup_flow_user`,绝不入库)。

/// Flow 引擎动态候选来源的 typed 身份。
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum DynamicCandidateKind {
    /// 用户词典词条(用户词典内已有的词,按用户权重排序)。
    UserWord,
    /// 学习短语(encoder 由提交历史机械生成的多字短语词条)。
    LearnedPhrase,
    /// 连续组句(`table_translator@flow` 的 enable_sentence 分段合成)。
    Sentence,
}

impl DynamicCandidateKind {
    /// 全部动态候选来源(稳定序:语义优先级从高到低)。
    ///
    /// 同码动态候选内部按此顺序排列;整体永远排在全部冻结静态候选之后
    /// (translator 间由 initial_quality 栅栏保证)。
    pub const ALL: [Self; 3] = [Self::UserWord, Self::LearnedPhrase, Self::Sentence];

    /// 报告用稳定标签。
    pub fn label(self) -> &'static str {
        match self {
            Self::UserWord => "user-word",
            Self::LearnedPhrase => "learned-phrase",
            Self::Sentence => "sentence",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_kinds_are_ordered_by_priority() {
        assert_eq!(
            DynamicCandidateKind::ALL,
            [
                DynamicCandidateKind::UserWord,
                DynamicCandidateKind::LearnedPhrase,
                DynamicCandidateKind::Sentence,
            ]
        );
    }

    #[test]
    fn labels_are_stable() {
        assert_eq!(DynamicCandidateKind::UserWord.label(), "user-word");
        assert_eq!(
            DynamicCandidateKind::LearnedPhrase.label(),
            "learned-phrase"
        );
        assert_eq!(DynamicCandidateKind::Sentence.label(), "sentence");
    }
}
