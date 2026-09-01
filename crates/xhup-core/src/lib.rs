//! XHUP Flow 领域核心:XHUP 键盘码的底层表示、规范双拼布局、规范输入音节清单、音节编码、规范汉字读音与规范汉字形码。
//!
//! 内建规范小鹤双拼布局、规范输入音节清单、规范汉字读音表与规范汉字形码表经 `include_str!`
//! 嵌入仓库级项目数据 `data/double_pinyin/`、`data/pinyin/`、`data/hanzi/`、`data/shape/`
//! (唯一事实来源);内建数据假定当前 XHUP Flow 仓库的目录结构。
//!
//! 本 crate 不涉及词典、组词、候选排序等更高层输入逻辑。
#![forbid(unsafe_code)]

mod code;
mod encoder;
mod hanzi;
mod input_syllable;
mod key;
mod layout;
mod sequence;
mod shape;

pub use code::{CodeError, DoublePinyinCode, FullCode, ShapeCode};
pub use hanzi::{HanziReading, XhupHanzi, XhupHanziError};
pub use input_syllable::{XhupInputSyllable, XhupInputSyllableError};
pub use key::{InvalidKeyError, Key};
pub use layout::{DoublePinyinLayout, FinalMapping, InitialMapping, ZeroInitialMapping};
pub use sequence::{KeySequence, KeySequenceError};
