//! XHUP Flow Rime 源数据生成器:把 `xhup-core` 的规范数据投影为确定性的
//! Rime 源词典文本。
//!
//! 当前提供规范单字全码词典(`xhup_flow_chars`)的纯内存生成:不读写文件,
//! 也不读取任何既有 Rime 词典。生成条目的顺序只是确定性的序列化顺序,
//! 不代表 Rime 候选排序、频率或首选读音/形码。在相同规范数据与相同
//! xhup-generator 源码(含其 package version)下,生成结果字节级一致。
#![forbid(unsafe_code)]

mod rime;

pub use rime::{
    RIME_CHAR_DICTIONARY_FILENAME, RimeCharEntry, canonical_char_entries,
    generate_rime_char_dictionary,
};
