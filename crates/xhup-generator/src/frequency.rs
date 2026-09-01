//! 规范读音频率数据(万象 / RIME-LMDG 提取子集)的解析与查询。
//!
//! 入库 TSV `data/frequency/wanxiang_reading_scores.tsv` 经 `include_str!` 嵌入,
//! 是候选排名的唯一频率证据来源(来源、提取规则与覆盖审计见该目录 README)。
//! 本模块不读写文件、不访问网络;TSV 损坏属于仓库不变量被破坏,解析时 panic
//! 并给出精确行号。
//!
//! 频率数据按 `(规范汉字, 规范读音)` 组织:多音字的每个规范读音有独立分数。
//! 查不到的关系返回分数 `0`——这仅表示「没有万象频率证据」,不表示读音无效;
//! 缺失关系仍是合法的规范编码条目。

use std::sync::OnceLock;

use xhup_core::{HanziReading, XhupHanzi};

/// 入库的规范频率 TSV(唯一事实来源;由仓库自带提取器可复现生成)。
const WANXIANG_TSV: &str = include_str!("../../../data/frequency/wanxiang_reading_scores.tsv");

/// 一行频率记录:`(汉字, 规范读音) -> 聚合分数`。
#[derive(Clone, Copy, Debug)]
struct FrequencyRow {
    hanzi: char,
    reading: &'static str,
    score: u64,
}

/// 解析后的频率表:按 `(汉字, 读音)` 升序,支持二分查找。
struct FrequencyTable {
    rows: Box<[FrequencyRow]>,
}

fn table() -> &'static FrequencyTable {
    static TABLE: OnceLock<FrequencyTable> = OnceLock::new();
    TABLE.get_or_init(|| parse_tsv(WANXIANG_TSV, "wanxiang_reading_scores.tsv"))
}

/// 查询一个规范读音关系的万象聚合分数;无频率证据的关系返回 `0`。
pub(crate) fn reading_score(hanzi: XhupHanzi, reading: HanziReading) -> u64 {
    let key = (hanzi.as_char(), reading.as_str());
    table()
        .rows
        .binary_search_by(|row| (row.hanzi, row.reading).cmp(&key))
        .map(|index| table().rows[index].score)
        .unwrap_or(0)
}

/// 频率表行数(匹配万象的规范读音关系数);用于测试锁定数据集规模。
#[cfg(test)]
pub(crate) fn relation_count() -> usize {
    table().rows.len()
}

/// 解析内嵌 TSV:`#` 开头为注释头;数据行 `汉字<TAB>规范读音<TAB>分数`。
///
/// 校验:恰好三个字段、字符字段恰为一个字符、读音为小写 ASCII、分数为正 u64、
/// 行按 `(汉字, 读音)` 严格升序(同时排除重复)、每行都对应一个真实规范读音关系。
fn parse_tsv(text: &'static str, name: &str) -> FrequencyTable {
    let mut rows: Vec<FrequencyRow> = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let row_number = index + 1;
        if line.starts_with('#') {
            continue;
        }
        let mut fields = line.split('\t');
        let (Some(char_field), Some(reading), Some(score_field), None) =
            (fields.next(), fields.next(), fields.next(), fields.next())
        else {
            panic!("{name} 第 {row_number} 行应为三个 TAB 分隔字段: {line:?}");
        };

        let mut chars = char_field.chars();
        let (Some(hanzi), None) = (chars.next(), chars.next()) else {
            panic!("{name} 第 {row_number} 行字符字段应恰好为一个字符: {char_field:?}");
        };
        assert!(
            !reading.is_empty() && reading.bytes().all(|byte| byte.is_ascii_lowercase()),
            "{name} 第 {row_number} 行读音应为非空小写 ASCII 字母: {reading:?}"
        );
        let score: u64 = score_field
            .parse()
            .unwrap_or_else(|_| panic!("{name} 第 {row_number} 行分数应为 u64: {score_field:?}"));
        assert!(score > 0, "{name} 第 {row_number} 行分数应为正数: {line:?}");

        if let Some(previous) = rows.last() {
            assert!(
                (previous.hanzi, previous.reading) < (hanzi, reading),
                "{name} 第 {row_number} 行未按 (汉字, 读音) 严格升序(重复或乱序): {line:?}"
            );
        }

        // 每行必须对应真实规范读音关系:成员资格由 xhup-core 规范数据唯一承载。
        let canonical = XhupHanzi::try_from(hanzi)
            .unwrap_or_else(|_| panic!("{name} 第 {row_number} 行汉字不在规范清单内: {hanzi:?}"));
        assert!(
            canonical.readings().iter().any(|r| r.as_str() == reading),
            "{name} 第 {row_number} 行读音不是该字的规范读音: {line:?}"
        );

        rows.push(FrequencyRow {
            hanzi,
            reading,
            score,
        });
    }
    assert!(!rows.is_empty(), "{name} 不应为空文件");
    FrequencyTable {
        rows: rows.into_boxed_slice(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hanzi(ch: char) -> XhupHanzi {
        XhupHanzi::try_from(ch).unwrap()
    }

    fn score_of(ch: char, spelling: &str) -> u64 {
        let hanzi = hanzi(ch);
        let reading = hanzi
            .readings()
            .iter()
            .copied()
            .find(|reading| reading.as_str() == spelling)
            .unwrap_or_else(|| panic!("{ch} 应有规范读音 {spelling}"));
        reading_score(hanzi, reading)
    }

    #[test]
    fn relation_count_matches_committed_dataset() {
        assert_eq!(relation_count(), 8544);
    }

    #[test]
    fn rows_are_strictly_ordered_and_unique() {
        let rows = &table().rows;
        for pair in rows.windows(2) {
            assert!((pair[0].hanzi, pair[0].reading) < (pair[1].hanzi, pair[1].reading));
        }
    }

    #[test]
    fn source_sentinel_scores() {
        // 万象聚合哨兵:啊 a 汇总全部声调变体(759839 + 11024 + 1356 + 570 + 600)
        assert_eq!(score_of('啊', "a"), 773389);
        // 行:多音字按读音区分频率证据
        assert!(score_of('行', "xing") > 0);
        assert!(score_of('行', "hang") > 0);
        assert!(score_of('行', "heng") > 0);
        assert_ne!(score_of('行', "xing"), score_of('行', "hang"));
        // 嗯:ńňǹ→n、ng 族聚合
        assert_eq!(score_of('嗯', "n"), 731821 + 3249 + 93);
        assert_eq!(score_of('嗯', "ng"), 434765 + 31262 + 35);
        // 呣:ḿ→m(分解形式 m+U+0300 坏行被忽略)
        assert_eq!(score_of('呣', "m"), 3);
    }

    #[test]
    fn missing_relations_score_zero() {
        // 覆盖审计确认的缺失关系:无频率证据但仍是合法规范读音
        assert_eq!(score_of('呒', "wu"), 0);
        assert_eq!(score_of('哼', "hng"), 0);
        assert_eq!(score_of('欸', "ea"), 0);
    }

    #[test]
    fn coverage_stays_above_threshold() {
        let canonical: usize = XhupHanzi::all()
            .iter()
            .map(|hanzi| hanzi.readings().len())
            .sum();
        assert_eq!(canonical, 8580);
        let matched = relation_count();
        assert!(
            matched * 100 >= canonical * 98,
            "读音级覆盖率应 >= 98%: {matched}/{canonical}"
        );
    }
}
