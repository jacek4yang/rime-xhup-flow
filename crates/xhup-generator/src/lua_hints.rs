//! Lua 简码提示数据(`lua/xhup_flow/data/quick_hints.lua`)的确定性生成。
//!
//! 提示视图 = canonical v2 简码映射(PRIMARY + FIXED_FIRST)按词聚合:
//! 每词保留最简码(键数最小,平手取字典序最小,
//! 确定性)。只收录**严格短于该词全码**的简码 —— 不省键的「简码」没有
//! 提示价值。模块源文件(quick_hint.lua)是入库源码,经 include_str!
//! 嵌入;数据文件由本模块生成。两者都由生成器拥有产物身份。

use crate::fixed_first_shortcuts::canonical_fixed_first_shortcut_entries;
use crate::primary_shortcuts::canonical_primary_shortcut_entries;

/// 候选注释模块产物文件名(入库源码,相对包根)。
pub const LUA_ANNOTATION_FILENAME: &str = "lua/xhup_flow/annotation.lua";

/// 简码提示模块产物文件名(入库源码,相对包根)。
pub const LUA_QUICK_HINT_FILENAME: &str = "lua/xhup_flow/quick_hint.lua";

/// Lua 命名空间入口与合同诊断模块产物文件名(入库源码,相对包根)。
pub const LUA_INIT_FILENAME: &str = "lua/xhup_flow/init.lua";

/// 简码提示数据产物文件名(生成器产出,相对包根)。
pub const LUA_QUICK_HINT_DATA_FILENAME: &str = "lua/xhup_flow/data/quick_hints.lua";

/// 候选注释模块源(入库源码,与模板同法嵌入)。
const ANNOTATION_SOURCE: &str = include_str!("../../../rime/lua/xhup_flow/annotation.lua");

/// 简码提示模块源(入库源码,与模板同法嵌入)。
const QUICK_HINT_SOURCE: &str = include_str!("../../../rime/lua/xhup_flow/quick_hint.lua");

/// Lua 命名空间入口与合同诊断模块源(入库源码,与模板同法嵌入)。
const INIT_SOURCE: &str = include_str!("../../../rime/lua/xhup_flow/init.lua");

/// 候选注释模块源文本(逐字嵌入,未经任何改写)。
pub fn lua_annotation_source() -> &'static str {
    ANNOTATION_SOURCE
}

/// 简码提示模块源文本(逐字嵌入,未经任何改写)。
pub fn lua_quick_hint_source() -> &'static str {
    QUICK_HINT_SOURCE
}

/// Lua 命名空间入口与合同诊断模块源文本(逐字嵌入,未经任何改写)。
pub fn lua_init_source() -> &'static str {
    INIT_SOURCE
}

/// 生成简码提示数据模块文本(Lua table,词 → 最简码)。
///
/// 输出 UTF-8、LF、恰好一个末尾换行;确定性:词按 Unicode 标量升序,
/// 与生成顺序无关。canonical 词只含 CJK 字符,无引号/反斜杠转义问题
/// (生成时硬断言)。
pub fn generate_lua_quick_hints_data() -> String {
    // 词 → (最优简码, 该词全码键数)。
    let mut best: std::collections::BTreeMap<&str, (String, usize)> =
        std::collections::BTreeMap::new();
    fn consider<'w>(
        best: &mut std::collections::BTreeMap<&'w str, (String, usize)>,
        word: &'w str,
        shortcut: &str,
        full_code_len: usize,
    ) {
        let shortcut_len = shortcut.chars().count();
        if shortcut_len >= full_code_len {
            return; // 不省键,无提示价值
        }
        let entry = best
            .entry(word)
            .or_insert_with(|| (shortcut.to_string(), full_code_len));
        let better = shortcut_len < entry.0.chars().count()
            || (shortcut_len == entry.0.chars().count() && shortcut < entry.0.as_str());
        if better {
            entry.0 = shortcut.to_string();
        }
    }
    for entry in canonical_primary_shortcut_entries() {
        consider(
            &mut best,
            entry.word(),
            &entry.shortcut_code().to_string(),
            entry.full_code().len(),
        );
    }
    for entry in canonical_fixed_first_shortcut_entries() {
        consider(
            &mut best,
            entry.word(),
            &entry.shortcut_code().to_string(),
            entry.full_code().len(),
        );
    }

    let mut out = String::new();
    out.push_str("-- 由 xhup-generator 生成,勿手改\n");
    out.push_str("-- canonical 简码映射的提示视图:词 → 最简码(严格短于全码)\n");
    out.push_str("return {\n");
    for (word, (shortcut, _)) in &best {
        assert!(
            !word.contains(['"', '\\']),
            "canonical 词不应含引号/反斜杠: {word}"
        );
        out.push_str(&format!("  [\"{word}\"] = \"{shortcut}\",\n"));
    }
    out.push_str("}\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_is_embedded_verbatim() {
        // 注释模块:包含纯 ASCII 格式化与清洗装饰标记核心逻辑。
        assert!(lua_annotation_source().contains("clean_decorative_markers"));
        assert!(lua_annotation_source().contains("format_shortcut_hint"));
        assert!(lua_annotation_source().contains("debug_tag_for_type"));
        assert!(lua_annotation_source().contains("format_candidate_comment"));

        // 源文件是完整模块本体:命名空间数据引用与组件三要素俱在。
        assert!(lua_quick_hint_source().contains("xhup_flow.data.quick_hints"));
        assert!(lua_quick_hint_source().contains("xhup_flow.annotation"));
        assert!(lua_quick_hint_source().contains("init = init"));
        assert!(lua_quick_hint_source().contains("func = func"));
    }

    #[test]
    fn data_is_deterministic() {
        assert_eq!(
            generate_lua_quick_hints_data(),
            generate_lua_quick_hints_data()
        );
    }

    #[test]
    fn data_carries_known_shortcuts() {
        let data = generate_lua_quick_hints_data();
        // 时间 的 canonical v2 FIXED_FIRST 简码 uij(3 键 < 全码 uijm 4 键)。
        assert!(data.contains("[\"时间\"] = \"uij\""), "应含 时间 → uij");
        // 全码桶检查:每条提示码都严格短于对应全码由生成逻辑保证;
        // 抽查条数与层规模一致(三层合计去重后 ≥ 各层最小值)。
        let entries = data.matches("] = \"").count();
        assert!(entries > 40_000, "提示视图应覆盖简码词主体,实际 {entries}");
    }

    #[test]
    fn every_hint_is_shorter_than_full_code() {
        // 与生产码表交叉:提示词必须存在于词词典且其全码更长。
        let words: std::collections::BTreeMap<String, usize> = crate::canonical_word_code_entries()
            .iter()
            .map(|e| (e.word().to_string(), e.code().len()))
            .collect();
        for line in generate_lua_quick_hints_data().lines() {
            let Some(rest) = line.trim().strip_prefix("[\"") else {
                continue;
            };
            let (word, rest) = rest.split_once("\"] = \"").expect("行格式");
            let hint = rest.strip_suffix("\",").expect("行尾");
            let full_len = words
                .get(word)
                .unwrap_or_else(|| panic!("提示词应在词词典: {word}"));
            assert!(
                hint.chars().count() < *full_len,
                "{word} 的提示码 {hint} 必须严格短于全码({full_len} 键)"
            );
        }
    }
}
