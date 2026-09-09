//! Flow 引擎词典的确定性序列化(两个专用词典)。
//!
//! - **组句词典 `xhup_flow_flow`**:供隔离的 `table_translator@flow`
//!   (enable_sentence)使用。它包含 hot / extended 词汇证据和全部两键单字
//!   音码原语：已知词改善分段排名，单字保证词表外组合、结构助词与语气字
//!   不会令句子路径中断。primary 静态 translator 的高质量栅栏保护冻结菜单。
//! - **学习词典 `xhup_flow_learn`**(单字 + 词条):供
//!   `table_translator@learn`(enable_sentence **false**、user_dict、
//!   enable_encoder)使用。单字是规则式短语编码的原语(TableEncoder
//!   的 DfsEncode 逐字 TranslateWord);learn translator 关闭组句,
//!   其单字/词条 exact 查询与 primary 静态层完全重合,经 uniquifier
//!   去重后不可见(零冲突)。
//!
//! 两个词典都只含 canonical 完整关系,**显式排除全部简码别名**
//! (一级简码 / canonical v2 PRIMARY + FIXED_FIRST):简码别名是肌肉记忆
//! 入口路由,不应成为组句/学习编码原语。
//!
//! 学习短语编码(encoder rules,内嵌于学习词典):对 4..=20 字学习短语
//! 定义「逐字声码首键」编码 —— 每字取其双拼音码第一键
//! 按字序拼接(5 字 → 5 键),机械可从 canonical 编码推导,无哈希/随机
//! 别名;≥ 5 字的键数 = 字数,与固定词全码键数(4/6/8)天然错开,
//! 4 字组合词可能与 4 键码位重合但受优先级栅栏保护(见 flow_encoder_yaml)。
//!
//! 组句词典的词条权重列携带**万象聚合频率分数**；官网 attested 单字码
//! 在组句语言内获得独立高置信权重（不改写 linguistic reading，也不改变
//! primary 静态排名）。librime `dict_compiler` 建表时对权重取 log，
//! quality 再 exp 还原,组句(`poet`)按句子权重总和比较分段路径 —— 只有
//! 真实频率证据才能让常用词路径压倒垃圾分段路径;排名权重(组内名次
//! 1..N)在同码组间不可比,无法区分高频词与生僻词。学习词典仍沿用各最终化
//! 条目的显式 Rime 权重(排名输出表示,不参与组句)。
//!
//! 行顺序只是确定性序列化顺序,不承担候选排序。输出 UTF-8、LF、恰好一个
//! 末尾换行、无 BOM;不含时间戳/主机/路径等易变内容,相同规范数据与源码
//! 下字节级一致。

use crate::analysis::word_code_analysis_entries;
use crate::char_codes::{canonical_input_char_code_entries, finalized_char_code_entries};
use crate::word_codes::canonical_extended_word_code_entries;
use crate::word_codes::canonical_word_code_entries;

/// 词典名称(组句词典:词条 + 两键单字音码原语)。
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
///   低于 primary 的 initial_quality 栅栏保证静态候选次序不变，用户短语
///   只追加在后
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

/// 生成完整的 Flow 组句 Rime 源词典文本。
///
/// hot / extended 词条携带万象聚合频率，逐字两键音码原语补齐词表外
/// 组合与语气字路径。该 translator 由 `initial_quality` 与 primary 静态层
/// 隔离，因此开放组句只追加候选，不改变冻结 static exact 前缀。
pub fn generate_rime_flow_dictionary() -> String {
    let mut rows: Vec<(String, String, u32)> = Vec::new();
    for entry in finalized_char_code_entries()
        .iter()
        .filter(|entry| entry.code().len() == 2)
    {
        let base = u32::try_from(entry.frequency_score())
            .unwrap_or(u32::MAX / 2)
            .max(1);
        let weight = if entry.is_official() {
            10_000_000u32.saturating_add(base)
        } else {
            base
        };
        rows.push((
            entry.hanzi().as_char().to_string(),
            entry.code().to_string(),
            weight,
        ));
    }
    for entry in word_code_analysis_entries() {
        rows.push((
            entry.word().to_string(),
            entry.code().to_string(),
            u32::try_from(entry.frequency_score()).unwrap_or(u32::MAX),
        ));
    }
    for entry in canonical_extended_word_code_entries() {
        rows.push((
            entry.word().to_string(),
            entry.code().to_string(),
            u32::try_from(entry.frequency_score()).unwrap_or(u32::MAX),
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
    for entry in canonical_input_char_code_entries() {
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

    #[test]
    fn flow_dictionary_contains_all_two_key_character_primitives() {
        let flow = generate_rime_flow_dictionary();
        let rows: Vec<_> = flow
            .lines()
            .skip_while(|line| *line != "...")
            .skip(1)
            .filter(|line| {
                line.split('\t')
                    .next()
                    .is_some_and(|text| text.chars().count() == 1)
            })
            .collect();
        assert_eq!(rows.len(), 9_254);
        for line in &rows {
            let fields: Vec<_> = line.split('\t').collect();
            assert_eq!(fields.len(), 3);
            assert_eq!(fields[0].chars().count(), 1);
            assert_eq!(fields[1].len(), 2);
            assert!(fields[2].parse::<u32>().unwrap() > 0);
        }
        assert!(flow.lines().any(|line| line.starts_with("嗯\tog\t")));
        assert!(flow.lines().any(|line| line.starts_with("诶\tei\t")));
    }

    #[test]
    fn flow_rows_are_complete_unique_and_include_old_top_n_tail() {
        let flow = generate_rime_flow_dictionary();
        let rows: Vec<_> = flow
            .lines()
            .skip_while(|line| *line != "...")
            .skip(1)
            .collect();
        assert_eq!(
            rows.len(),
            9_254
                + word_code_analysis_entries().len()
                + canonical_extended_word_code_entries().len()
        );
        let relations: std::collections::BTreeSet<_> = rows
            .iter()
            .map(|line| {
                let mut fields = line.split('\t');
                (fields.next().unwrap(), fields.next().unwrap())
            })
            .collect();
        assert_eq!(relations.len(), rows.len(), "Flow 不得有重复 (text, code)");
        assert!(
            relations.contains(&("提示词", "tiuici")),
            "旧三字 Top-N 外来源词必须由完整 secondary tier 保留"
        );
    }

    /// 组句词典权重携带万象频率分数(而非排名输出权重):逐条与分析投影
    /// 一致。这是组句质量的根基 —— librime 经 log/exp 往返后以词典权重
    /// 原值作为候选 quality,句权重 = 路径分数之和,频率证据决定分段胜负。
    #[test]
    fn flow_weights_carry_frequency_evidence() {
        let weights = flow_dict_weights();
        for entry in word_code_analysis_entries() {
            assert_eq!(
                *weights
                    .get(entry.word())
                    .unwrap_or_else(|| panic!("词条不在组句词典: {}", entry.word())),
                entry.frequency_score(),
                "组句词典权重应等于万象频率分数: {}",
                entry.word()
            );
        }
        for entry in canonical_extended_word_code_entries() {
            assert_eq!(
                *weights
                    .get(entry.word())
                    .unwrap_or_else(|| panic!("扩展词条不在组句词典: {}", entry.word())),
                entry.frequency_score(),
                "扩展词典权重应等于万象频率分数: {}",
                entry.word()
            );
        }
    }

    /// 回归守卫(组句审计失败根因):常用词路径必须在频率证据上压倒
    /// 垃圾分段路径。我们/时间/工作 的分数必须远高于 我们是/剪发/战功
    /// 等恰好拼出同码序列的生僻分段词,否则组句产出 我们是剪发战功…。
    #[test]
    fn common_words_outweigh_garbage_segmentation() {
        let weights = flow_dict_weights();
        let weight_of = |word: &str| weights.get(word).copied().unwrap_or(0);
        for (common, garbage) in [
            ("我们", "我们是"),
            ("时间", "剪发"),
            ("工作", "战功"),
            ("发展", "做客"),
        ] {
            assert!(
                weight_of(common) > weight_of(garbage) * 10,
                "{common}({}) 的分数应远超垃圾分段词 {garbage}({})",
                weight_of(common),
                weight_of(garbage),
            );
        }
    }

    /// 一次性解析组句词典为 词→权重 映射(测试辅助,避免逐条线性扫描)。
    fn flow_dict_weights() -> std::collections::HashMap<String, u64> {
        generate_rime_flow_dictionary()
            .lines()
            .skip_while(|line| *line != "...")
            .skip(1)
            .map(|line| {
                let mut fields = line.split('\t');
                let word = fields.next().expect("词字段存在").to_string();
                let weight = fields
                    .nth(1)
                    .expect("权重字段存在")
                    .parse()
                    .expect("权重为整数");
                (word, weight)
            })
            .collect()
    }

    /// Flow 词典排除全部简码别名；两键行只能是单字 sound primitive，
    /// 词条仍只使用逐字双拼完整码。
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
        // 全部数据行码长 ∈ {2,4,6,8}(2 键仅单字；其余仅词条全码)。
        for line in dict.lines().skip_while(|l| *l != "...").skip(1) {
            let fields: Vec<&str> = line.split('\t').collect();
            assert_eq!(fields.len(), 3, "每行 词<TAB>码<TAB>权重: {line}");
            let code_len = fields[1].chars().count();
            assert!(
                matches!(code_len, 2 | 4 | 6 | 8),
                "Flow 词典码长应 ∈ {{2,4,6,8}},实际 {code_len}: {line}"
            );
            assert_eq!(fields[0].chars().count() * 2, code_len, "逐字两键: {line}");
        }
    }

    /// 单字原语完整进入隔离的 Flow translator，解除固定词表边界。
    #[test]
    fn includes_all_sound_primitives_without_short_codes() {
        let dict = generate_rime_flow_dictionary();
        let char_rows = dict
            .lines()
            .skip_while(|line| *line != "...")
            .skip(1)
            .filter(|line| {
                line.split('\t')
                    .next()
                    .is_some_and(|text| text.chars().count() == 1)
            })
            .count();
        assert_eq!(char_rows, 9_254);
        assert!(
            !dict
                .lines()
                .any(|line| line.split('\t').nth(1).is_some_and(|code| code.len() == 1))
        );
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
