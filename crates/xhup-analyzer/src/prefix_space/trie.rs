//! Prefix-Closed Trie: 前缀闭合变长码空间。
//!
//! 核心不变量:
//! - 前缀闭合: 路径中任一合法码序列的所有前缀节点必然存在且为合法查询状态。
//! - valid prefix != commit boundary: 有效前缀不隐含提交边界, 用户始终可以继续输入。
//! - 候选位与继续输入解耦: 即使 rank 1 存在, 只要子树有后续分支, 即可无缝继续按键。

use std::collections::BTreeMap;
use xhup_core::{Key, KeySequence};

use super::slot::SlotCandidate;

/// 前缀树节点。
#[derive(Clone, Debug)]
pub struct TrieNode {
    /// 节点的完整按键序列(根节点为 None)。
    pub code: Option<KeySequence>,
    /// 子节点索引映射: 按键 → Arena 节点索引。
    pub children: BTreeMap<Key, usize>,
    /// 父节点索引(根节点为 None)。
    pub parent: Option<usize>,
    /// 位于当前节点的有序候选槽位列表(索引 0 即 rank 1)。
    pub slots: Vec<SlotCandidate>,
    /// 当前节点自身所有候选的质量总和。
    pub direct_mass: f64,
    /// 当前节点及其全部子树候选的质量总和。
    pub subtree_mass: f64,
    /// 当前节点的直接候选数量。
    pub direct_count: usize,
    /// 当前节点及其子树的所有候选总数。
    pub subtree_count: usize,
    /// 经过当前节点向下延伸的合法路径数(子树节点数)。
    pub continuation_paths: usize,
}

impl TrieNode {
    /// 创建根节点或空节点。
    pub fn new(code: Option<KeySequence>, parent: Option<usize>) -> Self {
        Self {
            code,
            children: BTreeMap::new(),
            parent,
            slots: Vec::new(),
            direct_mass: 0.0,
            subtree_mass: 0.0,
            direct_count: 0,
            subtree_count: 0,
            continuation_paths: 0,
        }
    }

    /// 当前前缀是否可以继续输入(子树存在后继分支)。
    pub fn can_continue(&self) -> bool {
        !self.children.is_empty()
    }

    /// 码长(根节点为 0)。
    pub fn depth(&self) -> usize {
        self.code.as_ref().map_or(0, |c| c.len())
    }

    /// 查找指定文本在当前节点的名次(1-based, 未找到返回 None)。
    pub fn find_rank(&self, text: &str) -> Option<usize> {
        self.slots
            .iter()
            .position(|c| c.text == text)
            .map(|idx| idx + 1)
    }
}

/// 前缀闭合码空间 Trie。
#[derive(Clone, Debug)]
pub struct PrefixTrie {
    /// 扁平 Arena 存储全部树节点, 索引即节点 ID。
    nodes: Vec<TrieNode>,
}

impl Default for PrefixTrie {
    fn default() -> Self {
        Self::new()
    }
}

impl PrefixTrie {
    /// 创建仅含根节点的初始前缀树。
    pub fn new() -> Self {
        let root = TrieNode::new(None, None);
        Self { nodes: vec![root] }
    }

    /// 节点总数。
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// 是否仅有根节点。
    pub fn is_empty(&self) -> bool {
        self.nodes.len() <= 1
    }

    /// 获取根节点。
    pub fn root(&self) -> &TrieNode {
        &self.nodes[0]
    }

    /// 确保从根到 `code` 的整条前缀路径闭合存在。
    ///
    /// 例如插入 `uior` 将自动确保 `""`, `u`, `ui`, `uio`, `uior` 均建立节点。
    /// 返回末端节点的索引。
    pub fn ensure_prefix_path(&mut self, code: &KeySequence) -> usize {
        let mut current_idx = 0;
        let keys = code.as_slice();

        for i in 0..keys.len() {
            let key = keys[i];
            if let Some(&child_idx) = self.nodes[current_idx].children.get(&key) {
                current_idx = child_idx;
            } else {
                // 构造中间或末端前缀 KeySequence
                let sub_keys = &keys[0..=i];
                let sub_seq =
                    KeySequence::from_keys(sub_keys).expect("不变量: 切片非空必能构造 KeySequence");
                let new_idx = self.nodes.len();
                let new_node = TrieNode::new(Some(sub_seq), Some(current_idx));
                self.nodes.push(new_node);
                self.nodes[current_idx].children.insert(key, new_idx);
                current_idx = new_idx;
            }
        }

        current_idx
    }

    /// 查找指定前缀的节点索引。
    pub fn find_node_index(&self, code: &KeySequence) -> Option<usize> {
        let mut current_idx = 0;
        for &key in code.as_slice() {
            match self.nodes[current_idx].children.get(&key) {
                Some(&child_idx) => current_idx = child_idx,
                None => return None,
            }
        }
        Some(current_idx)
    }

    /// 获取指定前缀节点引用。
    pub fn get_node(&self, code: &KeySequence) -> Option<&TrieNode> {
        self.find_node_index(code).map(|idx| &self.nodes[idx])
    }

    /// 获取指定前缀节点可变引用。
    pub fn get_node_mut(&mut self, code: &KeySequence) -> Option<&mut TrieNode> {
        self.find_node_index(code).map(|idx| &mut self.nodes[idx])
    }

    /// 依据索引获取节点。
    pub fn node(&self, idx: usize) -> &TrieNode {
        &self.nodes[idx]
    }

    /// 依据索引获取节点可变引用。
    pub fn node_mut(&mut self, idx: usize) -> &mut TrieNode {
        &mut self.nodes[idx]
    }

    /// 在指定前缀码放置候选(自动保证前缀路径存在)。
    ///
    /// 若文本已在槽位中, 则保留原槽位或更新属性; 否则插入并重新按质量与冻结状态排序。
    pub fn insert_candidate(&mut self, code: &KeySequence, mut candidate: SlotCandidate) {
        let node_idx = self.ensure_prefix_path(code);
        let node = &mut self.nodes[node_idx];

        // 检查是否已有同名候选
        if let Some(pos) = node.slots.iter().position(|c| c.text == candidate.text) {
            // 已存在同名候选, 若新候选优先级更高或属性更新, 则刷新
            if !node.slots[pos].is_frozen || candidate.is_frozen {
                node.slots[pos] = candidate;
            }
        } else {
            candidate.is_continuation = node.can_continue();
            node.slots.push(candidate);
        }

        // 维护槽位排序: 冻结层在前 > 质量降序 > 文本字典序
        self.sort_node_slots(node_idx);
    }

    /// 重新按规范顺序排序节点的槽位, 并重置 rank (1-based)。
    pub fn sort_node_slots(&mut self, node_idx: usize) {
        let node = &mut self.nodes[node_idx];
        let can_continue = node.can_continue();

        node.slots.sort_by(|a, b| {
            b.is_frozen
                .cmp(&a.is_frozen)
                .then(b.mass.total_cmp(&a.mass))
                .then(a.text.cmp(&b.text))
        });

        for (idx, slot) in node.slots.iter_mut().enumerate() {
            slot.rank = idx + 1;
            slot.is_continuation = can_continue;
        }

        node.direct_mass = node.slots.iter().map(|s| s.mass).sum();
        node.direct_count = node.slots.len();
    }

    /// 查询指定前缀的候选列表。
    ///
    /// 核心不变量: 查询前缀得到有序候选, 但绝不隐含自动提交。
    pub fn query(&self, prefix: &KeySequence) -> &[SlotCandidate] {
        self.get_node(prefix).map_or(&[], |node| &node.slots)
    }

    /// 全树后序遍历重算子树统计信息(subtree_mass, subtree_count, continuation_paths)。
    pub fn recompute_subtree_stats(&mut self) {
        let total_nodes = self.nodes.len();
        // 节点拓扑序自底向上: 由于父节点索引严格小于子节点索引, 逆序遍历即可完成自底向上聚合
        for idx in (0..total_nodes).rev() {
            let direct_mass = self.nodes[idx].slots.iter().map(|s| s.mass).sum();
            let direct_count = self.nodes[idx].slots.len();

            let mut subtree_mass = direct_mass;
            let mut subtree_count = direct_count;
            let mut continuation_paths = 0;

            // 子节点已先于父节点完成聚合
            let child_indices: Vec<usize> = self.nodes[idx].children.values().copied().collect();
            for child_idx in child_indices {
                subtree_mass += self.nodes[child_idx].subtree_mass;
                subtree_count += self.nodes[child_idx].subtree_count;
                continuation_paths += 1 + self.nodes[child_idx].continuation_paths;
            }

            self.nodes[idx].direct_mass = direct_mass;
            self.nodes[idx].direct_count = direct_count;
            self.nodes[idx].subtree_mass = subtree_mass;
            self.nodes[idx].subtree_count = subtree_count;
            self.nodes[idx].continuation_paths = continuation_paths;

            let can_continue = !self.nodes[idx].children.is_empty();
            for slot in &mut self.nodes[idx].slots {
                slot.is_continuation = can_continue;
            }
        }
    }

    /// 收集全部被占用的码序列及其候选列表(字典序)。
    pub fn all_occupied_codes(&self) -> Vec<(KeySequence, &[SlotCandidate])> {
        let mut result = Vec::new();
        for node in &self.nodes {
            if let (Some(code), false) = (&node.code, node.slots.is_empty()) {
                result.push((code.clone(), node.slots.as_slice()));
            }
        }
        result.sort_by(|a, b| a.0.cmp(&b.0));
        result
    }
}
