//! XHUP Flow 领域核心:XHUP 键盘码的底层表示与校验。
//!
//! 本 crate 只提供最小的结构原语(按键、按键序列、双拼单元、全码),
//! 不涉及输入法语义、词典、组词或候选排序。
#![forbid(unsafe_code)]

mod code;
mod key;
mod sequence;

pub use code::{CodeError, DoublePinyinCode, FullCode};
pub use key::{InvalidKeyError, Key};
pub use sequence::{KeySequence, KeySequenceError};
