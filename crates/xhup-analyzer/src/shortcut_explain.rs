//! 单词**简码提示**解释卡(Trainer 诊断,Issue #83 §3/§9/§24)。
//!
//! [`crate::explain`] 解释的是「该词在 optimizer v2 下的码位决策」;本模块解释
//! 的是**候选行上实际显示的 `~<简码>` 提示**是否名副其实 —— 即 §3 的硬产品
//! 合同:
//!
//! ```text
//! advertised shortcut => useful shortcut
//! ```
//!
//! 提示视图与 Lua 运行时 `quick_hint` 同一实现([`lua_hints_view_with_full_lens`]),
//! rank 来自 canonical 真实菜单([`CodeOccupancy`]),因此解释卡描述的就是用户
//! 真正看到的行为,不是影子副本。
//!
//! # 卡片内容(纯 ASCII,§7)
//!
//! - 词、提示码、提示码键数、全码键数、按键节省;
//! - 该提示码在真实菜单中的 **rank** 与菜单扇出(fanout);
//! - §3 判定:`USEFUL`(rank 1,输入即首选)/ `SELECT`(在菜单内但需翻页)/
//!   `MISLEADING`(词不在该码菜单 —— 提示无效);
//! - 无提示时的显式说明(不伪造空卡)。
//!
//! # 确定性
//!
//! 视图与占用表全部按进程 `OnceLock` 构建一次;同一词必得同一卡片。

use std::collections::BTreeMap;
use std::sync::OnceLock;

use crate::occupancy::CodeOccupancy;
use crate::shortcut_audit::{ShortcutAuditEntry, ShortcutVerdict};

/// 提示视图 + 全码键数 + 真实菜单占用(进程内构建一次)。
struct HintContext {
    hints: BTreeMap<String, String>,
    full_lens: BTreeMap<String, usize>,
    occupancy: CodeOccupancy,
}

fn hint_context() -> &'static HintContext {
    static CELL: OnceLock<HintContext> = OnceLock::new();
    CELL.get_or_init(|| {
        let (hints, full_lens) = xhup_generator::lua_hints_view_with_full_lens();
        let occupancy = CodeOccupancy::build_current_production();
        HintContext {
            hints,
            full_lens,
            occupancy,
        }
    })
}

/// 一个词的简码提示决策(结构化,便于 Trainer 渲染与测试断言)。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ShortcutHint {
    /// 查询词。
    pub word: String,
    /// 提示码(候选行上显示的 `~<code>`)。
    pub shortcut_code: String,
    /// 提示码键数。
    pub shortcut_len: usize,
    /// 全码键数。
    pub full_code_len: usize,
    /// 输入该提示码时该词的真实菜单 rank(1 起;`None` = 不在该码菜单)。
    pub menu_rank: Option<usize>,
    /// 该码菜单的候选总数。
    pub menu_fanout: usize,
    /// §3 判定。
    pub verdict: ShortcutVerdict,
}

impl ShortcutHint {
    /// 按键节省(全码 − 简码)。
    pub fn keystrokes_saved(&self) -> usize {
        self.full_code_len.saturating_sub(self.shortcut_len)
    }

    /// 提示是否名副其实(rank 1)。
    pub fn is_useful(&self) -> bool {
        self.verdict == ShortcutVerdict::Useful
    }

    /// 渲染纯 ASCII 解释卡(§7:无 emoji、无装饰性 Unicode)。
    pub fn render_card(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("word            {}\n", self.word));
        out.push_str(&format!("hint            ~{}\n", self.shortcut_code));
        out.push_str(&format!(
            "hint keys       {}  (full code {} keys, saves {})\n",
            self.shortcut_len,
            self.full_code_len,
            self.keystrokes_saved()
        ));
        out.push_str(&format!(
            "menu rank       {}\n",
            match self.menu_rank {
                Some(rank) => rank.to_string(),
                None => "absent".to_string(),
            }
        ));
        out.push_str(&format!("menu fanout     {}\n", self.menu_fanout));
        out.push_str(&format!(
            "verdict         {}\n",
            verdict_label(self.verdict)
        ));
        out.push_str("note            ");
        out.push_str(match self.verdict {
            ShortcutVerdict::Useful => {
                "typing the hint code commits this word as the first candidate (issue-83 S3 satisfied)"
            }
            ShortcutVerdict::UsefulWithSelection => {
                "the word is in the menu but not first; the hint costs a page/selection"
            }
            ShortcutVerdict::Misleading => {
                "the word is NOT in this code's menu; the hint is invalid (issue-83 S3 violated)"
            }
        });
        out.push('\n');
        out
    }
}

const fn verdict_label(verdict: ShortcutVerdict) -> &'static str {
    match verdict {
        ShortcutVerdict::Useful => "USEFUL",
        ShortcutVerdict::UsefulWithSelection => "SELECT",
        ShortcutVerdict::Misleading => "MISLEADING",
    }
}

/// 查询一个词的简码提示解释。
///
/// 返回 `None` 当且仅当:
/// - 查询词为空/纯空白;
/// - 该词**没有任何提示**(其候选行不显示 `~<code>`)。
///
/// 注意:「无提示」与「词不存在」是不同情况,调用方应据此给出不同文案
/// (本模块只报告事实,不做文案假设)。
pub fn explain_shortcut_hint(word: &str) -> Option<ShortcutHint> {
    let word = word.trim();
    if word.is_empty() {
        return None;
    }
    let ctx = hint_context();
    let shortcut_code = ctx.hints.get(word)?.clone();
    let full_code_len = ctx.full_lens.get(word).copied()?;
    let shortcut_len = shortcut_code.chars().count();

    // 真实菜单 rank:在该码的占用组里找该词。
    let code = shortcut_code.parse::<xhup_core::KeySequence>().ok()?;
    let group = ctx.occupancy.group(&code);
    let menu_fanout = group.map_or(0, |g| g.len());
    let menu_rank = group.and_then(|g| {
        g.iter()
            .position(|candidate| candidate.text() == word)
            .map(|index| index + 1)
    });

    let entry = ShortcutAuditEntry {
        word: word.to_string(),
        shortcut_code: shortcut_code.clone(),
        full_code_len,
        shortcut_len,
        menu_rank,
        menu_fanout,
    };
    Some(ShortcutHint {
        word: word.to_string(),
        shortcut_code,
        shortcut_len,
        full_code_len,
        menu_rank,
        menu_fanout,
        verdict: entry.verdict(),
    })
}

#[cfg(test)]
mod tests {
    use super::explain_shortcut_hint;

    #[test]
    fn empty_or_whitespace_query_returns_none() {
        assert_eq!(explain_shortcut_hint(""), None);
        assert_eq!(explain_shortcut_hint("   "), None);
    }

    #[test]
    fn unknown_word_has_no_hint_card() {
        assert_eq!(explain_shortcut_hint("xyznotaword"), None);
    }

    #[test]
    fn high_frequency_word_yields_ascii_card() {
        let hint = explain_shortcut_hint("我们").expect("高频词应有提示");
        assert_eq!(hint.word, "我们");
        assert!(!hint.shortcut_code.is_empty());
        assert!(hint.shortcut_len <= hint.full_code_len);
        let card = hint.render_card();
        assert!(card.contains("verdict"));
        assert!(card.contains("我们"), "卡片必须包含查询词本身");
        // §7 的约束是**标签与注释**极简纯 ASCII;**词本身**是用户查询的
        // 中文内容,当然会出现在卡里。因此断言「去掉查询词后余下文本为纯
        // ASCII」,而不是「整张卡为纯 ASCII」。
        let labels = card.replace(&hint.word, "");
        assert!(
            labels.is_ascii(),
            "除查询词外,卡片标签与注释必须是纯 ASCII(§7):{labels}"
        );
        // 提示码本身就是 ASCII 标注(如 ~wm),应出现在卡片里。
        assert!(
            labels.contains(&format!("~{}", hint.shortcut_code)),
            "卡片必须显示提示码本身:{labels}"
        );
    }
}
