//! #151 变长自动分段回归:真实生产码事实上的 2/3/4 键组合矩阵、
//! `jbzqu → jbz + qu → 进去` 哨兵、逐键 prefix-walk 增量不变量、
//! #150 未完成尾缀不变量。
//!
//! 事实来源:生成器 canonical 数据(与 `xhup_flow_flow` 组句词典同一
//! 投影),不用发明映射。层位于 `xhup-analyzer`(生产 lattice 桥)+
//! `xhup-decoder`(beam 解码),验证的是与 librime 组句词典同源的事实
//! 投影在 lattice 空间内的分段/排序语义。
//!
//! 断言策略:长输入的完整路径空间组合爆炸(8 键即可达数千),穷举
//! 枚举不保证期望路径在前 N 条内。因此:
//! - **边界表示**用边集合直接断言(span 存在 + 文本正确);
//! - **完整路径存在**用从左边界起点出发的定向 DFS 断言(有界);
//! - **排序可达**用 `decode_beam` 输出断言(top_k 内或非空+确定)。

use std::num::NonZeroUsize;

use xhup_analyzer::production_lattice::build_production_lattice;
use xhup_decoder::{BaselineScorer, DecodeConfig, RuntimeContext};

const LIMIT: NonZeroUsize = NonZeroUsize::new(256).unwrap();

/// 定向有界 DFS:从 `from` 出发是否存在到达输入末尾、且经过
/// `(left_span, left_text)` 的路径(利用 lattice 出边,与
/// `complete_paths` 同一语义,但锚定到指定起点,避免从头穷举)。
fn path_exists_through(
    built: &xhup_decoder::BuiltLattice,
    left_start: usize,
    left_end: usize,
    left_text: &str,
) -> bool {
    let lattice = built.lattice();
    let n = lattice.input().len();
    // 左边界边必须存在。
    let left_edge = lattice
        .outgoing(left_start)
        .unwrap_or(&[])
        .iter()
        .any(|&edge_id| {
            let edge = lattice.edge(edge_id).unwrap();
            edge.span().start() == left_start
                && edge.span().end() == left_end
                && edge.candidate().text() == left_text
        });
    if !left_edge {
        return false;
    }
    // 从 left_end 有界 DFS 到末尾。
    let mut stack = vec![left_end];
    let mut visited = std::collections::BTreeSet::new();
    while let Some(pos) = stack.pop() {
        if pos == n {
            return true;
        }
        if !visited.insert(pos) {
            continue;
        }
        for &edge_id in lattice.outgoing(pos).unwrap_or(&[]) {
            let edge = lattice.edge(edge_id).unwrap();
            stack.push(edge.span().end());
        }
    }
    false
}

fn decode(input: &str) -> Vec<(String, String)> {
    let built = build_production_lattice(input, LIMIT);
    let ctx = RuntimeContext::new("", input.parse().unwrap());
    let result = xhup_decoder::decode_beam(
        built.lattice(),
        &ctx,
        &BaselineScorer::default(),
        DecodeConfig::default(),
    );
    result
        .ranked()
        .iter()
        .map(|r| {
            let mut text = String::new();
            for edge_id in r.path().edge_ids() {
                text.push_str(built.lattice().edge(*edge_id).unwrap().candidate().text());
            }
            (text, String::new())
        })
        .collect()
}

fn full_coverage_paths(input: &str) -> Vec<(String, String)> {
    let built = build_production_lattice(input, LIMIT);
    built
        .paths()
        .paths()
        .iter()
        .map(|path| {
            let mut text = String::new();
            let mut segs = Vec::new();
            for edge_id in path.edge_ids() {
                let edge = built.lattice().edge(*edge_id).unwrap();
                text.push_str(edge.candidate().text());
                segs.push(format!(
                    "[{},{}):{}",
                    edge.span().start(),
                    edge.span().end(),
                    edge.candidate().text()
                ));
            }
            (text, segs.join("+"))
        })
        .collect()
}

/// 真实生产码事实上的 2/3/4 键相邻组合矩阵。
/// 每行 `raw = left_code + right_code`,左/右码与期望文本全部来自
/// canonical 数据(声码 + 全码投影),断言:
/// 1. 拼接输入可分段(存在完整覆盖路径,有界定向 DFS);
/// 2. 期望文本可达(同上,经由期望边界);
/// 3. 期望边界被表示(左/右边界边都在 lattice 中);
/// 4. 无需分隔符(整个 raw 是一次查询);
/// 5. 排序输出非空且确定;
/// 6. 排序输出(菜单表面)无重复语义候选。
macro_rules! seg_matrix_case {
    ($fn_name:ident, $raw:expr, $left:expr, $left_code:expr, $right:expr, $right_code:expr) => {
        #[test]
        fn $fn_name() {
            let raw = $raw;
            let left = $left;
            let right = $right;
            let left_len = $left_code.len();
            let built = build_production_lattice(raw, LIMIT);
            let lattice = built.lattice();

            // 3: 期望边界被表示 —— 左边界边与右边界边都存在。
            let left_edge = lattice.outgoing(0).unwrap().iter().any(|&edge_id| {
                let edge = lattice.edge(edge_id).unwrap();
                edge.span().start() == 0
                    && edge.span().end() == left_len
                    && edge.candidate().text() == left
            });
            assert!(left_edge, "{raw} 缺少左边界边 [0,{left_len}):{left}");
            let right_edge = lattice
                .outgoing(left_len)
                .unwrap_or(&[])
                .iter()
                .any(|&edge_id| {
                    let edge = lattice.edge(edge_id).unwrap();
                    edge.span().start() == left_len
                        && edge.span().end() == raw.len()
                        && edge.candidate().text() == right
                });
            assert!(
                right_edge,
                "{raw} 缺少右边界边 [{left_len},{}):{right}",
                raw.len()
            );

            // 1+2: 存在经由期望边界的完整路径(期望文本可达)。
            assert!(
                path_exists_through(&built, 0, left_len, left),
                "{raw} 必须存在经由 {left} 的完整路径"
            );

            // 5+6: 排序输出非空、确定、无重复语义候选。
            let ranked_once = decode(raw);
            assert!(!ranked_once.is_empty(), "{raw} 排序输出不得为空");
            assert_eq!(decode(raw), ranked_once, "{raw} 排序必须确定");
            let mut texts: Vec<_> = ranked_once.iter().map(|(t, _)| t.clone()).collect();
            texts.sort();
            texts.dedup();
            assert_eq!(
                texts.len(),
                ranked_once.len(),
                "{raw} 菜单不得有重复语义候选"
            );
        }
    };
}

// 2+2: 啊(aa) + 吧(ba) → 啊吧
seg_matrix_case!(matrix_2_2, "aaba", "啊", "aa", "吧", "ba");
// 2+3: 啊(aa) + 爸(bab) → 啊爸
seg_matrix_case!(matrix_2_3, "aabab", "啊", "aa", "爸", "bab");
// 3+2: 啊(aak) + 吧(ba) → 啊吧
seg_matrix_case!(matrix_3_2, "aakba", "啊", "aak", "吧", "ba");
// 3+3: 啊(aak) + 爸(bab) → 啊爸
seg_matrix_case!(matrix_3_3, "aakbab", "啊", "aak", "爸", "bab");
// 2+4: 啊(aa) + 爸(babb) → 啊爸
seg_matrix_case!(matrix_2_4, "aababb", "啊", "aa", "爸", "babb");
// 4+2: 啊(aakd) + 吧(ba) → 啊吧
seg_matrix_case!(matrix_4_2, "aakdba", "啊", "aakd", "吧", "ba");
// 3+4: 啊(aak) + 爸(babb) → 啊爸
seg_matrix_case!(matrix_3_4, "aakbabb", "啊", "aak", "爸", "babb");
// 4+3: 啊(aakd) + 爸(bab) → 啊爸
seg_matrix_case!(matrix_4_3, "aakdbab", "啊", "aakd", "爸", "bab");
// 4+4: 啊(aakd) + 爸(babb) → 啊爸
seg_matrix_case!(matrix_4_4, "aakdbabb", "啊", "aakd", "爸", "babb");

/// #151 精确回归哨兵:`jbzqu → jbz + qu → 进去`。
///
/// 事实:进 = jb(2键声码)/ jbz(3键全码);去 = qu(2键)。jbzqu 共
/// 5 键,唯一完整覆盖分段 = jbz + qu(2+3 切分 jb|zqu 中 zqu 非码,
/// jb+qu 只覆盖 4 键,均不构成完整路径)。全部来自 canonical 数据,
/// 无任何硬编码例外。
#[test]
fn sentinel_jbzqu_segments_as_jbz_plus_qu() {
    let built = build_production_lattice("jbzqu", LIMIT);
    let lattice = built.lattice();

    // 1. jbz+qu 边界必须被表示。
    assert!(
        path_exists_through(&built, 0, 3, "进"),
        "jbz+qu 边界必须被表示(经 jbz 的完整路径必须存在)"
    );
    // 2. 进去可达。
    let ranked = decode("jbzqu");
    assert!(
        ranked.iter().any(|(text, _)| text == "进去"),
        "jbzqu 必须可达 进去,实际: {:?}",
        &ranked[..ranked.len().min(5)]
    );
    // 3. 排序层:进去 rank-1(生产频率事实下该路径无歧义胜出)。
    assert_eq!(
        ranked.first().map(|(text, _)| text.as_str()),
        Some("进去"),
        "jbzqu 在生产排序下应 rank-1 进去,实际: {:?}",
        &ranked[..ranked.len().min(3)]
    );
    // 4. 排序输出(菜单表面)无重复语义候选。
    let mut texts: Vec<_> = ranked.iter().map(|(t, _)| t.clone()).collect();
    texts.sort();
    texts.dedup();
    assert_eq!(texts.len(), ranked.len());
    // 5. 确定性。
    assert_eq!(decode("jbzqu"), ranked);
    let _ = lattice;
}

/// #151 逐键 prefix-walk + #150 不变量:每个前缀(1..=全码)都必须
/// **可构建且确定**;存在完整覆盖路径的前缀(如 jbz、jbzqu)必须保持
/// 可用候选。中间态前缀(jbzq:尾码 q 未完成)在纯 lattice 空间内
/// 合法地为空 —— 菜单保持非空由运行时 completion 候选兜底
/// (schema `enable_completion: true`,librime Flow 审计覆盖),两者
/// 互补构成 #150 的完整语义。
#[test]
fn prefix_walk_jbzqu_stays_usable_at_every_step() {
    let raw = "jbzqu";
    for end in 1..=raw.len() {
        let prefix = &raw[..end];
        let paths = full_coverage_paths(prefix);
        let again = full_coverage_paths(prefix);
        assert_eq!(paths, again, "前缀 {prefix} 分段必须确定");
        if prefix == "jbz" || prefix == "jbzqu" {
            // 有完整覆盖的前缀:候选必须可用。
            assert!(
                !paths.is_empty(),
                "前缀 {prefix} 必须保持可用候选(菜单不得塌缩)"
            );
        }
        if prefix == "jbz" {
            assert!(paths.iter().any(|(text, _)| text == "进"));
        }
        if prefix == "jbzqu" {
            assert!(paths.iter().any(|(text, _)| text == "进去"));
        }
    }
}

/// #151 歧义分段:同一 raw 存在多条合法完整分段时必须**全部保留**
/// 供排序层选择,不得被贪心规则吞并。
///
/// 真实事实:`aajk`(4 键)同时有
/// - `[0,4):锕`(4 键全码,aajk)
/// - `[0,2):啊`(aa)+ `[2,4):经`(jk) —— 2+2 组合
///
/// 两条完整路径覆盖同一按键流,边界不同、文本不同,均合法。
#[test]
fn ambiguous_segmentation_preserves_alternatives() {
    let built = build_production_lattice("aajk", LIMIT);
    let lattice = built.lattice();

    // 边界 1:单 4 键码 锕@aajk。
    assert!(
        lattice.outgoing(0).unwrap().iter().any(|&edge_id| {
            let edge = lattice.edge(edge_id).unwrap();
            edge.span().start() == 0 && edge.span().end() == 4 && edge.candidate().text() == "锕"
        }),
        "歧义边界 1([0,4):锕)必须保留"
    );
    // 边界 2:2+2 啊(aa)+ 经(jk)。
    let two_two = lattice.outgoing(0).unwrap().iter().any(|&edge_id| {
        let edge = lattice.edge(edge_id).unwrap();
        edge.span().start() == 0 && edge.span().end() == 2 && edge.candidate().text() == "啊"
    }) && lattice.outgoing(2).unwrap_or(&[]).iter().any(|&edge_id| {
        let edge = lattice.edge(edge_id).unwrap();
        edge.span().start() == 2 && edge.span().end() == 4 && edge.candidate().text() == "经"
    });
    assert!(two_two, "歧义边界 2(啊+经,2+2)必须保留");

    // 两条分段路径都必须可达(有界定向 DFS,分别锚定两条边界)。
    assert!(
        path_exists_through(&built, 0, 4, "锕"),
        "锕([0,4) 单 4 键码)路径必须可达"
    );
    assert!(
        path_exists_through(&built, 0, 2, "啊") && {
            // 啊 + 经:经的边已断言存在,从位置 2 到末尾必有完整路径
            // (经边本身即覆盖 [2,4) 到达末尾)。
            true
        },
        "啊经(2+2)路径必须可达"
    );
    let paths = full_coverage_paths("aajk");
    assert!(
        paths.iter().any(|(text, _)| text == "啊经"),
        "啊经 路径必须可达"
    );

    // 确定性:歧义保留本身也要可复现。
    assert_eq!(paths, full_coverage_paths("aajk"));
}

/// #150 不变量:已解码前缀 + 未完成尾键不得清空候选。
/// 用真实词典词条码构造:我觉(wojt)+ 时间(uijm)→ 我觉时间
/// (两词均为组句词典真实条目)。中间态前缀可构建、确定;
/// 尾码不完整的状态(wojt + ui)下,wojt 覆盖的「我觉」路径保留。
#[test]
fn incomplete_trailing_prefix_preserves_decodable_prefix() {
    let full = "wojtuijm";
    // 完整输入:我觉时间可达(经 wojt + uijm 边界)。
    let built_full = build_production_lattice(full, LIMIT);
    assert!(
        path_exists_through(&built_full, 0, 4, "我觉"),
        "wojtuijm 必须存在经 我觉(wojt) 的完整路径"
    );

    // 逐前缀:可构建、确定、无 panic(含单键 w 这种零事实前缀)。
    for end in 1..=full.len() {
        let prefix = &full[..end];
        let paths = full_coverage_paths(prefix);
        let again = full_coverage_paths(prefix);
        assert_eq!(paths, again, "前缀 {prefix} 分段必须确定");
    }

    // 未完成尾键状态 wojt + ui:wojt 边存在 → 「我觉」前缀路径保留。
    let mid = build_production_lattice("wojtui", LIMIT);
    let wojt_edge = mid.lattice().outgoing(0).unwrap().iter().any(|&edge_id| {
        let edge = mid.lattice().edge(edge_id).unwrap();
        edge.span().start() == 0 && edge.span().end() == 4 && edge.candidate().text() == "我觉"
    });
    assert!(wojt_edge, "尾缀 ui 未完成时,wojt 段(我觉)边界边必须保留");
}
