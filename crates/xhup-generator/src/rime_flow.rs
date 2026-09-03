//! Flow 引擎词典的确定性序列化(两个专用词典)。
//!
//! - **组句词典 `xhup_flow_flow`**(仅词条):供 `table_translator@flow`
//!   (enable_sentence)使用。连续输入的句子只由**词**组成 —— 高质量
//!   分段、rank-1 质量;若含单字,`enable_sentence` 会对任意键序列产生
//!   逐字退化分段句子(如 4 键全码被拆成 4 个单字),污染全部静态
//!   exact 菜单。
//! - **学习词典 `xhup_flow_learn`**(单字 + 词条):供
//!   `table_translator@learn`(enable_sentence **false**、user_dict、
//!   enable_encoder)使用。单字是规则式短语编码的原语(TableEncoder
//!   的 DfsEncode 逐字 TranslateWord);learn translator 关闭组句,
//!   其单字/词条 exact 查询与 primary 静态层完全重合,经 uniquifier
//!   去重后不可见(零冲突)。
//!
//! 两个词典都只含 canonical 全码关系,**显式排除全部简码别名**
//! (一级简码 / ZR / FIXED_FIRST / 二码零冲突):简码别名是肌肉记忆
//! 入口路由,不应成为组句/学习编码原语。
//!
//! 学习短语编码(encoder rules,内嵌于学习词典):对 4..=20 字学习短语
//! 定义「逐字声码首键」编码 —— 每字取其双拼音码第一键
//! 按字序拼接(5 字 → 5 键),机械可从 canonical 编码推导,无哈希/随机
//! 别名;≥ 5 字的键数 = 字数,与固定词全码键数(4/6/8)天然错开,
//! 4 字组合词可能与 4 键码位重合但受优先级栅栏保护(见 flow_encoder_yaml)。
//!
//! 权重沿用各最终化条目的显式 Rime 权重(排名输出表示);行顺序只是确定性
//! 序列化顺序,不承担候选排序。输出 UTF-8、LF、恰好一个末尾换行、无 BOM;
//! 不含时间戳/主机/路径等易变内容,相同规范数据与源码下字节级一致。

use crate::char_codes::canonical_char_code_entries;
use crate::word_codes::canonical_word_code_entries;

/// 词典名称(组句词典:仅词条,无单字)。
const DICTIONARY_NAME: &str = "xhup_flow_flow";

/// 生成的 Flow 组句词典文件名(生成器拥有的产物标识,调用方不得自行命名)。
pub const RIME_FLOW_DICTIONARY_FILENAME: &str = "xhup_flow_flow.dict.yaml";

/// 学习词典名称(单字 + 词条,encoder 用)。
const LEARN_DICTIONARY_NAME: &str = "xhup_flow_learn";

/// 生成的 Flow 学习词典文件名。
pub const RIME_LEARN_DICTIONARY_FILENAME: &str = "xhup_flow_learn.dict.yaml";

/// 词典内 encoder rules(组句/学习短语的确定性编码规则)。
///
/// 短语编码采用「逐字声码首键」:每字取其双拼音码第一键,按字序拼接
/// (N 字短语 → N 键),机械可从 canonical 编码推导,不引入哈希/随机
/// 别名。覆盖 4..=20 字:
///
/// - 4 字:组合词(如 我们+时间 → 我们时间,码 = 四字首键 4 键)。
///   固定词层只收录单词条,组合出的 4 字短语不在其中;其 4 键码可能
///   命中既有 4 键码位(单字全码 / 2 字词),但 learn translator 的
///   initial_quality 0 保证静态候选次序不变,用户短语只追加在后
///   (与 FIXED_FIRST 同构的追加语义);
/// - ≥ 5 字:超出固定词层长度,键数 = 字数,与固定词全码键数
///   (4/6/8)天然错开。
///
/// 规则公式使用 librime TableEncoder 坐标语法:`A` = 首字、`B`/`C`/…
/// = 第 2/3/…字、`Z` = 末字;小写 `a` = 该字码的第一键。每个长度一条
/// 规则,覆盖 4..=20 字;超过 20 字的长句由 sentence 组句覆盖,不单独
/// 造短语词条(与 librime max_phrase_length 语义对齐)。
pub const FLOW_ENCODER_MAX_PHRASE_LENGTH: usize = 20;

/// 学习短语编码的最短长度(4 字组合词起)。
pub const FLOW_ENCODER_MIN_PHRASE_LENGTH: usize = 4;

/// 生成学习词典的 encoder 段(YAML 片段,供词典模板引用)。
pub fn flow_encoder_yaml() -> String {
    let mut out = String::new();
    out.push_str("encoder:\n");
    out.push_str("  exclude_patterns:\n");
    out.push_str("    - '^z.*$'\n");
    out.push_str("  rules:\n");
    for length in FLOW_ENCODER_MIN_PHRASE_LENGTH..=FLOW_ENCODER_MAX_PHRASE_LENGTH {
        // 逐字声码首键:每字坐标 → 首键。
        let formula: String = (0..length)
            .map(|index| {
                let char_pos = if index == length - 1 {
                    'Z' // 末字
                } else {
                    // A=首字,B=第 2 字,…
                    char::from(b'A' + index as u8)
                };
                format!("{char_pos}a")
            })
            .collect();
        out.push_str(&format!(
            "    - length: {length}\n      formula: \"{formula}\"\n"
        ));
    }
    out
}

/// 生成完整的 Flow 组句 Rime 源词典文本(仅词条全码,无单字)。
///
/// 语义见模块文档:供 `table_translator@flow`(enable_sentence)使用;
/// 句子只由词组成,杜绝逐字退化分段。
pub fn generate_rime_flow_dictionary() -> String {
    let mut rows: Vec<(String, String, u32)> = Vec::new();
    for entry in canonical_word_code_entries() {
        rows.push((
            entry.word().to_string(),
            entry.code().to_string(),
            entry.weight(),
        ));
    }
    rows.sort_by(|a, b| {
        a.1.chars()
            .count()
            .cmp(&b.1.chars().count())
            .then(a.1.cmp(&b.1))
            .then(b.2.cmp(&a.2))
            .then(a.0.cmp(&b.0))
    });
    render_dictionary(DICTIONARY_NAME, &rows, None)
}

/// 生成完整的 Flow 学习 Rime 源词典文本(单字 + 词条全码 + encoder)。
///
/// 供 `table_translator@learn`(enable_sentence **false**, user_dict,
/// enable_encoder)使用:单字条目是规则式短语编码的原语
/// (TableEncoder 的 DfsEncode 逐字 TranslateWord,经 reverse 词典解析);
/// 单字不会造成退化句子,因为 learn translator 关闭组句,其单字
/// exact 查询与 primary 静态单字完全重合,经 uniquifier 去重后不可见。
pub fn generate_rime_learn_dictionary() -> String {
    let mut rows: Vec<(String, String, u32)> = Vec::new();
    for entry in canonical_char_code_entries() {
        rows.push((
            entry.hanzi().as_char().to_string(),
            entry.code().to_string(),
            entry.weight(),
        ));
    }
    for entry in canonical_word_code_entries() {
        rows.push((
            entry.word().to_string(),
            entry.code().to_string(),
            entry.weight(),
        ));
    }
    rows.sort_by(|a, b| {
        a.1.chars()
            .count()
            .cmp(&b.1.chars().count())
            .then(a.1.cmp(&b.1))
            .then(b.2.cmp(&a.2))
            .then(a.0.cmp(&b.0))
    });
    render_dictionary(LEARN_DICTIONARY_NAME, &rows, Some(&flow_encoder_yaml()))
}

/// 词典 YAML 渲染共享(确定性;encoder 段可选)。
fn render_dictionary(
    name: &str,
    rows: &[(String, String, u32)],
    encoder_yaml: Option<&str>,
) -> String {
    let mut out = String::new();
    out.push_str("# Rime dictionary\n");
    out.push_str("# encoding: utf-8\n");
    out.push_str("---\n");
    out.push_str("name: ");
    out.push_str(name);
    out.push_str("\nversion: \"");
    out.push_str(env!("CARGO_PKG_VERSION"));
    out.push_str("\"\nsort: by_weight\nuse_preset_vocabulary: false\n");
    if let Some(encoder) = encoder_yaml {
        out.push_str(encoder);
    }
    out.push_str("...\n");
    for (text, code, weight) in rows {
        out.push_str(text);
        out.push('\t');
        out.push_str(code);
        out.push('\t');
        out.push_str(&weight.to_string());
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dictionary_filename_matches_dictionary_name() {
        // 产物文件名由词典名机械派生,防止两者漂移。
        assert_eq!(
            RIME_FLOW_DICTIONARY_FILENAME,
            format!("{DICTIONARY_NAME}.dict.yaml")
        );
    }

    #[test]
    fn generation_is_byte_identical() {
        assert_eq!(
            generate_rime_flow_dictionary(),
            generate_rime_flow_dictionary(),
            "两次生成字节级一致"
        );
    }

    /// Flow 词典排除全部简码别名:不含一级简码行(1 键),行数 = 单字关系
    /// + 固定词关系;抽查典型简码词条目不在词典中。
    #[test]
    fn excludes_shortcut_aliases() {
        let dict = generate_rime_flow_dictionary();
        // 时间 的全码 uijm 在词典中(固定词),但其 ZR/FF 简码 uij/ujm 不在
        //(3 键词行只来自固定词全码 = 6 键,不存在 3 键词行;uij 只可能是
        // 单字 3 码或被排除的别名)。
        assert!(dict.contains("时间\tuijm\t"), "固定词全码必须在词典中");
        assert!(
            !dict.contains("时间\tuij\t"),
            "ZR/FF 简码别名不得进入 Flow 词典"
        );
        assert!(
            !dict.contains("时间\tujm\t"),
            "非单调/FF 简码别名不得进入 Flow 词典"
        );
        assert!(
            !dict.contains("记得\tjd\t"),
            "二码零冲突别名不得进入 Flow 词典"
        );
        // 全部数据行码长 ∈ {4,6,8}(仅词条全码;无单字,无简码别名)。
        for line in dict.lines().skip_while(|l| *l != "...").skip(1) {
            let fields: Vec<&str> = line.split('\t').collect();
            assert_eq!(fields.len(), 3, "每行 词<TAB>码<TAB>权重: {line}");
            let code_len = fields[1].chars().count();
            assert!(
                matches!(code_len, 4 | 6 | 8),
                "Flow 词典码长应 ∈ {{4,6,8}},实际 {code_len}: {line}"
            );
        }
    }

    /// Flow 词典不含单字条目(架构决策:防止逐字退化分段句子)。
    #[test]
    fn excludes_char_entries() {
        let dict = generate_rime_flow_dictionary();
        // 数据行的词字段不得是单字(恰一个汉字)。
        for line in dict.lines().skip_while(|l| *l != "...").skip(1) {
            let word = line.split('\t').next().expect("词字段存在");
            assert!(
                word.chars().count() >= 2,
                "单字条目不得进入组句词典: {word}"
            );
        }
    }

    /// 学习词典:单字 + 词条,含 encoder 段。
    #[test]
    fn learn_dictionary_contains_chars_and_words() {
        let dict = generate_rime_learn_dictionary();
        // 单字条目存在(抽查:啊 的 2 键码 aa)。
        assert!(dict.contains("\taa\t"), "学习词典应含单字条目(如 啊 aa)");
        // 词条存在(抽查:我们 womf)。
        assert!(dict.contains("我们\twomf\t"), "学习词典应含词条");
        // encoder 段存在。
        assert!(dict.contains("encoder:"), "学习词典应含 encoder 段");
        assert!(dict.contains("formula: \"AaBaCaDaZa\""));
        // 简码别名排除不变。
        assert!(
            !dict.contains("时间\tuij\t") && !dict.contains("记得\tjd\t"),
            "简码别名不得进入学习词典"
        );
    }

    /// encoder 规则:逐字声码首键,公式坐标正确。
    #[test]
    fn encoder_rules_are_per_char_initials() {
        let yaml = flow_encoder_yaml();
        // 4 字规则:AaBaCaZa(组合词,如 我们时间 → 四字首键)。
        assert!(yaml.contains("length: 4\n      formula: \"AaBaCaZa\""));
        // 5 字规则:AaBaCaDaZa。
        assert!(yaml.contains("length: 5\n      formula: \"AaBaCaDaZa\""));
        // 20 字规则以 Za 结尾。
        assert!(
            yaml.contains(
                "length: 20\n      formula: \"AaBaCaDaEaFaGaHaIaJaKaLaMaNaOaPaQaRaSaZa\""
            )
        );
        // 规则条数 = 4..=20。
        let count = yaml.matches("    - length: ").count();
        assert_eq!(
            count,
            FLOW_ENCODER_MAX_PHRASE_LENGTH - FLOW_ENCODER_MIN_PHRASE_LENGTH + 1
        );
    }
}
