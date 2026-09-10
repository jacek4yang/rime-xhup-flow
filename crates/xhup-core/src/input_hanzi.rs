//! 可输入字符与有来源的小鹤编码事实。
//!
//! [`XhupHanzi`] 仍精确表示规范 8105 字核心；[`InputHanzi`] 表示生产输入法
//! 接受的字符并集。成员资格与编码来自 `data/xhup/attested_char_codes.tsv`，不要求
//! 字符存在语言学规范读音，也不把小鹤音码伪装成拼音。

use std::error::Error;
use std::fmt;
use std::str::FromStr;
use std::sync::OnceLock;

use crate::{DoublePinyinCode, FullCode, ShapeCode, XhupHanzi};

const ATTESTED_CODES_TSV: &str = include_str!("../../../data/xhup/attested_char_codes.tsv");

/// 生产输入法支持的一个字符（规范 8105 核心或有来源的扩展字符）。
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct InputHanzi(char);

/// 一条小鹤输入编码来源证据。
///
/// 音码和形码是输入法事实，不等同于语言学拼音/规范字形事实。
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct AttestedXhupCode {
    hanzi: InputHanzi,
    sound_code: DoublePinyinCode,
    shape_code: ShapeCode,
    source_weight: u32,
    source: &'static str,
    status: &'static str,
}

/// 输入字符解析错误。
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum InputHanziError {
    /// 输入为空。
    Empty,
    /// 输入包含多个字符。
    MultipleCharacters,
    /// 字符不在有来源的生产输入字符并集中。
    Unknown(char),
}

impl fmt::Display for InputHanziError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "输入字符为空"),
            Self::MultipleCharacters => write!(f, "输入应恰好为一个字符"),
            Self::Unknown(ch) => write!(f, "字符不在 XHUP 输入字符并集中: {ch:?}"),
        }
    }
}

impl Error for InputHanziError {}

#[derive(Debug)]
struct InputRecord {
    hanzi: InputHanzi,
    evidence: Box<[AttestedXhupCode]>,
}

#[derive(Debug)]
struct AttestedData {
    records: Box<[InputRecord]>,
    all: Box<[InputHanzi]>,
    evidence: Box<[AttestedXhupCode]>,
}

fn canonical() -> &'static AttestedData {
    static DATA: OnceLock<AttestedData> = OnceLock::new();
    DATA.get_or_init(|| parse_attested(ATTESTED_CODES_TSV, "attested_char_codes.tsv"))
}

impl InputHanzi {
    /// 全部生产输入字符，按 Unicode 标量值严格升序。
    pub fn all() -> &'static [Self] {
        &canonical().all
    }

    /// 字符本身。
    pub fn as_char(self) -> char {
        self.0
    }

    /// 若属于规范 8105 核心则返回强类型核心字符，否则返回 `None`。
    pub fn core_standard(self) -> Option<XhupHanzi> {
        XhupHanzi::try_from(self.0).ok()
    }

    /// 是否属于规范 8105 核心子集。
    pub fn is_core_standard(self) -> bool {
        self.core_standard().is_some()
    }

    /// 该字符的全部来源证据，按音码、形码、来源、status、权重排序。
    pub fn attested_codes(self) -> &'static [AttestedXhupCode] {
        let records = &canonical().records;
        let index = records
            .binary_search_by_key(&self.0, |record| record.hanzi.0)
            .expect("InputHanzi 不变量:必然存在 evidence 记录");
        &records[index].evidence
    }
}

impl TryFrom<char> for InputHanzi {
    type Error = InputHanziError;

    fn try_from(ch: char) -> Result<Self, Self::Error> {
        canonical()
            .records
            .binary_search_by_key(&ch, |record| record.hanzi.0)
            .map(|index| canonical().records[index].hanzi)
            .map_err(|_| InputHanziError::Unknown(ch))
    }
}

impl FromStr for InputHanzi {
    type Err = InputHanziError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let mut chars = input.chars();
        let Some(ch) = chars.next() else {
            return Err(InputHanziError::Empty);
        };
        if chars.next().is_some() {
            return Err(InputHanziError::MultipleCharacters);
        }
        Self::try_from(ch)
    }
}

impl fmt::Display for InputHanzi {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl AttestedXhupCode {
    /// 全部来源证据，包含同一编码的多来源记录。
    pub fn all() -> &'static [Self] {
        &canonical().evidence
    }

    /// 对应输入字符。
    pub fn hanzi(self) -> InputHanzi {
        self.hanzi
    }

    /// 两键小鹤输入音码。
    pub fn sound_code(self) -> DoublePinyinCode {
        self.sound_code
    }

    /// 两键小鹤形码。
    pub fn shape_code(self) -> ShapeCode {
        self.shape_code
    }

    /// 四键完整输入码。
    pub fn full_code(self) -> FullCode {
        FullCode::from_parts(self.sound_code, self.shape_code)
    }

    /// 来源携带的兼容权重；`0` 表示该来源没有提供权重事实。
    pub fn source_weight(self) -> u32 {
        self.source_weight
    }

    /// 稳定来源标识。
    pub fn source(self) -> &'static str {
        self.source
    }

    /// 来源状态，如 `official`、`official-yield-full`、`legacy-compatible`。
    pub fn status(self) -> &'static str {
        self.status
    }

    /// 是否来自当前小鹤官网 oracle。
    pub fn is_official(self) -> bool {
        self.source == "flypy-official-ix"
    }
}

fn parse_attested(text: &'static str, name: &str) -> AttestedData {
    let mut evidence = Vec::new();
    let mut previous: Option<(
        char,
        DoublePinyinCode,
        ShapeCode,
        &'static str,
        &'static str,
        u32,
    )> = None;
    for (index, line) in text.lines().enumerate() {
        let row = index + 1;
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        let mut fields = line.split('\t');
        let (
            Some(character),
            Some(sound),
            Some(shape),
            Some(weight),
            Some(source),
            Some(status),
            None,
        ) = (
            fields.next(),
            fields.next(),
            fields.next(),
            fields.next(),
            fields.next(),
            fields.next(),
            fields.next(),
        )
        else {
            panic!("{name} 第 {row} 行应为六个 TAB 字段: {line:?}");
        };
        let mut chars = character.chars();
        let (Some(character), None) = (chars.next(), chars.next()) else {
            panic!("{name} 第 {row} 行字符字段必须恰好一个 Unicode 标量: {line:?}");
        };
        let sound_code: DoublePinyinCode = sound
            .parse()
            .unwrap_or_else(|err| panic!("{name} 第 {row} 行音码非法: {err}"));
        let shape_code: ShapeCode = shape
            .parse()
            .unwrap_or_else(|err| panic!("{name} 第 {row} 行形码非法: {err}"));
        let source_weight: u32 = weight
            .parse()
            .unwrap_or_else(|_| panic!("{name} 第 {row} 行来源权重应为 u32: {weight:?}"));
        assert!(
            !source.is_empty()
                && source
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte == b'-'),
            "{name} 第 {row} 行 source 非法: {source:?}"
        );
        assert!(
            !status.is_empty()
                && status
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte == b'-'),
            "{name} 第 {row} 行 status 非法: {status:?}"
        );
        let key = (
            character,
            sound_code,
            shape_code,
            source,
            status,
            source_weight,
        );
        if let Some(previous) = previous {
            assert!(
                previous < key,
                "{name} 第 {row} 行未按声明键严格升序（重复或乱序）: {line:?}"
            );
        }
        previous = Some(key);
        evidence.push(AttestedXhupCode {
            hanzi: InputHanzi(character),
            sound_code,
            shape_code,
            source_weight,
            source,
            status,
        });
    }
    assert_eq!(evidence.len(), 9_796, "{name} evidence 行数漂移");

    let mut records = Vec::new();
    let mut all = Vec::new();
    let mut start = 0;
    while start < evidence.len() {
        let hanzi = evidence[start].hanzi;
        let mut end = start + 1;
        while end < evidence.len() && evidence[end].hanzi == hanzi {
            end += 1;
        }
        records.push(InputRecord {
            hanzi,
            evidence: evidence[start..end].to_vec().into_boxed_slice(),
        });
        all.push(hanzi);
        start = end;
    }
    assert_eq!(records.len(), 8_208, "{name} 输入字符数漂移");
    assert!(all.windows(2).all(|pair| pair[0] < pair[1]));
    for &core in XhupHanzi::all() {
        assert!(
            all.binary_search(&InputHanzi(core.as_char())).is_ok(),
            "规范核心字缺少输入编码事实: {core}"
        );
    }

    AttestedData {
        records: records.into_boxed_slice(),
        all: all.into_boxed_slice(),
        evidence: evidence.into_boxed_slice(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn core_is_a_strict_subset_of_input_universe() {
        assert_eq!(XhupHanzi::all().len(), 8_105);
        assert_eq!(InputHanzi::all().len(), 8_208);
        assert_eq!(
            InputHanzi::all()
                .iter()
                .filter(|input| input.is_core_standard())
                .count(),
            8_105
        );
        assert!(InputHanzi::try_from('嗯').unwrap().is_core_standard());
        assert!(!InputHanzi::try_from('〇').unwrap().is_core_standard());
    }

    #[test]
    fn official_and_legacy_facts_coexist_without_faking_readings() {
        let en = InputHanzi::try_from('嗯').unwrap();
        let accepted: BTreeSet<String> = en
            .attested_codes()
            .iter()
            .map(|entry| entry.full_code().to_string())
            .collect();
        assert_eq!(
            accepted,
            BTreeSet::from(["enkx".into(), "ngkx".into(), "ogkx".into(), "onkx".into()])
        );
        assert_eq!(
            en.core_standard().unwrap().primary_reading().as_str(),
            "n",
            "语言学读音事实不应被输入码覆盖"
        );

        let ei = InputHanzi::try_from('诶').unwrap();
        assert!(
            ei.attested_codes()
                .iter()
                .any(|entry| { entry.full_code().to_string() == "eiyu" && entry.is_official() })
        );
    }

    #[test]
    fn evidence_counts_and_relation_dedup_are_audited() {
        assert_eq!(AttestedXhupCode::all().len(), 9_796);
        let relations: BTreeSet<_> = AttestedXhupCode::all()
            .iter()
            .map(|entry| (entry.hanzi(), entry.sound_code(), entry.shape_code()))
            .collect();
        assert_eq!(relations.len(), 9_794);
    }

    #[test]
    fn parsing_is_strict() {
        assert_eq!("".parse::<InputHanzi>(), Err(InputHanziError::Empty));
        assert_eq!(
            "嗯诶".parse::<InputHanzi>(),
            Err(InputHanziError::MultipleCharacters)
        );
        assert!(InputHanzi::try_from('😀').is_err());
    }
}
