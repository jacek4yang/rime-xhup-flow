//! XHUP Flow Rime 源数据生成器:把 `xhup-core` 的规范数据、规范频率数据与
//! 入库模板投影为确定性的便携 Rime 源包与训练器数据集。
//!
//! 便携包面向主流 librime 前端(ibus-rime、fcitx5-rime、Weasel、Squirrel、
//! fcitx5-macos、fcitx5-android 等),只使用标准核心组件,不依赖 Lua 或
//! 其他可选插件。当前提供一级简码词典(26 键,`xhup_flow_shortcuts`)、固定层
//! 静态单字词典(2/3/4 码,`xhup_flow_chars`)、固定层静态高频词语词典
//! (2~4 字词 4/6/8 键,`xhup_flow_words`)、顶层词典(`xhup_flow`)、方案
//! (`xhup_flow`)与训练器数据集(`xhup_flow_trainer.json`)的纯内存生成:
//! 不读写文件,也不读取任何既有 Rime 词典。一级简码、单字、词语词典分别投影
//! 各自的规范/最终化数据;训练器数据集目前仍仅投影单字训练数据。静态单字/词语
//! 条目各由唯一管线最终化(推导 → 去重 → 万象读音分数聚合 → 组内排名 →
//! 显式 Rime 权重);候选排名由显式权重表达,行/条目输出顺序仅是确定性的
//! 序列化顺序。一级简码只是一键精确候选,不自动上屏;固定层 4 键词码与规范
//! 单字全码集严格不相交,词层绝不改变既有 2/3/4 码单字的精确查表行为。在相同
//! 规范数据、相同频率/词语数据、相同 xhup-generator 源码(含其 package
//! version)与相同模板下,生成结果字节级一致。
#![forbid(unsafe_code)]

mod analysis;
mod char_codes;
mod fixed_first_shortcuts;
mod frequency;
mod package;
mod rime;
mod rime_fixed_first_shortcuts;
mod rime_shortcuts;
mod rime_two_key_shortcuts;
mod rime_word_shortcuts;
mod rime_words;
mod shortcuts;
mod trainer;
mod two_key_shortcuts;
mod word_codes;
mod word_shortcuts;
mod words;

pub use analysis::{
    CharCodeAnalysisEntry, WordCodeAnalysisEntry, char_code_analysis_entries,
    word_code_analysis_entries,
};
pub use char_codes::{RimeCharCodeEntry, canonical_char_code_entries};
pub use fixed_first_shortcuts::{
    CanonicalFixedFirstShortcutEntry, canonical_fixed_first_shortcut_entries,
};
pub use package::{RimeArtifact, generate_rime_artifacts};
pub use rime::{
    RIME_CHAR_DICTIONARY_FILENAME, RimeCharEntry, canonical_char_entries,
    generate_rime_char_dictionary,
};
pub use rime_fixed_first_shortcuts::{
    RIME_FIXED_FIRST_SHORTCUT_DICTIONARY_FILENAME, generate_rime_fixed_first_shortcut_dictionary,
};
pub use rime_shortcuts::{RIME_SHORTCUT_DICTIONARY_FILENAME, generate_rime_shortcut_dictionary};
pub use rime_two_key_shortcuts::{
    RIME_TWO_KEY_SHORTCUT_DICTIONARY_FILENAME, generate_rime_two_key_shortcut_dictionary,
};
pub use rime_word_shortcuts::{
    RIME_WORD_SHORTCUT_DICTIONARY_FILENAME, generate_rime_word_shortcut_dictionary,
};
pub use rime_words::{RIME_WORD_DICTIONARY_FILENAME, generate_rime_word_dictionary};
pub use shortcuts::{Level1ShortcutEntry, canonical_level1_shortcuts};
pub use trainer::{TRAINER_DATA_FILENAME, generate_trainer_dataset};
pub use two_key_shortcuts::{CanonicalTwoKeyShortcutEntry, canonical_two_key_shortcut_entries};
pub use word_codes::{RimeWordCodeEntry, canonical_word_code_entries};
pub use word_shortcuts::{CanonicalWordShortcutEntry, canonical_word_shortcut_entries};
