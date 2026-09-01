//! 固定层静态词语 Rime 词典的确定性序列化。
//!
//! 词典是 [`crate::word_codes`] 最终化条目集的投影:每行 `词<TAB>码<TAB>权重`,
//! 所有行都携带显式确定性权重(排名证据为唯一贡献读音序列的万象聚合分数;
//! 权重是排名结果的输出表示,不是来源分数本身)。词码为 2 字 4 键、3 字 6 键、
//! 4 字 8 键,逐字规范双拼两码按字序拼接;4 键词码与规范单字全码集严格不相交
//! (提取期按 semantic entry 粒度过滤,最终化期构建级断言)。
//!
//! 输出为 UTF-8(写入字节时)、LF 换行、恰好一个末尾换行、无 BOM;行顺序只是
//! 确定性的**序列化顺序**(码长升序、码字典序升序、权重降序、词 Unicode 标量
//! 升序),不是 Rime 候选排序。输出不包含日期、时间、主机、路径等任何易变内容:
//! 在相同规范数据、相同词语数据与相同 xhup-generator 源码(含其 package
//! version)下,生成结果字节级一致。

use crate::word_codes::finalized_word_code_entries;

/// 词典名称。
const DICTIONARY_NAME: &str = "xhup_flow_words";

/// 生成的固定层词语词典文件名(生成器拥有的产物标识,调用方不得自行命名)。
pub const RIME_WORD_DICTIONARY_FILENAME: &str = "xhup_flow_words.dict.yaml";

/// 生成完整的固定层静态词语 Rime 源词典文本(2~4 字词,4/6/8 键)。
///
/// 序列化 [`crate::word_codes`] 的最终化条目集,语义见模块文档。
pub fn generate_rime_word_dictionary() -> String {
    let mut out = String::new();
    out.push_str("# Rime dictionary\n");
    out.push_str("# encoding: utf-8\n");
    out.push_str("---\n");
    out.push_str("name: ");
    out.push_str(DICTIONARY_NAME);
    out.push_str("\nversion: \"");
    out.push_str(env!("CARGO_PKG_VERSION"));
    out.push_str("\"\nsort: by_weight\nuse_preset_vocabulary: false\n...\n");
    for entry in finalized_word_code_entries() {
        out.push_str(entry.word());
        out.push('\t');
        out.push_str(&entry.code().to_string());
        out.push('\t');
        out.push_str(&entry.rime_weight().to_string());
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
            RIME_WORD_DICTIONARY_FILENAME,
            format!("{DICTIONARY_NAME}.dict.yaml")
        );
    }
}
