//! 静态简码 policy 注册表:全部已支持生产层的 typed 身份集中化。
//!
//! 未来 Trainer / 安装器 / 报告工具可以通过本注册表查询
//! 「存在哪些简码层、各用什么候选语法、是否冻结」,不解析 README 文本。
//!
//! 注册表只描述身份与兼容语义,不复刻各层的选择算法(算法仍在各自的
//! production 模块;此处不做纯美观的重构)。

use crate::candidates::CandidateGrammar;

/// 静态简码生产层 policy 的 typed 身份。
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum ShortcutPolicyId {
    /// 一级简码固定层(26 键,显式设计数据,PR #20 前的冻结层)。
    Level1FrozenV1,
    /// 高稳健零冲突词语简码层(PR #22,冻结)。
    ZeroRegressionHighV1,
    /// 高稳健 FIXED_FIRST 词语简码层(PR #23,冻结)。
    FixedFirstHighV1,
    /// 二码零冲突词语简码层(本仓库 PR #24;仅空 2 键码)。
    TwoKeyZeroRegressionV1,
}

impl ShortcutPolicyId {
    /// 全部已注册 policy(稳定序:层叠顺序)。
    pub const ALL: [Self; 4] = [
        Self::Level1FrozenV1,
        Self::ZeroRegressionHighV1,
        Self::FixedFirstHighV1,
        Self::TwoKeyZeroRegressionV1,
    ];

    /// 报告用稳定标签。
    pub fn label(self) -> &'static str {
        match self {
            Self::Level1FrozenV1 => "level1-frozen-v1",
            Self::ZeroRegressionHighV1 => "zero-regression-high-v1",
            Self::FixedFirstHighV1 => "fixed-first-high-v1",
            Self::TwoKeyZeroRegressionV1 => "two-key-zero-regression-v1",
        }
    }

    /// 该层使用的候选语法。
    ///
    /// 一级简码与单字层不是 F/I 投影简码,没有候选语法(词层才有);
    /// level1 属于显式设计数据,返回 None 语义上正确。
    pub fn candidate_grammar(self) -> Option<CandidateGrammar> {
        match self {
            Self::Level1FrozenV1 => None,
            Self::ZeroRegressionHighV1 => Some(CandidateGrammar::LegacyAnyFiV1),
            Self::FixedFirstHighV1 | Self::TwoKeyZeroRegressionV1 => {
                Some(CandidateGrammar::MonotoneSuffixInitialsV2)
            }
        }
    }

    /// 该层是否为冻结的用户肌肉记忆兼容接口(发布后不得静默重生成)。
    pub fn is_frozen(self) -> bool {
        match self {
            Self::Level1FrozenV1
            | Self::ZeroRegressionHighV1
            | Self::FixedFirstHighV1
            | Self::TwoKeyZeroRegressionV1 => true,
        }
    }

    /// 该层简码的码长范围(报告/校验用)。
    pub fn shortcut_lengths(self) -> (usize, usize) {
        match self {
            Self::Level1FrozenV1 => (1, 1),
            Self::ZeroRegressionHighV1 => (3, 7),
            Self::FixedFirstHighV1 => (3, 7),
            Self::TwoKeyZeroRegressionV1 => (2, 2),
        }
    }
}

/// 一条 production 简码的窄只读元数据(Trainer / 工具的 canonical feed)。
///
/// 只携带 production semantic identity 相关字段;analyzer 的瞬态
/// utility/票数等 evidence 不属于本投影。
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct ShortcutMetadata {
    /// 候选文本(字或词)。
    pub text: String,
    /// 完整码(alias 语义,保留可用)。
    pub full_code: String,
    /// 简码。
    pub shortcut_code: String,
    /// 简码长度。
    pub shortcut_length: usize,
    /// 所属 policy 层。
    pub policy: ShortcutPolicyId,
    /// F/I 投影模式(level1 无模式,为 None)。
    pub mode: Option<String>,
    /// 层是否冻结(稳定肌肉记忆接口)。
    pub frozen: bool,
}

impl ShortcutMetadata {
    /// 按层构造词语简码元数据。
    pub fn word_shortcut(
        text: &str,
        full_code: &str,
        shortcut_code: &str,
        policy: ShortcutPolicyId,
        mode: &str,
    ) -> Self {
        ShortcutMetadata {
            text: text.to_string(),
            full_code: full_code.to_string(),
            shortcut_code: shortcut_code.to_string(),
            shortcut_length: shortcut_code.chars().count(),
            policy,
            mode: Some(mode.to_string()),
            frozen: policy.is_frozen(),
        }
    }
}

/// 全部 production 简码元数据投影(层叠顺序,层内 canonical 序)。
///
/// 这是 Trainer / 未来工具的唯一事实查询入口;数据源自各层已解析的
/// canonical TSV / 冻结层,不重新推导。
pub fn all_shortcut_metadata() -> Vec<ShortcutMetadata> {
    let mut out = Vec::new();
    // level1:键 → 汉字(无完整码语义,full_code 即简码键)。
    for entry in xhup_generator::canonical_level1_shortcuts() {
        let key = entry.key().as_char().to_string();
        out.push(ShortcutMetadata {
            text: entry.hanzi().as_char().to_string(),
            full_code: key.clone(),
            shortcut_code: key.clone(),
            shortcut_length: 1,
            policy: ShortcutPolicyId::Level1FrozenV1,
            mode: None,
            frozen: true,
        });
    }
    // ZR / FF / 2-key:canonical TSV 已解析的条目直读。
    for entry in xhup_generator::canonical_word_shortcut_entries() {
        out.push(ShortcutMetadata::word_shortcut(
            entry.word(),
            &entry.full_code().to_string(),
            &entry.shortcut_code().to_string(),
            ShortcutPolicyId::ZeroRegressionHighV1,
            entry.mode(),
        ));
    }
    for entry in xhup_generator::canonical_fixed_first_shortcut_entries() {
        out.push(ShortcutMetadata::word_shortcut(
            entry.word(),
            &entry.full_code().to_string(),
            &entry.shortcut_code().to_string(),
            ShortcutPolicyId::FixedFirstHighV1,
            entry.mode(),
        ));
    }
    // 二码零冲突层:generator canonical TSV 解析投影直读。
    for entry in xhup_generator::canonical_two_key_shortcut_entries() {
        out.push(ShortcutMetadata::word_shortcut(
            entry.word(),
            &entry.full_code().to_string(),
            &entry.shortcut_code().to_string(),
            ShortcutPolicyId::TwoKeyZeroRegressionV1,
            entry.mode(),
        ));
    }
    out
}
