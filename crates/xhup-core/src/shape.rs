//! 规范汉字形码:汉字 → 可接受形码集合的只读 canonical 数据层。
//!
//! 内建规范数据经 `include_str!` 嵌入仓库级项目数据 `data/shape/hanzi_shapes.tsv`
//! (唯一事实来源,推导规则与来源见该目录 README);同样假定当前 XHUP Flow 仓库的
//! 目录结构。
//!
//! 形码是字形属性,与读音无关:同一字的多个 [`ShapeCode`] 是上游收录的可接受拆字
//! 变体,**没有首选/主形码语义**(来源无优先级证据)。本层不涉及 FullCode 组合、
//! 权重或字根元数据。

use std::sync::OnceLock;

use crate::{ShapeCode, XhupHanzi};

const SHAPES_TSV: &str = include_str!("../../../data/shape/hanzi_shapes.tsv");

/// 一个规范汉字的形码解析结果:`codes` 按规范键序升序,非空、无重复。
#[derive(Debug)]
struct ShapeRecord {
    hanzi: char,
    codes: Box<[ShapeCode]>,
}

/// 进程级规范形码数据:解析一次,按字符码点排序以供二分。
#[derive(Debug)]
struct CanonicalShapeData {
    records: Box<[ShapeRecord]>,
}

fn canonical() -> &'static CanonicalShapeData {
    static CANONICAL: OnceLock<CanonicalShapeData> = OnceLock::new();
    CANONICAL.get_or_init(|| parse_shapes(SHAPES_TSV, "hanzi_shapes.tsv"))
}

impl XhupHanzi {
    /// 该字的规范形码集合:非空、按规范键序升序、无重复、进程内共享的不可变切片。
    ///
    /// 当前规范不变量:全部 8105 个规范汉字都至少有一个形码,初始化时已逐字
    /// 与 [`XhupHanzi::all()`] 交叉校验。多个形码是被接受的并列变体,顺序为
    /// 规范键序,不代表优先级。
    pub fn shape_codes(self) -> &'static [ShapeCode] {
        let records = &canonical().records;
        let index = records
            .binary_search_by(|record| record.hanzi.cmp(&self.as_char()))
            .expect("XhupHanzi 不变量:必然有形码记录");
        &records[index].codes
    }
}

/// 解析内嵌规范形码表:每行 `字<TAB>形码`,零拷贝构造 [`ShapeCode`]。
/// 校验全部规范格式与排序不变量,并逐字核对字符集合与规范汉字清单一致;
/// 被破坏时 panic,消息含文件名与行号。
fn parse_shapes(text: &'static str, name: &str) -> CanonicalShapeData {
    assert!(!text.is_empty(), "{name} 不应为空文件");

    let mut records: Vec<ShapeRecord> = Vec::new();
    let mut row_count = 0usize;
    let mut group_char: Option<char> = None;
    let mut group_codes: Vec<ShapeCode> = Vec::new();

    for (index, line) in text.lines().enumerate() {
        let row = index + 1;
        row_count += 1;

        let mut fields = line.split('\t');
        let (Some(char_field), Some(code_field), None) =
            (fields.next(), fields.next(), fields.next())
        else {
            panic!("{name} 第 {row} 行应为两个 TAB 分隔字段: {line:?}");
        };
        let mut chars = char_field.chars();
        let (Some(hanzi_char), None) = (chars.next(), chars.next()) else {
            panic!("{name} 第 {row} 行字符字段应恰好为一个字符: {char_field:?}");
        };
        let code: ShapeCode = code_field
            .parse()
            .unwrap_or_else(|err| panic!("{name} 第 {row} 行形码非法: {code_field:?}({err})"));

        if group_char != Some(hanzi_char) {
            if let Some(previous) = group_char.take() {
                push_group(&mut records, previous, &mut group_codes);
                assert!(
                    hanzi_char > previous,
                    "{name} 第 {row} 行字符组未按 Unicode 标量值严格升序(重复或乱序): {hanzi_char:?}"
                );
            }
            group_char = Some(hanzi_char);
        }
        if let Some(last) = group_codes.last() {
            assert!(
                *last < code,
                "{name} 第 {row} 行形码未按键序严格升序(重复或乱序): {line:?}"
            );
        }
        group_codes.push(code);
    }

    if let Some(last_char) = group_char.take() {
        push_group(&mut records, last_char, &mut group_codes);
    }

    assert!(
        records.len() == 8105,
        "{name} 应恰好包含 8105 个规范汉字,实际 {} 个",
        records.len()
    );
    assert!(
        row_count == 8666,
        "{name} 应恰好包含 8666 行,实际 {row_count} 行"
    );

    // 与规范汉字清单逐字交叉校验:形码表必须恰好覆盖 XhupHanzi 全域。
    let all = XhupHanzi::all();
    assert!(
        records.len() == all.len(),
        "{name} 字符数 {} 与规范汉字清单 {} 不一致",
        records.len(),
        all.len()
    );
    for (index, (record, hanzi)) in records.iter().zip(all.iter()).enumerate() {
        assert!(
            record.hanzi == hanzi.as_char(),
            "{name} 第 {} 个字符组与规范汉字清单不一致: {:?} != {:?}",
            index + 1,
            record.hanzi,
            hanzi.as_char()
        );
    }

    CanonicalShapeData {
        records: records.into_boxed_slice(),
    }
}

/// 结束一个字符组:组在首行到达时创建,必然非空。
fn push_group(records: &mut Vec<ShapeRecord>, hanzi_char: char, codes: &mut Vec<ShapeCode>) {
    debug_assert!(!codes.is_empty());
    records.push(ShapeRecord {
        hanzi: hanzi_char,
        codes: std::mem::take(codes).into_boxed_slice(),
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hanzi(ch: char) -> XhupHanzi {
        XhupHanzi::try_from(ch).unwrap()
    }

    fn code_strs(hanzi: XhupHanzi) -> Vec<String> {
        hanzi
            .shape_codes()
            .iter()
            .map(|code| code.to_string())
            .collect()
    }

    #[test]
    fn sentinel_shape_sets() {
        let cases: [(char, &[&str]); 16] = [
            ('啊', &["kd", "kk"]),
            ('阿', &["ed", "ek"]),
            ('鞍', &["gn", "nn"]),
            ('贯', &["gr", "tr", "vr"]),
            ('到', &["ad", "vd"]),
            ('恩', &["kx", "yx"]),
            ('后', &["ik", "pk"]),
            ('将', &["dc", "pc"]),
            ('年', &["pl", "rl"]),
            ('日', &["ka", "la"]),
            ('吃', &["kq", "ky"]),
            ('快', &["xg", "xn"]),
            ('却', &["qe", "te"]),
            ('主', &["dw", "wt"]),
            ('内', &["kr", "ld"]),
            ('蜣', &["iq", "iv"]),
        ];
        for (ch, expected) in cases {
            assert_eq!(code_strs(hanzi(ch)), expected, "{ch}");
        }
    }

    #[test]
    fn every_canonical_hanzi_has_nonempty_sorted_unique_shape_codes() {
        for &hanzi in XhupHanzi::all() {
            let codes = hanzi.shape_codes();
            assert!(!codes.is_empty(), "{:?} 缺少形码", hanzi.as_char());
            assert!(codes.windows(2).all(|w| w[0] < w[1]));
        }
    }

    #[test]
    fn canonical_shape_counts_match_audit() {
        let mut total = 0usize;
        let mut distinct = std::collections::HashSet::new();
        let mut cardinality = std::collections::HashMap::new();
        let mut three_shaped = Vec::new();
        for &hanzi in XhupHanzi::all() {
            let codes = hanzi.shape_codes();
            total += codes.len();
            distinct.extend(codes.iter().map(|code| code.to_string()));
            *cardinality.entry(codes.len()).or_insert(0usize) += 1;
            if codes.len() == 3 {
                three_shaped.push(hanzi.as_char());
            }
        }
        assert_eq!(total, 8666);
        assert_eq!(distinct.len(), 670);
        assert_eq!(
            cardinality,
            [(1, 7545), (2, 559), (3, 1)].into_iter().collect()
        );
        assert_eq!(three_shaped, ['贯']);
    }

    #[test]
    fn shape_codes_are_stable_process_lifetime_slices() {
        let first = hanzi('啊').shape_codes();
        let second = hanzi('啊').shape_codes();
        assert!(std::ptr::eq(first, second));
    }

    #[test]
    fn shape_code_display_round_trips() {
        for &code in hanzi('贯').shape_codes() {
            let reparsed: ShapeCode = code.to_string().parse().unwrap();
            assert_eq!(reparsed, code);
        }
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
            ("啊\tkd\n啊\n", 2),     // 字段数不足
            ("啊\tkd\textra\n", 1),  // 字段数过多
            ("啊啊\tkd\n", 1),       // 字符字段多个字符
            ("啊\tk\n", 1),          // 形码长度非法
            ("啊\tKD\n", 1),         // 形码非小写 ASCII
            ("中\tkd\n一\tkk\n", 2), // 字符组乱序(中 U+4E2D > 一 U+4E00)
            ("一\tkk\n一\tkd\n", 2), // 组内形码乱序
            ("一\tkd\n一\tkd\n", 2), // 组内形码重复
        ] {
            let payload = std::panic::catch_unwind(|| parse_shapes(text, "test.tsv")).unwrap_err();
            let message = panic_message(payload);
            assert!(message.contains("test.tsv"), "{message}");
            assert!(message.contains(&format!("第 {row} 行")), "{message}");
        }
    }
}
