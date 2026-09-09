//! optimizer v2 PRIMARY 词语简码 Rime 词典的确定性序列化。
//!
//! 词典是 [`crate::primary_shortcuts`] canonical 关系的投影。每条权重由
//! merged_ranking 按 optimizer 的绝对混排位次确定;同码全组权重唯一,
//! 不依赖 TSV/import_tables 顺序或浮点 quality。shortcut 是新增别名,
//! 每个词的完整码关系继续保留。
//!
//! 输出为 UTF-8(写入字节时)、LF 换行、恰好一个末尾换行、无 BOM;行顺序为
//! canonical 序列化顺序(shortcut 长度 → 码 → 词),不承担排名语义。输出不
//! 包含日期、时间、主机、路径等任何易变内容:在相同规范数据与相同
//! xhup-generator 源码(含其 package version)下,生成结果字节级一致。

use crate::primary_shortcuts::canonical_primary_shortcut_entries;

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
    for entry in canonical_primary_shortcut_entries() {
        out.push_str(entry.word());
        out.push('\t');
        out.push_str(&entry.shortcut_code().to_string());
        let weight = crate::merged_ranking::merged_weight(entry.shortcut_code(), entry.word())
            .expect("每条 PRIMARY 简码都应具有 merged ranking 权重");
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
