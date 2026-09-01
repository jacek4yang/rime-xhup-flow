//! 规范汉字与汉字读音:汉字读音域的只读领域类型。
//!
//! 内建规范数据经 `include_str!` 嵌入仓库级项目数据 `data/hanzi/readings.tsv`
//! (唯一事实来源,推导、并集规则与来源见该目录 README);同样假定当前 XHUP Flow
//! 仓库的目录结构。规范汉字成员资格仅由该文件首列唯一集合承载,进程内派生索引不算
//! 第二份事实来源。
//!
//! [`XhupHanzi`] 表示「属于规范 8105 汉字清单的一个字符」;[`HanziReading`] 表示
//! 「实际出现在规范汉字数据中的归一化无调读音」。`HanziReading` 刻意保留来源事实,
//! 不等同于 [`XhupInputSyllable`]:当前有 6 个 `(字, 读音)` 关系落在 406 音节清单
//! 之外(m、n、ng、hng、ea),它们仍是合法读音,仅经
//! [`HanziReading::to_input_syllable()`] 作精确可选转换,不做任何语义替换。

use std::error::Error;
use std::fmt;
use std::str::FromStr;
use std::sync::OnceLock;

use crate::XhupInputSyllable;

const READINGS_TSV: &str = include_str!("../../../data/hanzi/readings.tsv");

/// 规范汉字:已证明属于规范 8105 汉字清单的字符,如 `'行'`。
///
/// 非法状态(不属于清单的任意 Unicode 标量)无法通过公开 API 构造;
/// 解析规则见 [`FromStr`] / [`TryFrom`] 实现。
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct XhupHanzi(char);

/// 汉字读音:实际出现在规范汉字数据中的归一化无调拼音拼写,如 `"xing"`。
///
/// 只能经 [`XhupHanzi`] 的发音 API 获得;任意字符串即使形似拼音也不能构造本类型。
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct HanziReading(&'static str);

/// 汉字解析错误。
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum XhupHanziError {
    /// 输入为空。
    Empty,
    /// 输入含多个字符。
    MultipleCharacters,
    /// 单个字符不属于规范 8105 汉字清单。
    Unknown(char),
}

impl fmt::Display for XhupHanziError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "汉字输入为空"),
            Self::MultipleCharacters => write!(f, "汉字输入应恰好为一个字符"),
            Self::Unknown(ch) => write!(f, "字符不在规范汉字清单内: {ch:?}"),
        }
    }
}

impl Error for XhupHanziError {}

/// 一个规范汉字的解析结果:`readings[0]` 为 primary,其余为按规范顺序的 alt。
#[derive(Debug)]
struct HanziRecord {
    hanzi: XhupHanzi,
    readings: Box<[HanziReading]>,
}

/// 进程级规范数据:解析一次,派生的字符索引用于 `XhupHanzi::all()`。
#[derive(Debug)]
struct CanonicalHanziData {
    records: Box<[HanziRecord]>,
    all: Box<[XhupHanzi]>,
}

fn canonical() -> &'static CanonicalHanziData {
    static CANONICAL: OnceLock<CanonicalHanziData> = OnceLock::new();
    CANONICAL.get_or_init(|| parse_readings(READINGS_TSV, "readings.tsv"))
}

impl XhupHanzi {
    /// 全部规范汉字(Unicode 标量值升序;进程内共享的不可变清单,解析一次)。
    ///
    /// 内嵌的仓库自有数据若损坏属于构建不变量被破坏,初始化会 panic 并给出精确位置。
    pub fn all() -> &'static [XhupHanzi] {
        &canonical().all
    }

    /// 字符本身。
    pub fn as_char(self) -> char {
        self.0
    }

    /// 主读音(上游「最常用读音」;可能不在 406 音节清单内,如 `呣 -> m`)。
    pub fn primary_reading(self) -> HanziReading {
        self.readings()[0]
    }

    /// 全部规范读音:非空,主读音在前,其余按规范顺序。
    ///
    /// 注意:不保证任一读音可转换为 [`XhupInputSyllable`];当前 `呣`、`嗯` 两字
    /// 没有任何 XHUP 可编码读音。
    pub fn readings(self) -> &'static [HanziReading] {
        let records = &canonical().records;
        let index = records
            .binary_search_by(|record| record.hanzi.0.cmp(&self.0))
            .expect("XhupHanzi 不变量:必然属于规范汉字清单");
        &records[index].readings
    }
}

impl TryFrom<char> for XhupHanzi {
    type Error = XhupHanziError;

    /// 成员校验:属于规范清单的字符返回对应实例,否则
    /// [`XhupHanziError::Unknown`]。不做繁简转换或 Unicode 规范化。
    fn try_from(ch: char) -> Result<Self, Self::Error> {
        let records = &canonical().records;
        match records.binary_search_by(|record| record.hanzi.0.cmp(&ch)) {
            Ok(index) => Ok(records[index].hanzi),
            Err(_) => Err(XhupHanziError::Unknown(ch)),
        }
    }
}

impl FromStr for XhupHanzi {
    type Err = XhupHanziError;

    /// 精确校验:空输入 → [`XhupHanziError::Empty`];多个字符 →
    /// [`XhupHanziError::MultipleCharacters`];单字符委托 [`TryFrom<char>`] 成员校验。
    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let mut chars = input.chars();
        let Some(ch) = chars.next() else {
            return Err(XhupHanziError::Empty);
        };
        if chars.next().is_some() {
            return Err(XhupHanziError::MultipleCharacters);
        }
        Self::try_from(ch)
    }
}

impl fmt::Display for XhupHanzi {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl HanziReading {
    /// 归一化无调拼写,如 `"xing"`。
    pub fn as_str(self) -> &'static str {
        self.0
    }

    /// 精确转换为规范输入音节;当前项目编码边界外的读音(`m`、`n`、`ng`、
    /// `hng`、`ea`)返回 `None`。不做任何归一化或语义替换。
    pub fn to_input_syllable(self) -> Option<XhupInputSyllable> {
        self.0.parse().ok()
    }
}

impl fmt::Display for HanziReading {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}

/// 解析内嵌规范读音表:每行 `字<TAB>读音<TAB>角色`,零拷贝借用内嵌 `&'static str`。
/// 校验全部规范格式与排序不变量;被破坏时 panic,消息含文件名与行号。
fn parse_readings(text: &'static str, name: &str) -> CanonicalHanziData {
    assert!(!text.is_empty(), "{name} 不应为空文件");

    let mut records: Vec<HanziRecord> = Vec::new();
    let mut all: Vec<XhupHanzi> = Vec::new();
    let mut row_count = 0usize;
    // 当前字符组:primary 槽位与已见 alt(严格字节序升序)。
    let mut group_char: Option<char> = None;
    let mut group_primary: Option<HanziReading> = None;
    let mut group_alts: Vec<HanziReading> = Vec::new();

    for (index, line) in text.lines().enumerate() {
        let row = index + 1;
        row_count += 1;

        let mut fields = line.split('\t');
        let (Some(char_field), Some(reading), Some(role), None) =
            (fields.next(), fields.next(), fields.next(), fields.next())
        else {
            panic!("{name} 第 {row} 行应为三个 TAB 分隔字段: {line:?}");
        };

        let mut chars = char_field.chars();
        let (Some(hanzi_char), None) = (chars.next(), chars.next()) else {
            panic!("{name} 第 {row} 行字符字段应恰好为一个字符: {char_field:?}");
        };
        assert!(
            !reading.is_empty() && reading.bytes().all(|byte| byte.is_ascii_lowercase()),
            "{name} 第 {row} 行读音应为非空小写 ASCII 字母: {reading:?}"
        );
        assert!(
            matches!(role, "primary" | "alt"),
            "{name} 第 {row} 行角色应为 primary 或 alt: {role:?}"
        );

        if group_char != Some(hanzi_char) {
            // 上一字符组结束:组内必须恰有一个 primary。
            if let Some(previous) = group_char.take() {
                finish_group(
                    &mut records,
                    &mut all,
                    previous,
                    &mut group_primary,
                    &mut group_alts,
                    name,
                    row,
                );
                assert!(
                    hanzi_char > previous,
                    "{name} 第 {row} 行字符组未按 Unicode 标量值严格升序(重复或乱序): {hanzi_char:?}"
                );
            }
            group_char = Some(hanzi_char);
        }

        if role == "primary" {
            assert!(
                group_primary.is_none() && group_alts.is_empty(),
                "{name} 第 {row} 行:每字恰有一个 primary 且必须为组内首行: {line:?}"
            );
            group_primary = Some(HanziReading(reading));
        } else {
            let Some(primary) = group_primary else {
                panic!("{name} 第 {row} 行:primary 必须为组内首行: {line:?}");
            };
            assert!(
                reading != primary.0,
                "{name} 第 {row} 行 alt 读音与 primary 重复: {line:?}"
            );
            if let Some(last) = group_alts.last() {
                assert!(
                    last.0 < reading,
                    "{name} 第 {row} 行 alt 读音未按字节序严格升序(重复或乱序): {line:?}"
                );
            }
            group_alts.push(HanziReading(reading));
        }
    }

    if let Some(last_char) = group_char.take() {
        finish_group(
            &mut records,
            &mut all,
            last_char,
            &mut group_primary,
            &mut group_alts,
            name,
            row_count,
        );
    }

    assert!(
        records.len() == 8105,
        "{name} 应恰好包含 8105 个规范汉字,实际 {} 个",
        records.len()
    );
    assert!(
        row_count == 8580,
        "{name} 应恰好包含 8580 行,实际 {row_count} 行"
    );
    CanonicalHanziData {
        records: records.into_boxed_slice(),
        all: all.into_boxed_slice(),
    }
}

/// 结束一个字符组:组内恰有一个 primary,拼接为 primary 在前的读音切片。
fn finish_group(
    records: &mut Vec<HanziRecord>,
    all: &mut Vec<XhupHanzi>,
    hanzi_char: char,
    group_primary: &mut Option<HanziReading>,
    group_alts: &mut Vec<HanziReading>,
    name: &str,
    row: usize,
) {
    let primary = group_primary
        .take()
        .unwrap_or_else(|| panic!("{name} 第 {row} 行之前:字符 {hanzi_char:?} 缺少 primary 行"));
    let mut readings = Vec::with_capacity(group_alts.len() + 1);
    readings.push(primary);
    readings.append(group_alts);
    let hanzi = XhupHanzi(hanzi_char);
    records.push(HanziRecord {
        hanzi,
        readings: readings.into_boxed_slice(),
    });
    all.push(hanzi);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(input: &str) -> Result<XhupHanzi, XhupHanziError> {
        input.parse()
    }

    fn hanzi(ch: char) -> XhupHanzi {
        XhupHanzi::try_from(ch).unwrap()
    }

    fn reading_strs(hanzi: XhupHanzi) -> Vec<&'static str> {
        hanzi.readings().iter().map(|reading| reading.0).collect()
    }

    #[test]
    fn sentinel_primary_and_reading_sets() {
        // 单音字
        for (ch, primary) in [('一', "yi"), ('中', "zhong"), ('为', "wei"), ('呣', "m")] {
            let hanzi = hanzi(ch);
            assert_eq!(hanzi.primary_reading().as_str(), primary, "{ch}");
            assert_eq!(reading_strs(hanzi), [primary], "{ch}");
        }
        // 多音字:主读音 + 完整规范读音集(primary 在前,alt 字节序升序)
        let cases: [(char, &str, &[&str]); 17] = [
            ('行', "xing", &["xing", "hang", "heng"]),
            ('长', "zhang", &["zhang", "chang"]),
            ('重', "zhong", &["zhong", "chong"]),
            ('乐', "le", &["le", "yue"]),
            ('的', "de", &["de", "di"]),
            ('得', "de", &["de", "dei"]),
            ('着', "zhe", &["zhe", "zhao", "zhuo"]),
            ('说', "shuo", &["shuo", "shui"]),
            ('差', "cha", &["cha", "chai", "ci"]),
            ('熟', "shu", &["shu", "shou"]),
            ('朝', "chao", &["chao", "zhao"]),
            ('呒', "wu", &["wu", "m"]),
            ('哼', "heng", &["heng", "hng"]),
            ('嗯', "n", &["n", "ng"]),
            ('欸', "ai", &["ai", "ea"]),
            ('欻', "chua", &["chua", "xu"]),
            ('剋', "ke", &["ke", "kei"]),
        ];
        for (ch, primary, expected) in cases {
            let hanzi = hanzi(ch);
            assert_eq!(hanzi.primary_reading().as_str(), primary, "{ch}");
            assert_eq!(reading_strs(hanzi), expected, "{ch}");
        }
        // 126 个 primary∉kTGHZ 之一:primary 与字典 alt 并集共存
        let hanzi = hanzi('嗲');
        assert_eq!(hanzi.primary_reading().as_str(), "die");
        assert_eq!(reading_strs(hanzi), ["die", "dia"]);
    }

    #[test]
    fn from_str_semantics() {
        assert_eq!(parse(""), Err(XhupHanziError::Empty));
        assert_eq!(parse("中国"), Err(XhupHanziError::MultipleCharacters));
        assert_eq!(parse("中"), Ok(hanzi('中')));
        // 单个字符但不属于规范清单
        assert_eq!(parse("😀"), Err(XhupHanziError::Unknown('😀')));
    }

    #[test]
    fn try_from_char_semantics() {
        assert_eq!(XhupHanzi::try_from('行').unwrap().as_char(), '行');
        assert_eq!(
            XhupHanzi::try_from('😀'),
            Err(XhupHanziError::Unknown('😀'))
        );
    }

    #[test]
    fn display_round_trips() {
        let hanzi = hanzi('行');
        assert_eq!(hanzi.to_string(), "行");
        let reparsed: XhupHanzi = hanzi.to_string().parse().unwrap();
        assert_eq!(reparsed, hanzi);
        assert_eq!(hanzi.primary_reading().to_string(), "xing");
    }

    #[test]
    fn copy_and_equality() {
        let zhong = hanzi('中');
        let copied = zhong;
        assert_eq!(zhong, copied);
        assert_ne!(zhong, hanzi('行'));
    }

    #[test]
    fn inventory_has_exactly_8105_entries_in_strict_order() {
        let all = XhupHanzi::all();
        assert_eq!(all.len(), 8105);
        assert!(all.windows(2).all(|w| w[0] < w[1]));
    }

    #[test]
    fn every_inventory_entry_round_trips() {
        for &hanzi in XhupHanzi::all() {
            let reparsed = XhupHanzi::try_from(hanzi.as_char()).unwrap();
            assert_eq!(reparsed, hanzi);
        }
    }

    #[test]
    fn all_returns_same_shared_backing_data() {
        assert!(std::ptr::eq(XhupHanzi::all(), XhupHanzi::all()));
    }

    #[test]
    fn readings_are_stable_process_lifetime_slices() {
        let first = hanzi('行').readings();
        let second = hanzi('行').readings();
        assert!(std::ptr::eq(first, second));
    }

    #[test]
    fn primary_is_always_first_reading() {
        for &hanzi in XhupHanzi::all() {
            assert!(!hanzi.readings().is_empty());
            assert_eq!(hanzi.primary_reading(), hanzi.readings()[0]);
        }
    }

    #[test]
    fn to_input_syllable_is_exact_optional_conversion() {
        assert!(hanzi('行').primary_reading().to_input_syllable().is_some());
        // ü 族的 v 表示属于清单内
        let lv_hanzi = hanzi('旅');
        assert!(
            lv_hanzi
                .readings()
                .iter()
                .any(|reading| reading.as_str() == "lv" && reading.to_input_syllable().is_some())
        );
        // 项目编码边界外的来源读音:保留为合法 HanziReading,转换返回 None
        for (ch, spelling) in [
            ('呒', "m"),
            ('呣', "m"),
            ('哼', "hng"),
            ('嗯', "n"),
            ('嗯', "ng"),
            ('欸', "ea"),
        ] {
            let reading = hanzi(ch)
                .readings()
                .iter()
                .copied()
                .find(|reading| reading.as_str() == spelling)
                .unwrap_or_else(|| panic!("{ch} 应有读音 {spelling}"));
            assert_eq!(reading.to_input_syllable(), None, "{ch} {spelling}");
        }
    }

    #[test]
    fn non_xhup_boundary_matches_audited_set() {
        // 再生后机械验证的边界:恰好 6 个非 XHUP (字, 读音) 关系、5 字、2 个非 XHUP 主读音
        let mut relations = Vec::new();
        for &hanzi in XhupHanzi::all() {
            for &reading in hanzi.readings() {
                if reading.to_input_syllable().is_none() {
                    let is_primary = reading == hanzi.primary_reading();
                    relations.push((hanzi.as_char(), reading.as_str(), is_primary));
                }
            }
        }
        assert_eq!(
            relations,
            [
                ('呒', "m", false),
                ('呣', "m", true),
                ('哼', "hng", false),
                ('嗯', "n", true),
                ('嗯', "ng", false),
                ('欸', "ea", false),
            ]
        );
    }

    #[test]
    fn exactly_two_hanzi_have_zero_encodable_readings() {
        // 架构事实:不得假设每个规范汉字都有 XHUP 可编码读音
        let zero: Vec<char> = XhupHanzi::all()
            .iter()
            .filter(|hanzi| {
                hanzi
                    .readings()
                    .iter()
                    .all(|reading| reading.to_input_syllable().is_none())
            })
            .map(|hanzi| hanzi.as_char())
            .collect();
        assert_eq!(zero, ['呣', '嗯']);
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
    fn malformed_data_panics_with_filename_and_row() {
        for (text, row) in [
            ("中\tzhong\n", 1),                                    // 字段数不足
            ("中\tzhong\tprimary\textra\n", 1),                    // 字段数过多
            ("中国\tzhong\tprimary\n", 1),                         // 字符字段多个字符
            ("中\t\tprimary\n", 1),                                // 读音为空
            ("中\tZhong\tprimary\n", 1),                           // 读音非小写 ASCII
            ("中\tzhong\tmain\n", 1),                              // 非法角色
            ("中\tzhong\tprimary\n一\tyi\tprimary\n", 2), // 字符组乱序(中 U+4E2D > 一 U+4E00)
            ("中\tzhong\tprimary\n中\tguo\tprimary\n", 2), // 第二个 primary
            ("中\tzhong\talt\n", 1),                      // primary 未在组内首行
            ("中\tzhong\tprimary\n中\tzhong\talt\n", 2),  // alt 与 primary 重复
            ("中\tguo\tprimary\n中\tzuo\talt\n中\tjia\talt\n", 3), // alt 乱序
        ] {
            let payload =
                std::panic::catch_unwind(|| parse_readings(text, "test.tsv")).unwrap_err();
            let message = panic_message(payload);
            assert!(message.contains("test.tsv"), "{message}");
            assert!(message.contains(&format!("第 {row} 行")), "{message}");
        }
    }
}
