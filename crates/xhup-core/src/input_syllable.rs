//! XHUP 规范输入音节:输入法域的只读领域类型。
//!
//! 内建规范清单经 `include_str!` 嵌入仓库级项目数据
//! `data/pinyin/xhup_input_syllables.txt`(唯一事实来源,推导、排除规则与来源见该目录
//! README);同样假定当前 XHUP Flow 仓库的目录结构。
//!
//! 本类型表示「XHUP Flow 接受、且属于当前规范小鹤可编码输入清单的归一化无调拼音
//! 输入音节」。它是输入法域概念:不声称覆盖全部语言学合法普通话音节。
//! 解析为精确匹配:不做大小写转换、Unicode `ü` 处理或声调归一化。

use std::error::Error;
use std::fmt;
use std::str::FromStr;
use std::sync::OnceLock;

const SYLLABLES_TXT: &str = include_str!("../../../data/pinyin/xhup_input_syllables.txt");

/// XHUP 规范输入音节:已证明属于规范输入清单的归一化无调拼音拼写,如 `"lv"`。
///
/// 非法状态无法通过公开 API 构造;解析规则见 [`FromStr`] 实现。
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct XhupInputSyllable(&'static str);

/// 输入音节解析错误。
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum XhupInputSyllableError {
    /// 输入为空。
    Empty,
    /// 输入含非小写 ASCII 字母的字符(大写、数字、调号、Unicode `ü` 等)。
    InvalidCharacter(char),
    /// 拼写全由小写 ASCII 字母构成,但不在规范输入清单内。
    Unknown,
}

impl fmt::Display for XhupInputSyllableError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "输入音节为空"),
            Self::InvalidCharacter(ch) => write!(f, "输入音节包含非法字符: {ch:?}"),
            Self::Unknown => write!(f, "输入音节不在规范清单内"),
        }
    }
}

impl Error for XhupInputSyllableError {}

impl XhupInputSyllable {
    /// 音节拼写,如 `"lv"`。
    pub fn as_str(&self) -> &str {
        self.0
    }

    /// 全部规范输入音节(规范顺序;进程内共享的不可变清单,解析一次)。
    ///
    /// 内嵌的仓库自有数据若损坏属于构建不变量被破坏,初始化会 panic 并给出精确位置。
    pub fn all() -> &'static [XhupInputSyllable] {
        static CANONICAL: OnceLock<Box<[XhupInputSyllable]>> = OnceLock::new();
        CANONICAL.get_or_init(|| parse_inventory(SYLLABLES_TXT, "xhup_input_syllables.txt"))
    }
}

impl FromStr for XhupInputSyllable {
    type Err = XhupInputSyllableError;

    /// 精确校验:空输入 → [`XhupInputSyllableError::Empty`];任一字符非小写 ASCII 字母
    /// → [`XhupInputSyllableError::InvalidCharacter`];其余在规范清单中二分查找,
    /// 未命中 → [`XhupInputSyllableError::Unknown`]。不做任何归一化。
    fn from_str(input: &str) -> Result<Self, Self::Err> {
        if input.is_empty() {
            return Err(XhupInputSyllableError::Empty);
        }
        for ch in input.chars() {
            if !ch.is_ascii_lowercase() {
                return Err(XhupInputSyllableError::InvalidCharacter(ch));
            }
        }
        let inventory = Self::all();
        match inventory.binary_search_by(|syllable| syllable.0.cmp(input)) {
            Ok(index) => Ok(inventory[index]),
            Err(_) => Err(XhupInputSyllableError::Unknown),
        }
    }
}

impl fmt::Display for XhupInputSyllable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}

/// 解析内嵌规范清单:每行一个音节,零拷贝借用内嵌 `&'static str` 切片。
/// 校验:非空行、无首尾空白、全部小写 ASCII、字节序严格升序(同时排除重复)。
/// 不变量被破坏时 panic,消息含文件名与行号。
fn parse_inventory(text: &'static str, name: &str) -> Box<[XhupInputSyllable]> {
    let mut syllables: Vec<XhupInputSyllable> = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let row = index + 1;
        assert!(!line.is_empty(), "{name} 第 {row} 行为空行");
        assert!(
            line.trim() == line,
            "{name} 第 {row} 行含首尾空白: {line:?}"
        );
        assert!(
            line.bytes().all(|byte| byte.is_ascii_lowercase()),
            "{name} 第 {row} 行应为小写 ASCII 字母: {line:?}"
        );
        if let Some(previous) = syllables.last() {
            assert!(
                previous.0 < line,
                "{name} 第 {row} 行未按字节序严格升序(重复或乱序): {line:?}"
            );
        }
        syllables.push(XhupInputSyllable(line));
    }
    assert!(!syllables.is_empty(), "{name} 不应为空文件");
    syllables.into_boxed_slice()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(input: &str) -> Result<XhupInputSyllable, XhupInputSyllableError> {
        input.parse()
    }

    #[test]
    fn parses_representative_syllables() {
        // 常规音节、边缘但已批准的政策成员、ü 族的 v 表示
        for input in [
            "a", "ang", "ba", "chua", "den", "dia", "kei", "lo", "me", "nou", "o", "pou", "shei",
            "yo", "lv", "lve", "nv", "nve",
        ] {
            let syllable = parse(input).unwrap();
            assert_eq!(syllable.as_str(), input);
            assert_eq!(syllable.to_string(), input);
        }
    }

    #[test]
    fn jqx_yu_spellings_keep_normal_orthography() {
        // j/q/x/y 后的常规拼写不改写为 v 形式
        for input in [
            "ju", "qu", "xu", "yu", "jue", "que", "xue", "yue", "jun", "qun", "xun", "yun",
        ] {
            assert!(parse(input).is_ok(), "{input}");
        }
        assert_eq!(parse("jv"), Err(XhupInputSyllableError::Unknown));
    }

    #[test]
    fn empty_input_is_rejected() {
        assert_eq!(parse(""), Err(XhupInputSyllableError::Empty));
    }

    #[test]
    fn invalid_characters_are_rejected_before_lookup() {
        // 大写、Unicode ü、数字、空白均报非法字符;不做归一化
        assert_eq!(
            parse("BA"),
            Err(XhupInputSyllableError::InvalidCharacter('B'))
        );
        assert_eq!(
            parse("nü"),
            Err(XhupInputSyllableError::InvalidCharacter('ü'))
        );
        assert_eq!(
            parse("lüè"),
            Err(XhupInputSyllableError::InvalidCharacter('ü'))
        );
        assert_eq!(
            parse("a1"),
            Err(XhupInputSyllableError::InvalidCharacter('1'))
        );
        assert_eq!(
            parse("ba "),
            Err(XhupInputSyllableError::InvalidCharacter(' '))
        );
    }

    #[test]
    fn well_formed_but_unlisted_spellings_are_unknown() {
        // 结构合法但不在清单:语言/结构合法性与清单成员资格是不同层面
        for input in ["biong", "m", "n", "ng", "hng", "ea", "tei", "zhei", "biang"] {
            assert_eq!(
                parse(input),
                Err(XhupInputSyllableError::Unknown),
                "{input}"
            );
        }
    }

    #[test]
    fn display_round_trips() {
        let syllable = parse("chua").unwrap();
        let reparsed: XhupInputSyllable = syllable.to_string().parse().unwrap();
        assert_eq!(reparsed, syllable);
    }

    #[test]
    fn copy_and_equality() {
        let syllable = parse("shei").unwrap();
        let copied = syllable;
        assert_eq!(syllable, copied);
        assert_ne!(syllable, parse("shui").unwrap());
    }

    #[test]
    fn inventory_has_exactly_406_entries_in_strict_order() {
        let all = XhupInputSyllable::all();
        assert_eq!(all.len(), 406);
        assert!(all.windows(2).all(|w| w[0] < w[1]));
    }

    #[test]
    fn every_inventory_entry_round_trips() {
        for &syllable in XhupInputSyllable::all() {
            let reparsed: XhupInputSyllable = syllable.as_str().parse().unwrap();
            assert_eq!(reparsed, syllable);
        }
    }

    #[test]
    fn all_returns_same_shared_backing_data() {
        let first = XhupInputSyllable::all();
        let second = XhupInputSyllable::all();
        assert!(std::ptr::eq(first, second));
    }

    #[test]
    fn parsed_entries_point_into_canonical_backing_data() {
        // 解析结果直接引用清单内条目,不另行分配
        let syllable = parse("ba").unwrap();
        let canonical = XhupInputSyllable::all()
            .iter()
            .find(|entry| entry.as_str() == "ba")
            .unwrap();
        assert!(std::ptr::eq(syllable.as_str(), canonical.as_str()));
    }

    fn panic_message(payload: Box<dyn std::any::Any + Send>) -> String {
        if let Some(message) = payload.downcast_ref::<String>() {
            message.clone()
        } else if let Some(message) = payload.downcast_ref::<&'static str>() {
            (*message).to_owned()
        } else {
            String::new()
        }
    }

    #[test]
    fn malformed_inventory_panics_with_filename_and_row() {
        for (text, row) in [
            ("ba\n\nchua\n", 2), // 空行
            ("ba\n chua\n", 2),  // 首尾空白
            ("ba\nChua\n", 2),   // 非小写 ASCII
            ("ba\nba\n", 2),     // 重复
            ("chua\nba\n", 2),   // 乱序
        ] {
            let payload =
                std::panic::catch_unwind(|| parse_inventory(text, "test.txt")).unwrap_err();
            let message = panic_message(payload);
            assert!(message.contains("test.txt"), "{message}");
            assert!(message.contains(&format!("第 {row} 行")), "{message}");
        }
    }
}
