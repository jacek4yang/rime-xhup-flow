//! XHUP Flow Rime 源数据生成器:把 `xhup-core` 的规范数据、规范频率数据与
//! 入库模板投影为确定性的便携 Rime 源包与训练器数据集。
//!
//! 便携包面向主流 librime 前端(ibus-rime、fcitx5-rime、Weasel、Squirrel、
//! fcitx5-macos、fcitx5-android 等);输入行为只依赖标准核心组件,Lua
//! 简码提示为可选增强(librime-lua 缺失时自动降级为纯静态行为)。
//! 当前提供一级简码词典(26 键,`xhup_flow_shortcuts`)、固定层
//! 静态单字词典(2/3/4 码,`xhup_flow_chars`)、固定层静态高频词语词典
//! (2~4 字词 4/6/8 键,`xhup_flow_words`)、顶层词典(`xhup_flow`)、方案
//! (`xhup_flow`)与训练器数据集(`xhup_flow_trainer.json`)的纯内存生成:
//! 不读写文件,也不读取任何既有 Rime 词典。一级简码、单字、词语词典分别投影
//! 各自的规范/最终化数据;训练器数据集目前仍仅投影单字训练数据。静态单字/词语
//! 条目各由唯一管线最终化(推导 → 去重 → 万象读音分数聚合 → 组内排名 →
//! 显式 Rime 权重);候选排名由显式权重表达,行/条目输出顺序仅是确定性的
//! 序列化顺序。一级简码只是一键精确候选,不自动上屏。固定层 4 键词码可能与
//! 规范单字全码碰撞:词与字在同码上合法共存,词汇存在性绝不因碰撞被剥夺;
//! 碰撞码的候选次序由 `merged_ranking` 按同源万象频率证据跨表仲裁(权重唯一、
//! 无平局),非碰撞码行为字节级不变。在相同
//! 规范数据、相同频率/词语数据、相同 xhup-generator 源码(含其 package
//! version)与相同模板下,生成结果字节级一致。
#![forbid(unsafe_code)]

mod analysis;
mod char_codes;
mod fixed_first_shortcuts;
mod frequency;
mod lua_hints;
mod merged_ranking;
mod package;
mod primary_shortcuts;
mod rime;
mod rime_fixed_first_shortcuts;
mod rime_flow;
mod rime_shortcuts;
mod rime_word_shortcuts;
mod rime_words;
mod shortcuts;
mod trainer;
mod two_key_shortcuts;
mod word_codes;
mod word_shortcuts;
mod words;

/// 各简码层的原始 TSV 词/码集合(纯文本扫描,**不经过校验管线**)。
///
/// PRIMARY/FIXED_FIRST 项仅供跨层引导与 canonical 再生成流程使用;
/// zero_regression/two_key 项是 legacy v1 research-only fixture。再生成某一简码层时,磁盘上的
/// 兄弟层 TSV 可能处于与新数据不一致的中间态,校验管线会按设计 panic;
/// 原始文本扫描永远可用。生产逻辑应使用 canonical 投影(经完整校验)。
pub mod raw_shortcuts {
    pub use crate::fixed_first_shortcuts::{
        raw_shortcut_codes as fixed_first_codes, raw_words as fixed_first_words,
    };
    pub use crate::primary_shortcuts::{
        raw_shortcut_codes as primary_codes, raw_words as primary_words,
    };
    pub use crate::two_key_shortcuts::raw_words as two_key_words;
    pub use crate::word_shortcuts::{
        raw_shortcut_codes as zero_regression_codes, raw_words as zero_regression_words,
    };
}

pub use analysis::{
    CharCodeAnalysisEntry, WordCodeAnalysisEntry, char_code_analysis_entries,
    word_code_analysis_entries,
};
pub use char_codes::{RimeCharCodeEntry, canonical_char_code_entries};
pub use fixed_first_shortcuts::{
    CanonicalFixedFirstShortcutEntry, canonical_fixed_first_shortcut_entries,
    legacy_v1_fixed_first_shortcut_entries,
};
pub use lua_hints::{
    LUA_QUICK_HINT_DATA_FILENAME, LUA_QUICK_HINT_FILENAME, generate_lua_quick_hints_data,
    lua_quick_hint_source,
};
pub use package::{RimeArtifact, generate_rime_artifacts};
pub use primary_shortcuts::{CanonicalPrimaryShortcutEntry, canonical_primary_shortcut_entries};
pub use rime::{
    RIME_CHAR_DICTIONARY_FILENAME, RimeCharEntry, canonical_char_entries,
    generate_rime_char_dictionary,
};
pub use rime_fixed_first_shortcuts::{
    RIME_FIXED_FIRST_SHORTCUT_DICTIONARY_FILENAME, generate_rime_fixed_first_shortcut_dictionary,
};
pub use rime_flow::{
    FLOW_ENCODER_MAX_PHRASE_LENGTH, RIME_FLOW_DICTIONARY_FILENAME, RIME_LEARN_DICTIONARY_FILENAME,
    flow_encoder_yaml, generate_rime_flow_dictionary, generate_rime_learn_dictionary,
};
pub use rime_shortcuts::{RIME_SHORTCUT_DICTIONARY_FILENAME, generate_rime_shortcut_dictionary};
pub use rime_word_shortcuts::{
    RIME_WORD_SHORTCUT_DICTIONARY_FILENAME, generate_rime_word_shortcut_dictionary,
};
pub use rime_words::{RIME_WORD_DICTIONARY_FILENAME, generate_rime_word_dictionary};
pub use shortcuts::{Level1ShortcutEntry, canonical_level1_shortcuts};
pub use trainer::{TRAINER_DATA_FILENAME, generate_trainer_dataset};
pub use two_key_shortcuts::{LegacyV1TwoKeyShortcutEntry, legacy_v1_two_key_shortcut_entries};
pub use word_codes::{RimeWordCodeEntry, canonical_word_code_entries};
pub use word_shortcuts::{LegacyV1WordShortcutEntry, legacy_v1_word_shortcut_entries};
