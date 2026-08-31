//! 单个合法 XHUP 输入按键及其错误类型。

use std::error::Error;
use std::fmt;
use std::fmt::Write as _;

/// 一个合法的 XHUP 输入按键:恰好一个小写 ASCII 字母(`a`–`z`)。
///
/// 内部以 ASCII 字节存储,紧凑且不可变;非法按键无法经公开 API 构造。
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct Key(u8);

impl Key {
    /// 从字符构造 `Key`;仅接受小写 ASCII 字母,其余一律返回 [`InvalidKeyError`]。
    pub const fn from_char(ch: char) -> Result<Self, InvalidKeyError> {
        if ch.is_ascii_lowercase() {
            Ok(Self(ch as u8))
        } else {
            Err(InvalidKeyError(ch))
        }
    }

    /// 返回对应的小写字母字符。
    pub const fn as_char(self) -> char {
        self.0 as char
    }

    /// 返回对应的 ASCII 字节。
    pub const fn as_byte(self) -> u8 {
        self.0
    }
}

impl TryFrom<char> for Key {
    type Error = InvalidKeyError;

    fn try_from(ch: char) -> Result<Self, Self::Error> {
        Self::from_char(ch)
    }
}

impl From<Key> for char {
    fn from(key: Key) -> Self {
        key.as_char()
    }
}

impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_char(self.as_char())
    }
}

/// 非法按键字符:输入不是小写 ASCII 字母。
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct InvalidKeyError(char);

impl InvalidKeyError {
    /// 导致错误的原始字符。
    pub const fn character(self) -> char {
        self.0
    }
}

impl fmt::Display for InvalidKeyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "无效按键:{:?}(应为小写 ASCII 字母 a-z)", self.0)
    }
}

impl Error for InvalidKeyError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_all_lowercase_ascii_letters() {
        for ch in 'a'..='z' {
            let key = Key::from_char(ch).unwrap();
            assert_eq!(key.as_char(), ch);
        }
    }

    #[test]
    fn boundary_letters_accepted() {
        assert_eq!(Key::from_char('a').unwrap().as_byte(), b'a');
        assert_eq!(Key::from_char('z').unwrap().as_byte(), b'z');
    }

    #[test]
    fn rejects_invalid_characters() {
        for ch in ['A', 'Z', '0', '9', '/', '.', ' ', '\t', '\n', '中', 'ä'] {
            let err = Key::from_char(ch).unwrap_err();
            assert_eq!(err.character(), ch);
        }
    }

    #[test]
    fn char_conversions_round_trip() {
        let key = Key::try_from('x').unwrap();
        let ch: char = key.into();
        assert_eq!(ch, 'x');
        assert_eq!(Key::from_char(ch), Ok(key));
    }

    #[test]
    fn display_writes_the_letter() {
        assert_eq!(Key::from_char('q').unwrap().to_string(), "q");
    }
}
