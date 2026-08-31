//! XHUP Flow 领域核心:XHUP 键盘码的底层表示与规范双拼布局。
//!
//! 内建规范小鹤双拼布局经 `include_str!` 嵌入仓库级项目数据
//! `data/double_pinyin/`(唯一事实来源);内建布局假定当前 XHUP Flow
//! 仓库的目录结构。
//!
//! 本 crate 不涉及输入法语义、词典、组词或候选排序。
#![forbid(unsafe_code)]

mod code;
mod key;
mod layout;
mod sequence;

pub use code::{CodeError, DoublePinyinCode, FullCode};
pub use key::{InvalidKeyError, Key};
pub use layout::{DoublePinyinLayout, FinalMapping, InitialMapping, ZeroInitialMapping};
pub use sequence::{KeySequence, KeySequenceError};
