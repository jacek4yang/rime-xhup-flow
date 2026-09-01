//! 非空合法按键序列及其错误类型。

use std::error::Error;
use std::fmt;
use std::fmt::Write as _;
use std::slice;
use std::str::FromStr;

use crate::key::{InvalidKeyError, Key};

/// 非空的合法按键序列。
///
/// 长度没有上限:未来的连续双拼输入可能包含超过四键的序列。
/// 序列不可变;本层不提供编辑/修改 API。
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct KeySequence(Box<[Key]>);

impl KeySequence {
    /// 由按键切片构造:空切片返回 [`KeySequenceError::Empty`],
    /// 非空切片拷贝为独立的不可变序列。
    pub fn from_keys(keys: &[Key]) -> Result<Self, KeySequenceError> {
        if keys.is_empty() {
            return Err(KeySequenceError::Empty);
        }
        Ok(Self(keys.to_vec().into_boxed_slice()))
    }

    /// 序列长度(键数)。保证不小于 1。
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// 序列按不变量永不为空;此方法仅为 API 完整性存在,恒返回 `false`。
    pub fn is_empty(&self) -> bool {
        false
    }

    /// 迭代序列中的按键。
    pub fn iter(&self) -> slice::Iter<'_, Key> {
        self.0.iter()
    }

    /// 以切片形式访问按键。
    pub fn as_slice(&self) -> &[Key] {
        &self.0
    }
}

impl FromStr for KeySequence {
    type Err = KeySequenceError;

    /// 从字符串解析:空输入返回 [`KeySequenceError::Empty`],
    /// 任一字符非法时立即返回 [`KeySequenceError::InvalidKey`]。
    fn from_str(input: &str) -> Result<Self, Self::Err> {
        if input.is_empty() {
            return Err(KeySequenceError::Empty);
        }
        let mut keys = Vec::with_capacity(input.len());
        for ch in input.chars() {
            keys.push(Key::from_char(ch).map_err(KeySequenceError::InvalidKey)?);
        }
        Ok(Self(keys.into_boxed_slice()))
    }
}

impl fmt::Display for KeySequence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for key in self.0.iter() {
            f.write_char(key.as_char())?;
        }
        Ok(())
    }
}

/// 按键序列解析错误。
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub enum KeySequenceError {
    /// 输入为空;序列至少包含一个按键。
    Empty,
    /// 序列包含非法字符。
    InvalidKey(InvalidKeyError),
}

impl fmt::Display for KeySequenceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "按键序列不能为空"),
            Self::InvalidKey(err) => write!(f, "按键序列包含{err}"),
        }
    }
}

impl Error for KeySequenceError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Empty => None,
            Self::InvalidKey(err) => Some(err),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_single_key_sequence() {
        let seq: KeySequence = "a".parse().unwrap();
        assert_eq!(seq.len(), 1);
        assert!(!seq.is_empty());
        assert_eq!(seq.as_slice(), &[Key::from_char('a').unwrap()]);
    }

    #[test]
    fn accepts_multi_key_sequence() {
        let seq: KeySequence = "xhup".parse().unwrap();
        assert_eq!(seq.len(), 4);
        let chars: Vec<char> = seq.iter().map(|k| k.as_char()).collect();
        assert_eq!(chars, ['x', 'h', 'u', 'p']);
    }

    #[test]
    fn accepts_long_sequence_beyond_four_keys() {
        let seq: KeySequence = "xhupshurufanganquanmayouxian".parse().unwrap();
        assert!(seq.len() > 4);
    }

    #[test]
    fn rejects_empty_input() {
        assert_eq!("".parse::<KeySequence>(), Err(KeySequenceError::Empty));
    }

    #[test]
    fn rejects_any_invalid_character() {
        for input in ["ab1", "AB", "a b", "中", "abc/", "xh\thp"] {
            assert!(matches!(
                input.parse::<KeySequence>(),
                Err(KeySequenceError::InvalidKey(_))
            ));
        }
    }

    #[test]
    fn from_keys_rejects_empty_slice() {
        assert_eq!(KeySequence::from_keys(&[]), Err(KeySequenceError::Empty));
    }

    #[test]
    fn from_keys_accepts_one_to_four_keys() {
        let keys: Vec<Key> = ('a'..='d').map(|ch| Key::from_char(ch).unwrap()).collect();
        for len in 1..=4 {
            let seq = KeySequence::from_keys(&keys[..len]).unwrap();
            assert_eq!(seq.len(), len);
            assert_eq!(seq.as_slice(), &keys[..len]);
        }
    }

    #[test]
    fn from_keys_copies_into_independent_sequence() {
        let keys = [Key::from_char('x').unwrap(), Key::from_char('k').unwrap()];
        let seq = KeySequence::from_keys(&keys).unwrap();
        assert_eq!(seq.to_string(), "xk");
        let reparsed: KeySequence = seq.to_string().parse().unwrap();
        assert_eq!(reparsed, seq);
    }

    #[test]
    fn display_round_trips() {
        let seq: KeySequence = "xhup".parse().unwrap();
        assert_eq!(seq.to_string(), "xhup");
        let reparsed: KeySequence = seq.to_string().parse().unwrap();
        assert_eq!(reparsed, seq);
    }
}
