//! 高稳健零冲突词语简码 Rime 词典的确定性序列化。
//!
//! 词典是 [`crate::word_shortcuts`] canonical 关系的投影:每行
//! `词<TAB>shortcut 码<TAB>1`。production selection 的不变量保证一个
//! shortcut exact code 恰对应一个词,且与 baseline fixed exact code 完全
//! 不相交,因此不存在 exact-code 候选竞争,权重恒为 1(格式与项目既有
//! 生成词典一致,不使用魔法大权重)。shortcut 是新增别名:每个词的完整码
//! 关系在固定词层中完整保留。
//!
//! 输出为 UTF-8(写入字节时)、LF 换行、恰好一个末尾换行、无 BOM;行顺序为
//! canonical 序列化顺序(shortcut 长度 → 码 → 词),不承担排名语义。输出不
//! 包含日期、时间、主机、路径等任何易变内容:在相同规范数据与相同
//! xhup-generator 源码(含其 package version)下,生成结果字节级一致。

use crate::word_shortcuts::canonical_word_shortcut_entries;

/// 词典名称。
const DICTIONARY_NAME: &str = "xhup_flow_word_shortcuts";

/// 生成的词语简码词典文件名(生成器拥有的产物标识,调用方不得自行命名)。
pub const RIME_WORD_SHORTCUT_DICTIONARY_FILENAME: &str = "xhup_flow_word_shortcuts.dict.yaml";

/// 生成完整的词语简码 Rime 源词典文本。
pub fn generate_rime_word_shortcut_dictionary() -> String {
    let mut out = String::new();
    out.push_str("# Rime dictionary\n");
    out.push_str("# encoding: utf-8\n");
    out.push_str("---\n");
    out.push_str("name: ");
    out.push_str(DICTIONARY_NAME);
    out.push_str("\nversion: \"");
    out.push_str(env!("CARGO_PKG_VERSION"));
    out.push_str("\"\nsort: by_weight\nuse_preset_vocabulary: false\n...\n");
    for entry in canonical_word_shortcut_entries() {
        out.push_str(entry.word());
        out.push('\t');
        out.push_str(&entry.shortcut_code().to_string());
        out.push_str("\t1\n");
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
            RIME_WORD_SHORTCUT_DICTIONARY_FILENAME,
            format!("{DICTIONARY_NAME}.dict.yaml")
        );
    }

    #[test]
    fn generation_is_byte_identical() {
        assert_eq!(
            generate_rime_word_shortcut_dictionary(),
            generate_rime_word_shortcut_dictionary()
        );
    }
}
