//! 规范单字全码 Rime 词典的推导与确定性序列化。
//!
//! 本模块把 `xhup-core` 的规范数据(可编码规范读音 × 规范形码)投影为
//! 唯一的 `(规范汉字, 全码)` 语义条目,再序列化为完整的 Rime 源词典文本。
//!
//! 条目的输出顺序只是确定性的**序列化顺序**(汉字按 Unicode 码点升序、
//! 同字内全码按键序升序),不代表 Rime 候选排序、频率或首选读音/形码。
//! 输出不包含日期、时间、主机、路径等任何易变内容:在相同规范数据与相同
//! xhup-generator 源码(含其 package version)下,生成结果字节级一致。

use std::collections::BTreeSet;

use xhup_core::{FullCode, XhupHanzi};

/// 词典名称。
const DICTIONARY_NAME: &str = "xhup_flow_chars";

/// 一条生成的规范单字全码关系。
///
/// 表示一个规范汉字的一个可接受全码。条目不携带权重或优先级;其在词典
/// 中的位置仅是序列化顺序,不是 Rime 候选排序。
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct RimeCharEntry {
    hanzi: XhupHanzi,
    code: FullCode,
}

impl RimeCharEntry {
    /// 该条目对应的规范汉字。
    pub fn hanzi(self) -> XhupHanzi {
        self.hanzi
    }

    /// 该条目对应的全码。
    pub fn code(self) -> FullCode {
        self.code
    }
}

/// 从规范数据推导全部单字全码条目。
///
/// 对每字取「可编码规范读音 × 规范形码」并按键序去重:归一到同一双拼码
/// 的不同读音(如「咯」的 lo/luo)自然合并为一条;不同汉字共享同一全码
/// 的重码关系保持不变;无规范可编码读音的汉字(如「呣」「嗯」)不产生条目。
///
/// 返回顺序:汉字 Unicode 码点升序,同字内全码升序。
pub fn canonical_char_entries() -> Vec<RimeCharEntry> {
    let mut entries = Vec::new();
    for &hanzi in XhupHanzi::all() {
        let mut codes = BTreeSet::new();
        for &reading in hanzi.readings() {
            let Some(syllable) = reading.to_input_syllable() else {
                continue;
            };
            let sound = syllable.to_double_pinyin_code();
            for &shape in hanzi.shape_codes() {
                codes.insert(FullCode::from_parts(sound, shape));
            }
        }
        entries.extend(codes.into_iter().map(|code| RimeCharEntry { hanzi, code }));
    }
    entries
}

/// 生成完整的规范单字全码 Rime 源词典文本。
///
/// 输出为 UTF-8(写入字节时)、LF 换行、恰好一个末尾换行、无 BOM;
/// `sort: by_weight` 且不输出权重列——本词典不承诺任何候选排序。
pub fn generate_rime_char_dictionary() -> String {
    render_char_dictionary(&canonical_char_entries())
}

/// 把单字全码条目序列化为 Rime 源词典文本。
fn render_char_dictionary(entries: &[RimeCharEntry]) -> String {
    let mut out = String::new();
    out.push_str("# Rime dictionary\n");
    out.push_str("# encoding: utf-8\n");
    out.push_str("---\n");
    out.push_str("name: ");
    out.push_str(DICTIONARY_NAME);
    out.push_str("\nversion: \"");
    out.push_str(env!("CARGO_PKG_VERSION"));
    out.push_str("\"\nsort: by_weight\nuse_preset_vocabulary: false\n...\n");
    for entry in entries {
        out.push(entry.hanzi().as_char());
        out.push('\t');
        out.push_str(&entry.code().to_string());
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn codes_of(ch: char) -> BTreeSet<FullCode> {
        let hanzi = XhupHanzi::try_from(ch).unwrap();
        canonical_char_entries()
            .into_iter()
            .filter(|entry| entry.hanzi() == hanzi)
            .map(RimeCharEntry::code)
            .collect()
    }

    fn parse_codes(codes: &[&str]) -> BTreeSet<FullCode> {
        codes
            .iter()
            .map(|s| s.parse::<FullCode>().unwrap())
            .collect()
    }

    #[test]
    fn entry_counts_match_canonical_audit() {
        let entries = canonical_char_entries();
        assert_eq!(entries.len(), 9158);
        let hanzi: BTreeSet<_> = entries.iter().copied().map(RimeCharEntry::hanzi).collect();
        assert_eq!(hanzi.len(), 8103);
        let codes: BTreeSet<_> = entries.iter().copied().map(RimeCharEntry::code).collect();
        assert_eq!(codes.len(), 8416);
    }

    #[test]
    fn entries_are_strictly_ordered_without_duplicates() {
        let entries = canonical_char_entries();
        for pair in entries.windows(2) {
            assert!(pair[0] < pair[1]);
        }
    }

    #[test]
    fn sentinel_full_code_sets() {
        assert_eq!(codes_of('啊'), parse_codes(&["aakd", "aakk"]));
        assert_eq!(
            codes_of('阿'),
            parse_codes(&["aaed", "aaek", "eeed", "eeek"])
        );
        assert_eq!(codes_of('贯'), parse_codes(&["grgr", "grtr", "grvr"]));
        assert_eq!(codes_of('欻'), parse_codes(&["ixhr", "xuhr"]));
        assert_eq!(codes_of('行'), parse_codes(&["hgii", "hhii", "xkii"]));
        assert_eq!(codes_of('长'), parse_codes(&["ihpn", "vhpn"]));
    }

    #[test]
    fn ge_lo_luo_collapse_deduplicates_generically() {
        // 「咯」的 lo/luo 两个规范读音归一到同一双拼码,4 个原始组合去重为 3 条。
        assert_eq!(codes_of('咯').len(), 3);
    }

    #[test]
    fn zero_encodable_reading_hanzi_produce_no_entries() {
        assert!(codes_of('呣').is_empty());
        assert!(codes_of('嗯').is_empty());
    }

    #[test]
    fn full_code_collisions_across_hanzi_are_preserved() {
        let jumk: FullCode = "jumk".parse().unwrap();
        let hanzi: BTreeSet<char> = canonical_char_entries()
            .into_iter()
            .filter(|entry| entry.code() == jumk)
            .map(|entry| entry.hanzi().as_char())
            .collect();
        let expected: BTreeSet<char> = ['枸', '桔', '椐', '橘', '驹'].into_iter().collect();
        assert_eq!(hanzi, expected);
    }
}
