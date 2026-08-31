//! 规范输入音节 → 双拼编码的全量回归测试。
//!
//! 核心不变量:全部 406 个规范 `XhupInputSyllable` 都必须可编码,
//! 且编码确定(同一音节重复编码结果一致)。

use std::collections::BTreeSet;

use xhup_core::XhupInputSyllable;

#[test]
fn every_canonical_syllable_encodes_deterministically() {
    let all = XhupInputSyllable::all();
    assert_eq!(all.len(), 406);
    for &syllable in all {
        let code = syllable.to_double_pinyin_code();
        assert_eq!(code.as_slice().len(), 2, "{syllable}");
        assert_eq!(
            syllable.to_double_pinyin_code(),
            code,
            "编码应确定: {syllable}"
        );
    }
}

#[test]
fn public_api_regression_sentinels() {
    // 与仓库词典当前行为一致的代表性编码(零声母、特殊声母、共享韵母键、
    // 紧缩韵母、ü 族 v 拼写、y/w、j/q/x/y + u 走 u 韵)
    for (input, expected) in [
        ("a", "aa"),
        ("ang", "ah"),
        ("er", "er"),
        ("ba", "ba"),
        ("zhang", "vh"),
        ("chua", "ix"),
        ("shui", "uv"),
        ("shei", "uw"),
        ("liu", "lq"),
        ("lv", "lv"),
        ("lve", "lt"),
        ("nve", "nt"),
        ("yi", "yi"),
        ("weng", "wg"),
        ("ju", "ju"),
        ("jue", "jt"),
        ("quan", "qr"),
        ("xue", "xt"),
        ("yun", "yy"),
    ] {
        let code = input
            .parse::<XhupInputSyllable>()
            .unwrap()
            .to_double_pinyin_code();
        assert_eq!(code.to_string(), expected, "{input}");
    }
}

#[test]
fn current_canonical_data_has_single_known_collision() {
    // 当前规范数据的回归事实(非公开契约):406 个音节产生 405 个不同编码,
    // 唯一碰撞为 lo/luo 同码(o 与 uo 共享 o 键)。双拼允许合法碰撞;
    // 未来清单变动可合法改变碰撞统计。
    let lo: XhupInputSyllable = "lo".parse().unwrap();
    let luo: XhupInputSyllable = "luo".parse().unwrap();
    assert_eq!(lo.to_double_pinyin_code().to_string(), "lo");
    assert_eq!(lo.to_double_pinyin_code(), luo.to_double_pinyin_code());

    let distinct: BTreeSet<_> = XhupInputSyllable::all()
        .iter()
        .map(|syllable| syllable.to_double_pinyin_code())
        .collect();
    assert_eq!(distinct.len(), 405, "当前规范数据的碰撞统计");
}
