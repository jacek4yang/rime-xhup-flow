//! 语义编码:XHUP 规范输入音节 → 小鹤双拼两键编码。
//!
//! 本模块把 [`XhupInputSyllable`](清单成员层)连接到
//! [`DoublePinyinLayout`](键盘布局层),不削弱任一层的边界:
//! 音节成员资格由规范清单保证,键盘组合仍只是结构操作(见 `layout.rs`)。
//!
//! 分解规则全部由规范数据驱动,不复制键位映射表:
//!
//! 1. 零声母音节使用 `zero_initials.tsv` 的显式映射(如 `ang→ah`);
//! 2. 其余音节按最长前缀原则取规范声母(`zh`/`ch`/`sh` 优先于 `z`/`c`/`s`),
//!    剩余部分必须是规范韵母,再经 [`DoublePinyinLayout::compose`] 结构组合。
//!
//! 每个可构造的 [`XhupInputSyllable`] 都必须可编码。无法分解或组合说明内嵌
//! 规范数据自相矛盾——那是仓库构建不变量被破坏,而非普通用户输入错误,
//! 此时 panic 并指出违规音节。

use crate::code::DoublePinyinCode;
use crate::input_syllable::XhupInputSyllable;
use crate::layout::DoublePinyinLayout;

impl XhupInputSyllable {
    /// 编码为小鹤双拼两键编码。
    ///
    /// 零声母音节使用规范显式映射;其余音节按「最长规范声母前缀 + 规范韵母」
    /// 分解后结构组合。对已合法的规范输入音节不可失败。
    ///
    /// # Panics
    ///
    /// 仅当内嵌规范清单与规范布局数据不一致(仓库不变量被破坏)时 panic,
    /// 消息含违规音节。
    pub fn to_double_pinyin_code(self) -> DoublePinyinCode {
        let layout = DoublePinyinLayout::canonical();
        let spelling = self.as_str();
        if let Some(code) = layout.zero_initial_code(spelling) {
            return code;
        }
        let (initial, final_) = decompose(spelling).unwrap_or_else(|| {
            panic!("规范输入音节无法分解编码: {spelling:?}(规范清单与规范布局数据不一致)")
        });
        layout.compose(initial, final_).unwrap_or_else(|| {
            panic!("规范输入音节无法组合编码: {spelling:?}(规范清单与规范布局数据不一致)")
        })
    }
}

/// 按最长前缀原则分解出规范声母与规范韵母。
///
/// 线性扫描布局自有的 23 个规范声母,零分配;候选中声母更长者胜,
/// 且剩余部分必须是规范韵母。零声母音节应在上游先行处理,不走此路径。
fn decompose(spelling: &str) -> Option<(&str, &str)> {
    let layout = DoublePinyinLayout::canonical();
    let mut best: Option<(&str, &str)> = None;
    for mapping in layout.initials() {
        let initial = mapping.initial();
        let Some(remainder) = spelling.strip_prefix(initial) else {
            continue;
        };
        if remainder.is_empty() || layout.final_key(remainder).is_none() {
            continue;
        }
        if best.is_none_or(|(current, _)| initial.len() > current.len()) {
            best = Some((initial, remainder));
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encode(input: &str) -> DoublePinyinCode {
        input
            .parse::<XhupInputSyllable>()
            .unwrap()
            .to_double_pinyin_code()
    }

    #[test]
    fn zero_initial_syllables_use_explicit_mapping() {
        // 布局零声母表共 12 行,但 ei、eng 独立音节不在 406 清单内,
        // 此处只覆盖清单成员
        for (input, expected) in [
            ("a", "aa"),
            ("ai", "ai"),
            ("an", "an"),
            ("ang", "ah"),
            ("ao", "ao"),
            ("e", "ee"),
            ("en", "en"),
            ("er", "er"),
            ("o", "oo"),
            ("ou", "ou"),
        ] {
            assert_eq!(encode(input).to_string(), expected, "{input}");
        }
    }

    #[test]
    fn multi_character_initials_win_over_single_letter() {
        // zh/ch/sh 必须优先于 z/c/s
        assert_eq!(encode("zhang").to_string(), "vh");
        assert_eq!(encode("chua").to_string(), "ix");
        assert_eq!(encode("shui").to_string(), "uv");
        assert_eq!(encode("shei").to_string(), "uw");
    }

    #[test]
    fn ordinary_and_edge_syllables_match_repository_behavior() {
        for (input, expected) in [
            ("ba", "ba"),
            ("bing", "bk"),
            ("dia", "dx"),
            ("kei", "kw"),
            ("lo", "lo"),
            // 紧缩韵母 iu/ui/un 直接命中
            ("liu", "lq"),
            ("gui", "gv"),
            ("lun", "ly"),
            ("jiong", "js"),
            // ü 族的 v 拼写直接命中 v/ve 韵
            ("lv", "lv"),
            ("lve", "lt"),
            ("nv", "nv"),
            ("nve", "nt"),
        ] {
            assert_eq!(encode(input).to_string(), expected, "{input}");
        }
    }

    #[test]
    fn y_w_spellings_use_y_w_initials_without_rewrite() {
        for (input, expected) in [
            ("yi", "yi"),
            ("you", "yz"),
            ("ying", "yk"),
            ("yong", "ys"),
            ("wu", "wu"),
            ("wei", "ww"),
            ("weng", "wg"),
        ] {
            assert_eq!(encode(input).to_string(), expected, "{input}");
        }
    }

    #[test]
    fn jqx_yu_spellings_use_plain_u_final() {
        // 当前规范行为的语义边界:ju/qu/xu/yu 走 u 韵(u 键),
        // 与仓库词典一致(据=ju、去=qu、需=xu、与=yu);不做 ü 改写
        for (input, expected) in [
            ("ju", "ju"),
            ("jue", "jt"),
            ("juan", "jr"),
            ("jun", "jy"),
            ("qu", "qu"),
            ("que", "qt"),
            ("quan", "qr"),
            ("qun", "qy"),
            ("xu", "xu"),
            ("xue", "xt"),
            ("xuan", "xr"),
            ("xun", "xy"),
            ("yu", "yu"),
            ("yue", "yt"),
            ("yuan", "yr"),
            ("yun", "yy"),
        ] {
            assert_eq!(encode(input).to_string(), expected, "{input}");
        }
    }

    #[test]
    fn encoding_matches_structural_composition() {
        // 非零声母音节的编码必须等同于对分解结果的规范结构组合,
        // 即本模块不持有任何独立键位表
        let layout = DoublePinyinLayout::canonical();
        for spelling in ["zhang", "chua", "shui", "lv", "nve", "jiong"] {
            let (initial, final_) = decompose(spelling).unwrap();
            assert_eq!(
                layout.compose(initial, final_).unwrap(),
                encode(spelling),
                "{spelling}"
            );
        }
    }

    #[test]
    fn decompose_rejects_undecomposable_strings() {
        // 私有分解助手对无规范分解的拼写返回 None(公开 API 不可能拿到此类音节)
        assert_eq!(decompose("zzz"), None);
        assert_eq!(decompose("bx"), None);
    }
}
