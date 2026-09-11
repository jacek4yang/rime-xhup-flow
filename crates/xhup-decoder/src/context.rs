//! 隐私安全的运行时上下文快照。

use std::fmt;

use xhup_core::KeySequence;

/// 一次解码查询所需的最小上下文。
///
/// committed left context 与当前未提交按键明确分层。类型自身不做日志输出；
/// [`Debug`] 只显示长度，避免诊断日志意外泄露用户文本。
#[derive(Clone, Eq, PartialEq)]
pub struct RuntimeContext {
    committed_left: Box<str>,
    composition: KeySequence,
}

impl RuntimeContext {
    /// 创建不可变上下文快照。左上下文允许为空；composition 由
    /// [`KeySequence`] 保证非空且只含合法按键。
    pub fn new(committed_left: impl Into<Box<str>>, composition: KeySequence) -> Self {
        Self {
            committed_left: committed_left.into(),
            composition,
        }
    }

    /// 已提交左上下文。调用方不得在普通日志中记录该值。
    pub fn committed_left(&self) -> &str {
        &self.committed_left
    }

    /// 当前未提交原始按键。
    pub fn composition(&self) -> &KeySequence {
        &self.composition
    }

    /// 已提交左上下文的 Unicode 标量数，用于有界窗口与诊断。
    pub fn committed_left_chars(&self) -> usize {
        self.committed_left.chars().count()
    }
}

impl fmt::Debug for RuntimeContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RuntimeContext")
            .field("committed_left_chars", &self.committed_left_chars())
            .field("composition_keys", &self.composition.len())
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_redacts_user_text_and_raw_keys() {
        let context = RuntimeContext::new("这是私密上下文", "yjjqugmk".parse().unwrap());
        let debug = format!("{context:?}");
        assert!(debug.contains("committed_left_chars: 7"));
        assert!(debug.contains("composition_keys: 8"));
        assert!(!debug.contains("私密"));
        assert!(!debug.contains("yjjq"));
    }
}
