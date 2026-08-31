//! 小鹤双拼键盘布局的只读领域对象。
//!
//! 内建规范布局通过 `include_str!` 从仓库级项目数据 `data/double_pinyin/*.tsv`
//! 编译期嵌入:TSV 文件是唯一的映射事实来源,本模块不复制映射内容。
//! 这也意味着内建规范布局假定当前 XHUP Flow 仓库的目录结构;
//! 独立打包发布不在当前范围内。

use std::sync::OnceLock;

use crate::code::DoublePinyinCode;
use crate::key::Key;

const INITIALS_TSV: &str = include_str!("../../../data/double_pinyin/initials.tsv");
const FINALS_TSV: &str = include_str!("../../../data/double_pinyin/finals.tsv");
const ZERO_INITIALS_TSV: &str = include_str!("../../../data/double_pinyin/zero_initials.tsv");

/// 一条「声母 → 键位」映射。
#[derive(Clone, Copy, Eq, PartialEq, Hash, Debug)]
pub struct InitialMapping {
    initial: &'static str,
    key: Key,
}

impl InitialMapping {
    /// 声母名称,如 `"zh"`。
    pub fn initial(&self) -> &str {
        self.initial
    }

    /// 映射到的键位。
    pub fn key(&self) -> Key {
        self.key
    }
}

/// 一条「韵母 → 键位」映射。多个韵母可以共享同一键位。
#[derive(Clone, Copy, Eq, PartialEq, Hash, Debug)]
pub struct FinalMapping {
    final_: &'static str,
    key: Key,
}

impl FinalMapping {
    /// 韵母名称,如 `"iang"`;ü 族以 ASCII `v` 表示,如 `"ve"`。
    pub fn final_(&self) -> &str {
        self.final_
    }

    /// 映射到的键位。
    pub fn key(&self) -> Key {
        self.key
    }
}

/// 一条「零声母音节 → 两键编码」映射。
#[derive(Clone, Copy, Eq, PartialEq, Hash, Debug)]
pub struct ZeroInitialMapping {
    syllable: &'static str,
    code: DoublePinyinCode,
}

impl ZeroInitialMapping {
    /// 零声母音节,如 `"ang"`。
    pub fn syllable(&self) -> &str {
        self.syllable
    }

    /// 对应的双键编码,如 `ang→ah`。
    pub fn code(&self) -> DoublePinyinCode {
        self.code
    }
}

/// 小鹤双拼键盘布局:规范数据的只读、不可变内存表示。
///
/// 数据量很小(23 声母 / 33 韵母 / 12 零声母),查询采用线性查找,
/// 不引入哈希索引。查询为精确匹配:不做大小写转换、不做 Unicode/ü 归一化。
#[derive(Debug)]
pub struct DoublePinyinLayout {
    initials: Box<[InitialMapping]>,
    finals: Box<[FinalMapping]>,
    zero_initials: Box<[ZeroInitialMapping]>,
}

static CANONICAL: OnceLock<DoublePinyinLayout> = OnceLock::new();

impl DoublePinyinLayout {
    /// 返回内建规范小鹤双拼布局(进程内共享的不可变实例,解析一次)。
    ///
    /// 内嵌的仓库自有数据若损坏属于构建不变量被破坏,此处会 panic 并给出精确位置。
    pub fn canonical() -> &'static Self {
        CANONICAL.get_or_init(parse_canonical)
    }

    /// 精确查询声母对应键位;未知或非规范形式(如大写)返回 `None`。
    pub fn initial_key(&self, initial: &str) -> Option<Key> {
        self.initials
            .iter()
            .find(|mapping| mapping.initial == initial)
            .map(|mapping| mapping.key)
    }

    /// 精确查询韵母对应键位;不做 `ü` 到 `v` 的隐式归一化。
    pub fn final_key(&self, final_: &str) -> Option<Key> {
        self.finals
            .iter()
            .find(|mapping| mapping.final_ == final_)
            .map(|mapping| mapping.key)
    }

    /// 精确查询零声母音节的两键编码。
    pub fn zero_initial_code(&self, syllable: &str) -> Option<DoublePinyinCode> {
        self.zero_initials
            .iter()
            .find(|mapping| mapping.syllable == syllable)
            .map(|mapping| mapping.code)
    }

    /// 全部声母映射(规范顺序)。
    pub fn initials(&self) -> &[InitialMapping] {
        &self.initials
    }

    /// 全部韵母映射(规范顺序)。
    pub fn finals(&self) -> &[FinalMapping] {
        &self.finals
    }

    /// 全部零声母映射(规范顺序)。
    pub fn zero_initials(&self) -> &[ZeroInitialMapping] {
        &self.zero_initials
    }
}

/// 一行两列 TSV 的私有中间表示;保留行号以便不变量报错精确定位。
struct TsvRow {
    row: usize,
    first: &'static str,
    second: &'static str,
}

/// 解析内嵌规范数据。结构不变量被破坏时 panic,消息含文件名与行号。
fn parse_canonical() -> DoublePinyinLayout {
    DoublePinyinLayout {
        initials: parse_initials(INITIALS_TSV, "initials.tsv"),
        finals: parse_finals(FINALS_TSV, "finals.tsv"),
        zero_initials: parse_zero_initials(ZERO_INITIALS_TSV, "zero_initials.tsv"),
    }
}

fn parse_initials(tsv: &'static str, name: &str) -> Box<[InitialMapping]> {
    tsv_rows(tsv, name)
        .into_iter()
        .map(|row| InitialMapping {
            initial: row.first,
            key: parse_key_field(row.second, name, row.row),
        })
        .collect()
}

fn parse_finals(tsv: &'static str, name: &str) -> Box<[FinalMapping]> {
    tsv_rows(tsv, name)
        .into_iter()
        .map(|row| FinalMapping {
            final_: row.first,
            key: parse_key_field(row.second, name, row.row),
        })
        .collect()
}

fn parse_zero_initials(tsv: &'static str, name: &str) -> Box<[ZeroInitialMapping]> {
    tsv_rows(tsv, name)
        .into_iter()
        .map(|row| ZeroInitialMapping {
            syllable: row.first,
            code: row.second.parse().unwrap_or_else(|err| {
                panic!("{name} 第 {} 行编码非法: {:?}({err})", row.row, row.second)
            }),
        })
        .collect()
}

/// 逐行解析两列 TSV;零拷贝借用内嵌 `&'static str` 切片。
/// 校验:恰好两个字段、字段非空、首列严格升序(同时排除重复)。
fn tsv_rows(tsv: &'static str, name: &str) -> Vec<TsvRow> {
    let mut rows: Vec<TsvRow> = Vec::new();
    for (index, line) in tsv.lines().enumerate() {
        let row = index + 1;
        let fields: Vec<&str> = line.split('\t').collect();
        assert!(
            fields.len() == 2,
            "{name} 第 {row} 行应有 2 个 TAB 分隔字段: {line:?}"
        );
        let (first, second) = (fields[0], fields[1]);
        assert!(
            !first.is_empty() && !second.is_empty(),
            "{name} 第 {row} 行字段不能为空"
        );
        if let Some(previous) = rows.last() {
            assert!(
                previous.first < first,
                "{name} 第 {row} 行首列未按字典序严格升序(重复或乱序): {first:?}"
            );
        }
        rows.push(TsvRow { row, first, second });
    }
    rows
}

/// 键位字段必须为单字符且通过 [`Key`] 校验。
fn parse_key_field(value: &'static str, name: &str, row: usize) -> Key {
    let mut chars = value.chars();
    let ch = chars
        .next()
        .unwrap_or_else(|| panic!("{name} 第 {row} 行键位字段为空"));
    assert!(
        chars.next().is_none(),
        "{name} 第 {row} 行键位应为单字符: {value:?}"
    );
    Key::from_char(ch).unwrap_or_else(|_| panic!("{name} 第 {row} 行键位非法: {value:?}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(ch: char) -> Option<Key> {
        Some(Key::from_char(ch).unwrap())
    }

    #[test]
    fn canonical_returns_same_shared_instance() {
        let first = DoublePinyinLayout::canonical();
        let second = DoublePinyinLayout::canonical();
        assert!(std::ptr::eq(first, second));
    }

    #[test]
    fn enumeration_counts_match_embedded_canonical_data() {
        let layout = DoublePinyinLayout::canonical();
        assert_eq!(layout.initials().len(), 23);
        assert_eq!(layout.finals().len(), 33);
        assert_eq!(layout.zero_initials().len(), 12);
    }

    #[test]
    fn initial_lookup_is_exact() {
        let layout = DoublePinyinLayout::canonical();
        assert_eq!(layout.initial_key("b"), key('b'));
        assert_eq!(layout.initial_key("zh"), key('v'));
        assert_eq!(layout.initial_key("ch"), key('i'));
        assert_eq!(layout.initial_key("sh"), key('u'));
        assert_eq!(layout.initial_key("ZH"), None);
        assert_eq!(layout.initial_key("Zh"), None);
        assert_eq!(layout.initial_key("zz"), None);
        assert_eq!(layout.initial_key(""), None);
    }

    #[test]
    fn final_lookup_is_exact() {
        let layout = DoublePinyinLayout::canonical();
        assert_eq!(layout.final_key("ang"), key('h'));
        assert_eq!(layout.final_key("ing"), key('k'));
        assert_eq!(layout.final_key("uai"), key('k'));
        assert_eq!(layout.final_key("ong"), key('s'));
        assert_eq!(layout.final_key("iong"), key('s'));
        assert_eq!(layout.final_key("ue"), key('t'));
        assert_eq!(layout.final_key("ve"), key('t'));
        assert_eq!(layout.final_key("v"), key('v'));
        assert_eq!(layout.final_key("ANG"), None);
        assert_eq!(layout.final_key("zzz"), None);
        // 不隐式归一化 Unicode `ü`
        assert_eq!(layout.final_key("üe"), None);
    }

    #[test]
    fn zero_initial_lookup_is_exact() {
        let layout = DoublePinyinLayout::canonical();
        assert_eq!(layout.zero_initial_code("a").unwrap().to_string(), "aa");
        assert_eq!(layout.zero_initial_code("an").unwrap().to_string(), "an");
        assert_eq!(layout.zero_initial_code("ang").unwrap().to_string(), "ah");
        assert_eq!(layout.zero_initial_code("eng").unwrap().to_string(), "eg");
        assert_eq!(layout.zero_initial_code("ANG"), None);
        assert_eq!(layout.zero_initial_code("xyz"), None);
    }

    #[test]
    fn enumeration_preserves_canonical_order_with_typed_entries() {
        let layout = DoublePinyinLayout::canonical();

        let initials = layout.initials();
        assert!(initials.windows(2).all(|w| w[0].initial() < w[1].initial()));
        assert_eq!(initials[0].initial(), "b");
        assert_eq!(initials[0].key(), Key::from_char('b').unwrap());
        assert_eq!(initials[22].initial(), "zh");
        assert_eq!(initials[22].key(), Key::from_char('v').unwrap());

        let finals = layout.finals();
        assert!(finals.windows(2).all(|w| w[0].final_() < w[1].final_()));
        assert_eq!(finals[0].final_(), "a");
        assert_eq!(finals[32].final_(), "ve");
        assert_eq!(finals[32].key(), Key::from_char('t').unwrap());

        let zero_initials = layout.zero_initials();
        assert!(
            zero_initials
                .windows(2)
                .all(|w| w[0].syllable() < w[1].syllable())
        );
        assert_eq!(zero_initials[0].syllable(), "a");
        assert_eq!(zero_initials[0].code().to_string(), "aa");
        // 枚举条目暴露类型化 `DoublePinyinCode`,可直接使用
        let typed: DoublePinyinCode = zero_initials[2].code();
        assert_eq!(typed.to_string(), "an");
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
    fn invalid_initial_key_panic_reports_filename_and_row() {
        let payload = std::panic::catch_unwind(|| parse_initials("b\tb\nch\tI\n", "initials.tsv"))
            .unwrap_err();
        let message = panic_message(payload);
        assert!(message.contains("initials.tsv"), "{message}");
        assert!(message.contains("第 2 行"), "{message}");
        assert!(message.contains("\"I\""), "{message}");
    }

    #[test]
    fn invalid_final_key_panic_reports_filename_and_row() {
        let payload =
            std::panic::catch_unwind(|| parse_finals("a\ta\nang\t1\n", "finals.tsv")).unwrap_err();
        let message = panic_message(payload);
        assert!(message.contains("finals.tsv"), "{message}");
        assert!(message.contains("第 2 行"), "{message}");
        assert!(message.contains("\"1\""), "{message}");
    }

    #[test]
    fn invalid_zero_initial_code_panic_reports_filename_and_row() {
        let payload = std::panic::catch_unwind(|| {
            parse_zero_initials("a\taa\nang\tabc\n", "zero_initials.tsv")
        })
        .unwrap_err();
        let message = panic_message(payload);
        assert!(message.contains("zero_initials.tsv"), "{message}");
        assert!(message.contains("第 2 行"), "{message}");
        assert!(message.contains("\"abc\""), "{message}");
    }
}
