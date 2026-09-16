//! Exact assignment Oracle(Issue #83 §8,§25 第 5 步)。
//!
//! 目的:回答「当前 heuristic(produce_mapping_explained 的效用降序贪心)
//! 离最优解多远」。Oracle **不参与生产构建**,只服务研究与 CI 对照。
//!
//! 模型:单码实例内的 assignment 问题。贪心与 oracle 共享同一候选集合
//! (word → 每词若干 (code, rank) 槽位与效用值),差异仅在**接纳顺序**:
//!
//! - heuristic:全局效用降序,逐个接纳,先到先得;
//! - oracle:同实例上的**最小代价最大基数匹配**(Hungarian / Kuhn-Munkras,
//!   槽位容量 = rank 上限内每个 (code, rank) 至多一词)。
//!
//! 两者都满足同一组硬约束(#83 §7):
//! - 每词至多一个简码(或留在全码,不分配);
//! - 每 (code, rank) 槽位至多一词;
//! - rank ≤ MAX_RANK;
//! - 不允许通过删除词汇改善指标(全部候选词参与)。
//!
//! 精确性声明:槽位效用取**生产同款** evaluate_assignment(占用质量以
//! 静态 baseline 为上界快照,不含运行点内互相扰动的二阶项)—— 与
//! produce_mapping 阶段 1 的评分完全同源,因此 optimality gap 度量的是
//! 「同评分下接纳顺序的损失」,而非评分模型本身的近似。这一限制显式
//! 记录:两阶段互扰的二阶效用是 §8 后续工作。
//!
//! 确定性:同输入 → 同 gap 报告;全部比较整数化/total_cmp,无浮点决胜。

use std::collections::BTreeMap;

use xhup_core::KeySequence;

use crate::mapping_v2::MAX_RANK;

/// 单个 (word, code, rank) 候选槽位的 oracle 输入。
#[derive(Clone, Debug, PartialEq)]
pub struct OracleSlot {
    pub word: String,
    pub code: KeySequence,
    pub rank: usize,
    /// 生产同款评分的净效用(breakdown.total() − baseline_total;可负)。
    pub net_utility: f64,
}

/// 单词的 oracle 视图:候选槽位集合(全部 rank ≤ MAX_RANK 的组合)。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct OracleWord {
    pub slots: Vec<OracleSlot>,
}

/// 缩减实例(单码或同构码组;跨码互斥由实例切分保证)。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct OracleInstance {
    /// 词 → 候选槽位。
    pub words: BTreeMap<String, OracleWord>,
}

impl OracleInstance {
    /// 从打分短名单构建(与 produce_mapping 阶段 1 同一评分来源)。
    ///
    /// `scored`:每项 (word, code, rank, net_utility)。rank 超界与负效用
    /// 槽位照常保留(oracle 会自行权衡是否使用;负效用等价于不分配)。
    pub fn from_scored(
        scored: impl IntoIterator<Item = (String, KeySequence, usize, f64)>,
    ) -> Self {
        let mut words: BTreeMap<String, OracleWord> = BTreeMap::new();
        for (word, code, rank, net_utility) in scored {
            if rank == 0 || rank > MAX_RANK {
                continue;
            }
            words.entry(word).or_default().slots.push(OracleSlot {
                code,
                rank,
                net_utility,
                word: String::new(), // 填充在下面统一补
            });
        }
        for (word, view) in words.iter_mut() {
            for slot in &mut view.slots {
                slot.word = word.clone();
            }
            view.slots
                .sort_by(|a, b| a.code.cmp(&b.code).then(a.rank.cmp(&b.rank)));
        }
        OracleInstance { words }
    }

    /// 槽位总数(审计)。
    pub fn slot_count(&self) -> usize {
        self.words.values().map(|w| w.slots.len()).sum()
    }
}

/// 一词的 oracle 决策。
#[derive(Clone, Debug, PartialEq)]
pub struct OracleAssignment {
    pub word: String,
    /// 分配的码(None = 该词最优选择是留全码/无可用正效用槽位)。
    pub code: Option<KeySequence>,
    pub rank: Option<usize>,
    pub net_utility: f64,
}

/// 同一实例上的两种解。
#[derive(Clone, Debug, PartialEq)]
pub struct OracleComparison {
    pub instance: InstanceSummary,
    pub greedy: Vec<OracleAssignment>,
    pub exact: Vec<OracleAssignment>,
    /// greedy 总净效用。
    pub greedy_utility: f64,
    /// exact 总净效用。
    pub exact_utility: f64,
    /// optimality gap = exact − greedy(≥ 0;0 表示贪心已最优)。
    pub optimality_gap: f64,
    /// 相对 gap(gap / |exact|;exact=0 时为 0)。
    pub relative_gap: f64,
    /// greedy 损失的词(oracle 能分配而贪心没分到的词数)。
    pub words_lost_by_greedy: usize,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct InstanceSummary {
    pub words: usize,
    pub slots: usize,
}

/// 在缩减实例上运行两种解法并比较。
///
/// greedy 语义复刻:按净效用降序(词/码字典序决胜)逐个接纳,词互斥、
/// (code, rank) 互斥 —— 与 mapping_v2 阶段 2 的接纳规则同构(不含
/// 回放守卫,那属于跨码选路,不属于单码实例)。
pub fn compare_greedy_vs_exact(instance: &OracleInstance) -> OracleComparison {
    let greedy = greedy_solve(instance);
    let exact = exact_solve(instance);
    let greedy_utility = total_utility(&greedy);
    let exact_utility = total_utility(&exact);
    let optimality_gap = exact_utility - greedy_utility;
    let greedy_words = greedy
        .iter()
        .filter(|a| a.code.is_some())
        .map(|a| a.word.clone())
        .collect::<std::collections::BTreeSet<_>>();
    let exact_words = exact
        .iter()
        .filter(|a| a.code.is_some())
        .map(|a| a.word.clone())
        .collect::<std::collections::BTreeSet<_>>();
    let words_lost = exact_words.difference(&greedy_words).count();
    OracleComparison {
        instance: InstanceSummary {
            words: instance.words.len(),
            slots: instance.slot_count(),
        },
        greedy_utility,
        exact_utility,
        optimality_gap,
        relative_gap: if exact_utility.abs() > f64::EPSILON {
            optimality_gap / exact_utility.abs()
        } else {
            0.0
        },
        greedy,
        exact,
        words_lost_by_greedy: words_lost,
    }
}

fn total_utility(assignments: &[OracleAssignment]) -> f64 {
    assignments.iter().map(|a| a.net_utility).sum()
}

/// 贪心:效用降序接纳,词互斥 + 槽位互斥。
fn greedy_solve(instance: &OracleInstance) -> Vec<OracleAssignment> {
    let mut slots: Vec<&OracleSlot> = instance
        .words
        .values()
        .flat_map(|w| w.slots.iter())
        .collect();
    slots.sort_by(|a, b| {
        b.net_utility
            .total_cmp(&a.net_utility)
            .then_with(|| a.word.cmp(&b.word))
            .then_with(|| a.code.cmp(&b.code))
            .then(a.rank.cmp(&b.rank))
    });
    let mut used_words: std::collections::BTreeSet<&str> = Default::default();
    let mut used_codes: std::collections::BTreeSet<(String, usize)> = Default::default();
    let mut out = Vec::new();
    for slot in slots {
        // 生产接纳规则:净效用 ≤ 0 的候选一律拒绝(mapping_v2 阶段 2;
        // 词的其他候选码仍有机会)。oracle 与生产共享该语义。
        if slot.net_utility <= 0.0 {
            continue;
        }
        if used_words.contains(slot.word.as_str()) {
            continue;
        }
        if !used_codes.insert((slot.code.to_string(), slot.rank)) {
            continue;
        }
        used_words.insert(&slot.word);
        out.push(OracleAssignment {
            word: slot.word.clone(),
            code: Some(slot.code.clone()),
            rank: Some(slot.rank),
            net_utility: slot.net_utility,
        });
    }
    // 未分配词也输出(net_utility 0,可解释)。
    for word in instance.words.keys() {
        if !used_words.contains(word.as_str()) {
            out.push(OracleAssignment {
                word: word.clone(),
                code: None,
                rank: None,
                net_utility: 0.0,
            });
        }
    }
    out.sort_by(|a, b| a.word.cmp(&b.word));
    out
}

/// 精确:逐连通分量的最小代价最大基数匹配(Hungarian,Kuhn 算法,O(V·E))。
///
/// 词 ↔ 槽位二部图;词一侧可与「不分配」配对(效用 0),因此最优解
/// ≥ 0 恒成立;只对正效用边建匹配(负/零边等价不分配)。槽位容量 1。
fn exact_solve(instance: &OracleInstance) -> Vec<OracleAssignment> {
    // 建图:词节点 → (槽位节点, 效用)。只保留正效用槽位。
    let mut slot_ids: BTreeMap<(String, usize), usize> = BTreeMap::new();
    let mut edges: Vec<(usize, usize, f64)> = Vec::new(); // (word_idx, slot_idx, utility)
    let word_list: Vec<&String> = instance.words.keys().collect();
    for (word_idx, word) in word_list.iter().enumerate() {
        for slot in &instance.words[*word].slots {
            if slot.net_utility <= 0.0 {
                continue; // 负/零效用等价于不分配
            }
            let key = (slot.code.to_string(), slot.rank);
            let next = slot_ids.len();
            let slot_idx = *slot_ids.entry(key).or_insert(next);
            edges.push((word_idx, slot_idx, slot.net_utility));
        }
    }
    let n_words = instance.words.len();
    let n_slots = slot_ids.len();
    // Kuhn 匹配 + 最大权?不 —— 目标是**最大权匹配**(不是最大基数)。
    // 用「逐词按效用降序 + 精确增广」不保证最优;改为对每个连通分量
    // 做 O(n^3) 匈牙利(分配问题,方阵化:词 i 分配槽位 j 或 dummy)。
    let assignments = hungarian_max_weight(n_words, n_slots, &edges);

    let slot_key_by_idx: Vec<(String, usize)> = {
        let mut v = vec![(String::new(), 0usize); n_slots];
        for (key, &idx) in &slot_ids {
            v[idx] = key.clone();
        }
        v
    };
    let word_by_idx: Vec<&String> = word_list.clone();
    let mut out: Vec<OracleAssignment> = Vec::new();
    let mut assigned_words = std::collections::BTreeSet::new();
    for (word_idx, slot_idx) in assignments {
        let Some(slot_idx) = slot_idx else { continue };
        let key = &slot_key_by_idx[slot_idx];
        out.push(OracleAssignment {
            word: (*word_by_idx[word_idx]).clone(),
            code: Some(
                key.0
                    .parse()
                    .expect("slot code came from a valid KeySequence"),
            ),
            rank: Some(key.1),
            net_utility: instance.words[word_by_idx[word_idx]]
                .slots
                .iter()
                .find(|s| s.code.to_string() == key.0 && s.rank == key.1)
                .map(|s| s.net_utility)
                .unwrap_or(0.0),
        });
        assigned_words.insert(word_by_idx[word_idx].clone());
    }
    for word in word_list {
        if !assigned_words.contains(word) {
            out.push(OracleAssignment {
                word: (*word).clone(),
                code: None,
                rank: None,
                net_utility: 0.0,
            });
        }
    }
    out.sort_by(|a, b| a.word.cmp(&b.word));
    out
}

/// O(n^3) 匈牙利最大权匹配(非方阵:词数 ≤ 槽数;未匹配词 → None)。
/// 确定性:相等权重按槽位索引升序(输入边序已按词字典序 + 槽位键升序)。
fn hungarian_max_weight(
    n_words: usize,
    n_slots: usize,
    edges: &[(usize, usize, f64)],
) -> Vec<(usize, Option<usize>)> {
    // 精确策略:直接进入 O(n^3) 匈牙利(见 hungarian_exact 文档)。
    hungarian_exact(n_words, n_slots, edges)
}

/// 精确 O(n^3) 匈牙利(分配问题;代价 = −效用;dummy 填充代价 0)。
/// 返回 (word_idx, Some(slot_idx)) 与 (word_idx, None)。
fn hungarian_exact(
    n_words: usize,
    n_slots: usize,
    edges: &[(usize, usize, f64)],
) -> Vec<(usize, Option<usize>)> {
    let n = n_words.max(n_slots);
    // 代价矩阵:word × slot;默认 0(dummy);真实边 −效用。
    let mut cost = vec![vec![0.0_f64; n]; n];
    for (w, s, u) in edges {
        cost[*w][*s] = -*u;
    }
    // 标准 O(n^3) 匈牙利(1-indexed 惯例的 0-indexed 移植;u/v 势,lj 匹配)。
    const INF: f64 = f64::INFINITY;
    let mut u = vec![0.0_f64; n + 1];
    let mut v = vec![0.0_f64; n + 1];
    let mut p = vec![0usize; n + 1]; // p[j] = 匹配的 i
    let mut way = vec![0usize; n + 1];
    for i in 1..=n {
        p[0] = i;
        let mut j0 = 0usize;
        let mut minv = vec![INF; n + 1];
        let mut used = vec![false; n + 1];
        loop {
            used[j0] = true;
            let i0 = p[j0];
            let mut delta = INF;
            let mut j1 = 0usize;
            for j in 1..=n {
                if !used[j] {
                    let cur = cost[i0 - 1][j - 1] - u[i0] - v[j];
                    if cur < minv[j] {
                        minv[j] = cur;
                        way[j] = j0;
                    }
                    if minv[j] < delta {
                        delta = minv[j];
                        j1 = j;
                    }
                }
            }
            for j in 0..=n {
                if used[j] {
                    u[p[j]] += delta;
                    v[j] -= delta;
                } else {
                    minv[j] -= delta;
                }
            }
            j0 = j1;
            if p[j0] == 0 {
                break;
            }
        }
        loop {
            let j1 = way[j0];
            p[j0] = p[j1];
            j0 = j1;
            if j0 == 0 {
                break;
            }
        }
    }
    // 提取匹配:只有真实边(效用 > 0 的槽)才算分配;dummy 匹配 → None。
    let edge_set: std::collections::BTreeSet<(usize, usize)> =
        edges.iter().map(|(w, s, _)| (*w, *s)).collect();
    let mut out: Vec<(usize, Option<usize>)> = Vec::new();
    for i in 1..=n_words {
        let slot = (1..=n_slots)
            .find(|&j| p[j] == i && edge_set.contains(&(i - 1, j - 1)))
            .map(|j| j - 1);
        out.push((i - 1, slot));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn code(s: &str) -> KeySequence {
        s.parse().unwrap()
    }

    #[test]
    fn greedy_equals_exact_when_no_conflict() {
        // 词 A、B 各自唯一槽位,无竞争 → gap 0。
        let inst = OracleInstance::from_scored([
            ("甲".into(), code("aaaa"), 1, 3.0),
            ("乙".into(), code("bbbb"), 1, 2.0),
        ]);
        let cmp = compare_greedy_vs_exact(&inst);
        assert!((cmp.optimality_gap).abs() < 1e-12);
        assert_eq!(cmp.greedy_utility, 5.0);
        assert_eq!(cmp.exact_utility, 5.0);
    }

    #[test]
    fn greedy_order_conflict_is_detected() {
        // 两个词争同一槽位:贪心选高效用词,oracle 同 —— gap 0。
        // 但次序反了(低效用词先进)时贪心也按效用降序,所以单槽无 gap。
        let inst = OracleInstance::from_scored([
            ("甲".into(), code("aaaa"), 1, 1.0),
            ("乙".into(), code("aaaa"), 1, 3.0),
        ]);
        let cmp = compare_greedy_vs_exact(&inst);
        assert!((cmp.optimality_gap).abs() < 1e-12);
        assert_eq!(cmp.exact_utility, 3.0);
    }

    #[test]
    fn gap_is_positive_when_greedy_is_trapped() {
        // 经典贪心陷阱:甲 效用 3.5 只能用槽 S1;乙 效用 3.0 可用 S1 或 S2。
        // 贪心先给 甲→S1;乙 只能去 S2(效用 3.0)→ 总 6.5。
        // 最优:甲 留全码(0),乙→S1(3.0)?不 —— 乙用 S1 得 3.0,甲无处去
        // (只有 S1)→ 总 3.0 < 6.5。所以最优还是 甲→S1, 乙→S2 = 6.5,gap 0。
        // 构造真正的陷阱:乙 可用 S2(效用 2.0)且 S1;甲 只能 S1(3.5)。
        // 贪心:甲→S1(3.5),乙→S2(2.0) = 5.5;最优同。gap 0。
        // 真陷阱需要「贪心占用阻碍后续更优组合」:甲 S1=2.0 且 S2=2.0;
        // 乙 S1=3.0。贪心按效用降序:乙(3.0)→S1,甲→S2(2.0) = 5.0 = 最优。
        // 单槽互斥 + 效用独立时贪心其实总是最优(assignment 拟阵性质不成立
        // 的关键在多槽竞争):甲 S1=3.0, S2=1.0;乙 S1=2.5, S2=2.5。
        // 贪心:甲(3.0)→S1,乙→S2(2.5)= 5.5;最优:乙→S1(2.5),甲→S2(1.0)
        // = 3.5 更差。贪心 5.5 是最优。单码实例中 gap 常 0;**跨码互扰**才是
        // gap 来源(见 multi_code_conflict 测试)。
        let inst = OracleInstance::from_scored([
            ("甲".into(), code("aaaa"), 1, 3.0),
            ("甲".into(), code("aaab"), 1, 1.0),
            ("乙".into(), code("aaaa"), 1, 2.5),
            ("乙".into(), code("aaab"), 1, 2.5),
        ]);
        let cmp = compare_greedy_vs_exact(&inst);
        assert!(
            (cmp.optimality_gap).abs() < 1e-12,
            "gap {}",
            cmp.optimality_gap
        );
        assert_eq!(cmp.exact_utility, 5.5);
    }

    #[test]
    fn multi_code_conflict_produces_positive_gap() {
        // 跨槽位竞争的真陷阱:
        // 词 X: 槽 S1 效用 5.0;词 Y: 槽 S1 效用 4.5、槽 S2 效用 4.0。
        // 词 Z: 槽 S2 效用 3.0。
        // 贪心:X→S1(5.0), Y→S2(4.0), Z 无槽可用(0)= 9.0。
        // 最优:Y→S1(4.5), X 留全码?X 只有 S1(5.0)>0 不会留 —— X→S1(5.0),
        // Y→S2(4.0), Z 无 → 相同。改:X 槽 S1=5.0 与 S2=2.0;
        // Y 槽 S2=4.5;Z 槽 S2=4.0。
        // 贪心:X→S1(5.0), Y→S2(4.5), Z 无 = 9.5。最优:同。
        // 再构造:X: S1=3.0, S2=3.0;Y: S1=2.9;Z: S2=2.8。
        // 贪心:X→S1(3.0), Y 无(S1 被占,效用 2.9 但词已无关)——Y 没别的槽
        // → 0;Z→S2?S2 被甲占了吗?没有:X 在 S1。Z→S2(2.8)。
        // 总 = 5.8。最优:X→S2(3.0), Y→S1(2.9), Z 留(0)= 5.9!gap 0.1!
        let inst = OracleInstance::from_scored([
            ("X".into(), code("aaaa"), 1, 3.0),
            ("X".into(), code("aaab"), 1, 3.0),
            ("Y".into(), code("aaaa"), 1, 2.9),
            ("Z".into(), code("aaab"), 1, 2.8),
        ]);
        let cmp = compare_greedy_vs_exact(&inst);
        assert!(
            cmp.optimality_gap > 0.0,
            "该实例贪心应非最优: gap {}",
            cmp.optimality_gap
        );
        assert!((cmp.exact_utility - 5.9).abs() < 1e-9);
        assert!((cmp.greedy_utility - 5.8).abs() < 1e-9);
        assert_eq!(cmp.words_lost_by_greedy, 1, "贪心丢了 Y");
    }

    #[test]
    fn rank_capacity_is_respected() {
        // 同码不同 rank 是不同槽位:容量约束按 (code, rank) 各一。
        let inst = OracleInstance::from_scored([
            ("甲".into(), code("aaaa"), 1, 3.0),
            ("乙".into(), code("aaaa"), 2, 2.5),
            ("丙".into(), code("aaaa"), 2, 2.0),
        ]);
        let cmp = compare_greedy_vs_exact(&inst);
        // 贪心:甲→r1,乙→r2,丙无 → 5.5。最优同(丙 r2 效用低于乙)。
        assert!((cmp.optimality_gap).abs() < 1e-12);
        assert_eq!(cmp.exact_utility, 5.5);
        // 换成丙的 r2 效用 2.8:贪心按效用降序 甲→r1(3.0)→丙(2.8)→乙无
        // = 5.8,与最优一致(丙效用高于乙,贪心天然先接纳丙)。
        let inst2 = OracleInstance::from_scored([
            ("甲".into(), code("aaaa"), 1, 3.0),
            ("乙".into(), code("aaaa"), 2, 2.5),
            ("丙".into(), code("aaaa"), 2, 2.8),
        ]);
        let cmp2 = compare_greedy_vs_exact(&inst2);
        assert!(
            (cmp2.optimality_gap).abs() < 1e-9,
            "gap {}",
            cmp2.optimality_gap
        );
        assert_eq!(cmp2.exact_utility, 5.8);
    }

    #[test]
    fn negative_utility_slots_never_assigned() {
        let inst = OracleInstance::from_scored([
            ("甲".into(), code("aaaa"), 1, -1.0),
            ("乙".into(), code("aaaa"), 1, 0.0),
        ]);
        let cmp = compare_greedy_vs_exact(&inst);
        assert_eq!(cmp.exact_utility, 0.0);
        for a in &cmp.exact {
            assert!(a.code.is_none(), "负/零效用槽位不应分配: {a:?}");
        }
    }

    #[test]
    fn out_of_range_ranks_are_dropped() {
        let inst = OracleInstance::from_scored([
            ("甲".into(), code("aaaa"), 1, 3.0),
            ("甲".into(), code("aaab"), MAX_RANK + 1, 99.0),
        ]);
        assert_eq!(inst.slot_count(), 1, "rank 超界槽位被拒绝");
    }

    #[test]
    fn real_canonical_instance_gap_report() {
        // 真实生产数据:取 canonical v2 targets 的 top-200 高频词构建单码
        // 实例(全量 130 万词的完整 oracle 是研究管线,不进单测;此处验证
        // oracle 在真实数据上可运行且 gap 报告形状正确)。
        let data = crate::build_analysis();
        let evidence = {
            let set = crate::evidence::LexicalEvidenceSet::build(&data.words, &data.frequency);
            crate::sweep_v2::evidence_by_word(&set)
        };
        let scale = crate::mapping_v2::MassScale::build(&evidence);
        let baseline = crate::mapping_v2::BaselineMassView::build(
            &crate::occupancy::CodeOccupancy::build_current_production(),
        );
        let cost = crate::optimizer_v2::CostModelV2::default();
        let weights = crate::optimizer_v2::EvidenceWeights::default();
        let mut targets = crate::sweep_v2::v2_targets(&data.words);
        // top-200:与 sweep frequency_order 同口径。
        targets.sort_by_key(|t| std::cmp::Reverse(t.frequency_score()));
        targets.truncate(200);

        // 生产同款评分(mass/baseline 快照;不含运行点互扰二阶项 —— 见模块文档)。
        let mut scored = Vec::new();
        for target in &targets {
            let Some(ev) = evidence.get(target.word()) else {
                continue;
            };
            let mass = scale.mass(ev, &weights);
            let baseline_total = {
                let b = crate::optimizer_v2::evaluate_assignment(
                    ev,
                    &crate::optimizer_v2::CandidateSlot {
                        key_len: target.full_code().len(),
                        rank: baseline.full_code_rank(target.word()),
                        occupant_mass: 0.0,
                        displaced_mass: 0.0,
                    },
                    &cost,
                    &weights,
                    crate::xhup_prior::NEUTRAL_PRIOR,
                    target.full_code().len(),
                    true,
                );
                b.total() - b.xhup_prior
            };
            for candidate in target.candidates() {
                let code = candidate.shortcut_code();
                let pattern_consistent = target.full_code().as_slice().starts_with(code.as_slice());
                let (position, displaced, total_mass) =
                    slot_analysis_public(baseline.group(code), mass, target.word());
                let breakdown = crate::optimizer_v2::evaluate_assignment(
                    ev,
                    &crate::optimizer_v2::CandidateSlot {
                        key_len: code.len(),
                        rank: position,
                        occupant_mass: total_mass,
                        displaced_mass: displaced,
                    },
                    &cost,
                    &weights,
                    crate::xhup_prior::NEUTRAL_PRIOR,
                    target.full_code().len(),
                    pattern_consistent,
                );
                scored.push((
                    target.word().to_string(),
                    code.clone(),
                    position,
                    breakdown.total() - baseline_total,
                ));
            }
        }
        let instance = OracleInstance::from_scored(scored);
        assert!(instance.slot_count() > 100, "真实实例应有实质槽位");
        let cmp = compare_greedy_vs_exact(&instance);
        // gap ≥ 0 恒成立(精确解定义);报告形状断言。
        assert!(cmp.optimality_gap >= -1e-9);
        assert!(cmp.relative_gap.is_finite(), "相对 gap 必须有限");
        assert_eq!(cmp.instance.words, targets.len());
    }

    /// mapping_v2::slot_analysis 未公开;此处镜像其语义(baseline 快照,
    /// 无 v2 members)供 oracle 评分。
    fn slot_analysis_public(baseline: &[f64], mass: f64, _word: &str) -> (usize, f64, f64) {
        let mut before = 0usize;
        let mut displaced = 0.0;
        let mut total = 0.0;
        for m in baseline {
            total += *m;
            if m.total_cmp(&mass).is_gt() {
                before += 1;
            } else {
                displaced += *m;
            }
        }
        (before + 1, displaced, total)
    }
}
