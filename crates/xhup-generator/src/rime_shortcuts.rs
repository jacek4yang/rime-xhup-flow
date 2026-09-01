//! 一级简码 Rime 词典的确定性序列化。
//!
//! 词典是 [`crate::shortcuts`] 规范映射的投影:每行 `汉字<TAB>键<TAB>1`。
//! 每个 1 键码恰有一个候选,候选间不存在竞争关系,权重恒为 1(格式与项目
//! 既有生成词典保持一致,不使用魔法大权重)。一级简码只是一键精确候选,
//! 不自动上屏,也不替换任何 2/3/4 码关系;1 键码与单字层(2/3/4 码)、
//! 词语层(4/6/8 键)不存在 exact code 冲突。
//!
//! 输出为 UTF-8(写入字节时)、LF 换行、恰好一个末尾换行、无 BOM;行顺序为
//! canonical 序列化顺序(QWERTY 物理布局),不承担排名语义。输出不包含日期、
//! 时间、主机、路径等任何易变内容:在相同规范数据与相同 xhup-generator 源码
//! (含其 package version)下,生成结果字节级一致。

use crate::shortcuts::canonical_level1_shortcuts;

/// 词典名称。
const DICTIONARY_NAME: &str = "xhup_flow_shortcuts";

/// 生成的一级简码词典文件名(生成器拥有的产物标识,调用方不得自行命名)。
pub const RIME_SHORTCUT_DICTIONARY_FILENAME: &str = "xhup_flow_shortcuts.dict.yaml";

/// 生成完整的一级简码 Rime 源词典文本(26 条一键关系)。
pub fn generate_rime_shortcut_dictionary() -> String {
    let mut out = String::new();
    out.push_str("# Rime dictionary\n");
    out.push_str("# encoding: utf-8\n");
    out.push_str("---\n");
    out.push_str("name: ");
    out.push_str(DICTIONARY_NAME);
    out.push_str("\nversion: \"");
    out.push_str(env!("CARGO_PKG_VERSION"));
    out.push_str("\"\nsort: by_weight\nuse_preset_vocabulary: false\n...\n");
    for entry in canonical_level1_shortcuts() {
        out.push(entry.hanzi().as_char());
        out.push('\t');
        out.push(entry.key().as_char());
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
            RIME_SHORTCUT_DICTIONARY_FILENAME,
            format!("{DICTIONARY_NAME}.dict.yaml")
        );
    }
}
