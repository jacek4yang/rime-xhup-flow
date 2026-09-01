//! XHUP Flow Rime 源数据生成器:把 `xhup-core` 的规范数据投影为确定性的
//! Rime 源词典文本。
//!
//! 当前提供规范单字全码词典(`xhup_flow_chars`)的纯内存生成:不读写文件,
//! 也不读取任何既有 Rime 词典。生成条目的顺序只是确定性的序列化顺序,
//! 不代表 Rime 候选排序、频率或首选读音/形码;同一规范数据与同一 crate
//! 版本必然产生字节级一致的输出。
#![forbid(unsafe_code)]

mod rime;

pub use rime::{RimeCharEntry, canonical_char_entries, generate_rime_char_dictionary};
