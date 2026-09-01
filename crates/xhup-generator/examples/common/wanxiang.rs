//! 万象提取器共享的带调拼音归一化。
//!
//! 本文件经 `#[path]` 被多个 extractor example 包含,是归一化规则的唯一实现:
//! 字频与词频提取器因此天然保持 parity,也不向 generator crate 扩张公共 API。

/// 把带调拼音归一化为项目规范无调读音。
///
/// ü 族映射为 ASCII `v`(不是 `u`,ü/v 在 XHUP 语义中区分);
/// 归一化后仍含非小写 ASCII 字母的输入返回 `None`(坏行,显式忽略)。
pub fn normalize_reading(pinyin: &str) -> Option<String> {
    let mut out = String::with_capacity(pinyin.len());
    for ch in pinyin.chars() {
        let mapped = match ch {
            'ā' | 'á' | 'ǎ' | 'à' => 'a',
            'ē' | 'é' | 'ě' | 'è' => 'e',
            'ī' | 'í' | 'ǐ' | 'ì' => 'i',
            'ō' | 'ó' | 'ǒ' | 'ò' => 'o',
            'ū' | 'ú' | 'ǔ' | 'ù' => 'u',
            'ü' | 'ǖ' | 'ǘ' | 'ǚ' | 'ǜ' => 'v',
            'ń' | 'ň' | 'ǹ' => 'n',
            'ḿ' => 'm',
            _ if ch.is_ascii_lowercase() => ch,
            _ => return None,
        };
        out.push(mapped);
    }
    if out.is_empty() { None } else { Some(out) }
}
