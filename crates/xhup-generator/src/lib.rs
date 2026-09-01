//! XHUP Flow Rime 源数据生成器:把 `xhup-core` 的规范数据与入库模板投影为
//! 确定性的便携 Rime 源包。
//!
//! 便携包面向主流 librime 前端(ibus-rime、fcitx5-rime、Weasel、Squirrel、
//! fcitx5-macos、fcitx5-android 等),只使用标准核心组件,不依赖 Lua 或
//! 其他可选插件。当前提供规范单字全码词典(`xhup_flow_chars`)、顶层词典
//! (`xhup_flow`)与方案(`xhup_flow`)的纯内存生成:不读写文件,也不读取
//! 任何既有 Rime 词典。生成条目的顺序只是确定性的序列化顺序,不代表
//! Rime 候选排序、频率或首选读音/形码。在相同规范数据、相同
//! xhup-generator 源码(含其 package version)与相同模板下,生成结果
//! 字节级一致。
#![forbid(unsafe_code)]

mod package;
mod rime;

pub use package::{RimeArtifact, generate_rime_artifacts};
pub use rime::{
    RIME_CHAR_DICTIONARY_FILENAME, RimeCharEntry, canonical_char_entries,
    generate_rime_char_dictionary,
};
