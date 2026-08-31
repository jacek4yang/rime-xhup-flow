//! XHUP Flow 领域核心:XHUP 键盘码的底层表示、规范双拼布局与规范输入音节清单。
//!
//! 内建规范小鹤双拼布局与规范输入音节清单经 `include_str!` 嵌入仓库级项目数据
//! `data/double_pinyin/`、`data/pinyin/`(唯一事实来源);内建数据假定当前 XHUP Flow
//! 仓库的目录结构。
//!
//! 本 crate 不涉及词典、组词、候选排序等更高层输入逻辑。
#![forbid(unsafe_code)]

mod code;
mod input_syllable;
mod key;
mod layout;
mod sequence;

pub use code::{CodeError, DoublePinyinCode, FullCode};
pub use input_syllable::{XhupInputSyllable, XhupInputSyllableError};
pub use key::{InvalidKeyError, Key};
pub use layout::{DoublePinyinLayout, FinalMapping, InitialMapping, ZeroInitialMapping};
pub use sequence::{KeySequence, KeySequenceError};
