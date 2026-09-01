//! 定长编码:双拼单元(2 键)与全码(4 键)的结构表示。

use std::error::Error;
use std::fmt;
use std::fmt::Write as _;
use std::str::FromStr;

use crate::key::{InvalidKeyError, Key};

/// 定长编码解析错误。
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub enum CodeError {
    /// 编码长度不符。
    InvalidLength {
        /// 期望的键数。
        expected: usize,
        /// 实际键数(字符数,而非 UTF-8 字节数)。
        actual: usize,
    },
    /// 编码包含非法字符。
    InvalidKey(InvalidKeyError),
}

impl fmt::Display for CodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidLength { expected, actual } => {
                write!(f, "编码长度应为 {expected} 键,实际为 {actual} 键")
            }
            Self::InvalidKey(err) => write!(f, "编码包含{err}"),
        }
    }
}

impl Error for CodeError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidLength { .. } => None,
            Self::InvalidKey(err) => Some(err),
        }
    }
}

/// 按定长编码规则解析输入:先逐字符转换为 [`Key`],任一字符非法立即返回
/// [`CodeError::InvalidKey`];全部合法后再校验键数。
fn parse_fixed<const N: usize>(input: &str) -> Result<[Key; N], CodeError> {
    let mut keys = Vec::with_capacity(N);
    for ch in input.chars() {
        keys.push(Key::from_char(ch).map_err(CodeError::InvalidKey)?);
    }
    let actual = keys.len();
    if actual != N {
        return Err(CodeError::InvalidLength {
            expected: N,
            actual,
        });
    }
    Ok(<[Key; N]>::try_from(keys.as_slice()).expect("长度已校验"))
}

fn write_keys<const N: usize>(keys: &[Key; N], f: &mut fmt::Formatter<'_>) -> fmt::Result {
    for key in keys {
        f.write_char(key.as_char())?;
    }
    Ok(())
}

/// 双拼编码单元:恰好两个合法按键。
///
/// 仅表示结构上的双键双拼单元,不涉及小鹤声母/韵母的实际映射。
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct DoublePinyinCode([Key; 2]);

impl DoublePinyinCode {
    /// 由两个合法按键构造。
    pub const fn new(keys: [Key; 2]) -> Self {
        Self(keys)
    }

    /// 以切片形式访问两个按键。
    pub fn as_slice(&self) -> &[Key] {
        &self.0
    }
}

impl FromStr for DoublePinyinCode {
    type Err = CodeError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        parse_fixed(input).map(Self)
    }
}

impl fmt::Display for DoublePinyinCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_keys(&self.0, f)
    }
}

/// 形码单元:恰好两个合法按键。
///
/// 仅表示结构上的双键形码单元;不表示该形码已实际指派给某个规范汉字
/// (规范指派关系见 `XhupHanzi::shape_codes()`)。
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct ShapeCode([Key; 2]);

impl ShapeCode {
    /// 由两个合法按键构造。
    pub const fn new(keys: [Key; 2]) -> Self {
        Self(keys)
    }

    /// 以切片形式访问两个按键。
    pub fn as_slice(&self) -> &[Key] {
        &self.0
    }
}

impl FromStr for ShapeCode {
    type Err = CodeError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        parse_fixed(input).map(Self)
    }
}

impl fmt::Display for ShapeCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_keys(&self.0, f)
    }
}

/// 全码:恰好四个合法按键。
///
/// 仅表示结构上的四键全码,不涉及拆字、重码或候选排序规则。
///
/// 组合/分解语义:前两个键为双拼音码、后两个键为形码,见
/// [`FullCode::from_parts`]。该关系是纯粹的结构拼接,`FullCode` 本身不做
/// 规范音码/形码的成员校验。
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct FullCode([Key; 4]);

impl FullCode {
    /// 由四个合法按键构造。
    pub const fn new(keys: [Key; 4]) -> Self {
        Self(keys)
    }

    /// 以切片形式访问四个按键。
    pub fn as_slice(&self) -> &[Key] {
        &self.0
    }

    /// 由双拼音码与形码组合成全码:前两键为音码,后两键为形码。
    ///
    /// 全函数、零分配;两个来源类型已各自保证恰好两个合法按键。
    pub const fn from_parts(double_pinyin: DoublePinyinCode, shape: ShapeCode) -> Self {
        let [a, b] = double_pinyin.0;
        let [c, d] = shape.0;
        Self([a, b, c, d])
    }

    /// 全码的前两键:双拼音码部分。
    pub const fn double_pinyin_code(self) -> DoublePinyinCode {
        DoublePinyinCode::new([self.0[0], self.0[1]])
    }

    /// 全码的后两键:形码部分。
    pub const fn shape_code(self) -> ShapeCode {
        ShapeCode::new([self.0[2], self.0[3]])
    }
}

impl FromStr for FullCode {
    type Err = CodeError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        parse_fixed(input).map(Self)
    }
}

impl fmt::Display for FullCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_keys(&self.0, f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn invalid_key(ch: char) -> CodeError {
        CodeError::InvalidKey(Key::from_char(ch).unwrap_err())
    }

    #[test]
    fn double_pinyin_accepts_exactly_two_valid_keys() {
        let code: DoublePinyinCode = "xh".parse().unwrap();
        let keys = [Key::from_char('x').unwrap(), Key::from_char('h').unwrap()];
        assert_eq!(code, DoublePinyinCode::new(keys));
        assert_eq!(code.as_slice(), &keys);
        assert_eq!(code.to_string(), "xh");
    }

    #[test]
    fn double_pinyin_rejects_wrong_lengths() {
        for (input, actual) in [("", 0), ("a", 1), ("abc", 3), ("abcdef", 6)] {
            assert_eq!(
                input.parse::<DoublePinyinCode>(),
                Err(CodeError::InvalidLength {
                    expected: 2,
                    actual
                })
            );
        }
    }

    #[test]
    fn double_pinyin_reports_invalid_char_before_length() {
        // 长度也不符,但应优先报告非法字符
        assert_eq!("ab!".parse::<DoublePinyinCode>(), Err(invalid_key('!')));
        assert_eq!("!".parse::<DoublePinyinCode>(), Err(invalid_key('!')));
        assert_eq!("中".parse::<DoublePinyinCode>(), Err(invalid_key('中')));
    }

    #[test]
    fn double_pinyin_display_round_trips() {
        let code: DoublePinyinCode = "xh".parse().unwrap();
        let reparsed: DoublePinyinCode = code.to_string().parse().unwrap();
        assert_eq!(reparsed, code);
    }

    #[test]
    fn shape_code_accepts_exactly_two_valid_keys() {
        let code: ShapeCode = "kk".parse().unwrap();
        let keys = [Key::from_char('k').unwrap(), Key::from_char('k').unwrap()];
        assert_eq!(code, ShapeCode::new(keys));
        assert_eq!(code.as_slice(), &keys);
        assert_eq!(code.to_string(), "kk");
    }

    #[test]
    fn shape_code_rejects_wrong_lengths() {
        for (input, actual) in [("", 0), ("k", 1), ("kkd", 3)] {
            assert_eq!(
                input.parse::<ShapeCode>(),
                Err(CodeError::InvalidLength {
                    expected: 2,
                    actual
                })
            );
        }
    }

    #[test]
    fn shape_code_reports_invalid_char_before_length() {
        assert_eq!("k!".parse::<ShapeCode>(), Err(invalid_key('!')));
        assert_eq!("中".parse::<ShapeCode>(), Err(invalid_key('中')));
    }

    #[test]
    fn shape_code_display_round_trips() {
        let code: ShapeCode = "kd".parse().unwrap();
        let reparsed: ShapeCode = code.to_string().parse().unwrap();
        assert_eq!(reparsed, code);
    }

    #[test]
    fn full_code_accepts_exactly_four_valid_keys() {
        let code: FullCode = "xhup".parse().unwrap();
        let keys = [
            Key::from_char('x').unwrap(),
            Key::from_char('h').unwrap(),
            Key::from_char('u').unwrap(),
            Key::from_char('p').unwrap(),
        ];
        assert_eq!(code, FullCode::new(keys));
        assert_eq!(code.as_slice(), &keys);
        assert_eq!(code.to_string(), "xhup");
    }

    #[test]
    fn full_code_rejects_wrong_lengths() {
        for (input, actual) in [("", 0), ("a", 1), ("abc", 3), ("abcde", 5)] {
            assert_eq!(
                input.parse::<FullCode>(),
                Err(CodeError::InvalidLength {
                    expected: 4,
                    actual
                })
            );
        }
    }

    #[test]
    fn full_code_reports_invalid_char_before_length() {
        assert_eq!("abc!".parse::<FullCode>(), Err(invalid_key('!')));
        assert_eq!("!".parse::<FullCode>(), Err(invalid_key('!')));
    }

    #[test]
    fn full_code_display_round_trips() {
        let code: FullCode = "xhup".parse().unwrap();
        let reparsed: FullCode = code.to_string().parse().unwrap();
        assert_eq!(reparsed, code);
    }

    #[test]
    fn full_code_composes_from_sound_and_shape() {
        // 音码前两键 + 形码后两键
        for (sound, shape, expected) in [
            ("vh", "mt", "vhmt"),
            ("aa", "aa", "aaaa"),
            ("zz", "zz", "zzzz"),
        ] {
            let sound: DoublePinyinCode = sound.parse().unwrap();
            let shape: ShapeCode = shape.parse().unwrap();
            let full = FullCode::from_parts(sound, shape);
            assert_eq!(full.to_string(), expected);
            assert_eq!(full, expected.parse().unwrap());
            assert_eq!(full.double_pinyin_code(), sound);
            assert_eq!(full.shape_code(), shape);
        }
    }

    #[test]
    fn full_code_composition_round_trips_exhaustively() {
        // 26^4 = 456,976 个结构值:组合后分解必然还原,分解后组合必然还原
        let keys: Vec<Key> = ('a'..='z').map(|ch| Key::from_char(ch).unwrap()).collect();
        for &a in &keys {
            for &b in &keys {
                let sound = DoublePinyinCode::new([a, b]);
                for &c in &keys {
                    for &d in &keys {
                        let shape = ShapeCode::new([c, d]);
                        let full = FullCode::from_parts(sound, shape);
                        assert_eq!(full.double_pinyin_code(), sound);
                        assert_eq!(full.shape_code(), shape);
                        assert_eq!(
                            FullCode::from_parts(full.double_pinyin_code(), full.shape_code()),
                            full
                        );
                    }
                }
            }
        }
    }
}
