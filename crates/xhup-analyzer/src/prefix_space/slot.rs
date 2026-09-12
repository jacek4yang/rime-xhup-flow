//! Prefix-Space 候选位资源模型。
//!
//! 核心不变量:
//! 1. valid prefix != commit boundary: 有效短前缀不因首选候选而自动提交。
//! 2. 码空间不被单个单词独占: 码位是有序槽位 `(code, rank)`。
//! 3. 一个目标可跨多个前缀码长以不同位次可达。

use xhup_core::KeySequence;

/// 候选位放置的来源分类。
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum SlotPlacementSource {
    /// 一级简码冻结层(26 键显式数据)。
    Level1Frozen,
    /// 固定单字全码或声形规范码。
    FixedChar,
    /// 固定词语规范全码。
    FixedWord,
    /// 官方小鹤别名或特殊规则码。
    OfficialRule,
    /// 历史/既有兼容简码(v1/v2 继承)。
    LegacyShortcut,
    /// Flow 扩展前缀简码(由 v3 前缀空间编译器全局分配)。
    FlowExtension,
}

impl SlotPlacementSource {
    /// 来源展示标识。
    pub fn label(self) -> &'static str {
        match self {
            Self::Level1Frozen => "LEVEL1_FROZEN",
            Self::FixedChar => "FIXED_CHAR",
            Self::FixedWord => "FIXED_WORD",
            Self::OfficialRule => "OFFICIAL_RULE",
            Self::LegacyShortcut => "LEGACY_SHORTCUT",
            Self::FlowExtension => "FLOW_EXTENSION",
        }
    }

    /// 是否为不可被编译器替换的冻结层。
    pub fn is_frozen(self) -> bool {
        matches!(
            self,
            Self::Level1Frozen | Self::FixedChar | Self::FixedWord | Self::OfficialRule
        )
    }
}

/// 放置在指定前缀节点槽位上的候选条目。
#[derive(Clone, Debug, PartialEq)]
pub struct SlotCandidate {
    /// 候选文本(字或词)。
    pub text: String,
    /// 目标规范全码(保留可用;省键与可达性基准)。
    pub full_code: KeySequence,
    /// 当前前缀码位上的组内名次(1-based, 1 = 首选)。
    pub rank: usize,
    /// 放置来源。
    pub source: SlotPlacementSource,
    /// 综合频率质量(无量纲中位锚定)。
    pub mass: f64,
    /// 是否为后续输入的前缀延续点(true 表示后接子树有更多输入)。
    pub is_continuation: bool,
    /// 是否受保护冻结。
    pub is_frozen: bool,
}

impl SlotCandidate {
    /// 创建新槽位候选。
    pub fn new(
        text: impl Into<String>,
        full_code: KeySequence,
        rank: usize,
        source: SlotPlacementSource,
        mass: f64,
        is_continuation: bool,
    ) -> Self {
        let is_frozen = source.is_frozen();
        Self {
            text: text.into(),
            full_code,
            rank,
            source,
            mass,
            is_continuation,
            is_frozen,
        }
    }
}

/// 前缀树上的候选槽位标识: (code, rank)。
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct CandidateSlotKey {
    /// 前缀按键序列。
    pub code: KeySequence,
    /// 候选名次(1 = 首选)。
    pub rank: usize,
}

impl CandidateSlotKey {
    /// 构造槽位标识。
    pub fn new(code: KeySequence, rank: usize) -> Self {
        Self { code, rank }
    }
}

/// 槽位资源状态(用于评估竞争与成本)。
#[derive(Clone, Debug, PartialEq)]
pub struct CandidateSlotState {
    /// 槽位键。
    pub key: CandidateSlotKey,
    /// 码内已占用候选数。
    pub occupant_count: usize,
    /// 码内总占用质量(竞争强度)。
    pub occupant_mass: f64,
    /// 插入该槽位将被挤后的候选质量。
    pub displaced_mass: f64,
    /// 该前缀当前累积的前缀拥塞度。
    pub congestion: f64,
}
