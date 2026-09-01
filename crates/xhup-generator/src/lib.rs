//! XHUP Flow Rime 源数据生成器:把 `xhup-core` 的规范数据、规范频率数据与
//! 入库模板投影为确定性的便携 Rime 源包与训练器数据集。
//!
//! 便携包面向主流 librime 前端(ibus-rime、fcitx5-rime、Weasel、Squirrel、
//! fcitx5-macos、fcitx5-android 等),只使用标准核心组件,不依赖 Lua 或
//! 其他可选插件。当前提供固定层静态单字词典(2/3/4 码,`xhup_flow_chars`)、
//! 固定层静态高频词语词典(2~4 字词 4/6/8 键,`xhup_flow_words`)、顶层词典
//! (`xhup_flow`)、方案(`xhup_flow`)与训练器数据集(`xhup_flow_trainer.json`)
//! 的纯内存生成:不读写文件,也不读取任何既有 Rime 词典。静态单字/词语条目各由
//! 唯一管线最终化(推导 → 去重 → 万象读音分数聚合 → 组内排名 → 显式 Rime
//! 权重),Rime 词典与训练器 JSON 都是最终化数据的投影;候选排名由显式权重表达,
//! 行/条目输出顺序仅是确定性的序列化顺序。固定层 4 键词码与规范单字全码集严格
//! 不相交,词层绝不改变既有 2/3/4 码单字的精确查表行为。在相同规范数据、相同
//! 频率/词语数据、相同 xhup-generator 源码(含其 package version)与相同模板下,
//! 生成结果字节级一致。
#![forbid(unsafe_code)]

mod char_codes;
mod frequency;
mod package;
mod rime;
mod rime_words;
mod trainer;
mod word_codes;
mod words;

pub use char_codes::{RimeCharCodeEntry, canonical_char_code_entries};
pub use package::{RimeArtifact, generate_rime_artifacts};
pub use rime::{
    RIME_CHAR_DICTIONARY_FILENAME, RimeCharEntry, canonical_char_entries,
    generate_rime_char_dictionary,
};
pub use rime_words::{RIME_WORD_DICTIONARY_FILENAME, generate_rime_word_dictionary};
pub use trainer::{TRAINER_DATA_FILENAME, generate_trainer_dataset};
pub use word_codes::{RimeWordCodeEntry, canonical_word_code_entries};
