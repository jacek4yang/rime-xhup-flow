# Joint Key/Text Lattice 架构与运行时合同

状态:**本里程碑实现完成**(影子模式;#83 路线图第 7 个里程碑)

## 1. 运行时原生查找机制

### 1.1 已验证的 librime-lua API

**Component.Translator 实例化**(已验证,魔然 `moran_express_translator` 先例):

```lua
-- 在 init(env) 中实例化原生 table_translator
local primary = Component.Translator(env.engine, "", "table_translator")
local flow    = Component.Translator(env.engine, "", "table_translator@flow")
local learn   = Component.Translator(env.engine, "", "table_translator@learn")
```

通过 `@namespace` 后缀指定 schema 中的 translator 配置命名空间。

**候选字段暴露**:

| 字段 | 含义 | 可靠性 |
|------|------|--------|
| `cand.text` | 候选文本 | ✅ 已验证 |
| `cand.comment` | 候选注释 | ✅ 已验证 |
| `cand.type` | 候选类型 | ✅ 已验证(`table` / `user_table` / `sentence`) |
| `cand.quality` | 原生质量分数 | ✅ 已验证 |
| `cand.start` / `cand.end` | Segment 内偏移 | ⚠️ 依赖 Segment 边界 |

**候选类型可区分性**:

| `cand.type` | 含义 | CandidateSource 映射 |
|-------------|------|---------------------|
| `"table"` | 来自 table.bin 的静态词条 | `StaticTable` 或 `FlowTable`(取决于 translator namespace) |
| `"user_table"` | 来自 userdb 的用户学习词条 | `UserLearned` |
| `"sentence"` | 组句合成候选 | `SentenceComposition` |

**关键限制**:

1. 无法在运行时区分 hot vs extended 词条(同一 flow table 中)
2. `cand.start/end` 是 Segment 内偏移,不是原始按键偏移
3. 查询本身是只读的(不写 userdb),但 sentence translator 可能有内部状态

### 1.2 查询策略

对给定原始按键流 `raw_keys[0..n)`,lattice 构建需要查询所有合法前缀子串
的候选。生产策略:

1. 对每个起始位置 `start ∈ [0, n)`,构造从 `start` 开始的按键子串
2. 对每个子串长度 `len ∈ [1, min(n - start, max_key_span)]`,查询候选
3. 使用 `Component.Translator` 实例化的 translator 对子串做查找
4. 收集所有 `(span, text, source, quality, rank)` 元组

## 2. Rust/Lua 边界

### 2.1 职责划分

```text
Rust (xhup-decoder)          Lua (rime/lua/xhup_flow/)
━━━━━━━━━━━━━━━━━━━━━━       ━━━━━━━━━━━━━━━━━━━━━━━━━━━
Lattice 语义模型              原生 translator 查询
CandidateSource 枚举          候选元数据归一化
SourceEvidence 结构            证据收集与标准化
FusedCandidate 融合            重复证据融合
LatticeBounds 约束             有界 lattice 状态构建
LatticeBuildStats 统计         构建统计回报
确定性评分契约                 当前候选菜单透传
隐私安全 Debug                 零用户文本日志
```

### 2.2 合同版本

共享 fixture 格式:`xhup-joint-lattice-fixture/v1`

Rust 测试消费 `data/fixtures/joint-lattice-v1.json`,验证:
- 融合语义:同 (span, text) 对的多源证据融合为单一 FusedCandidate
- 确定性:相同输入 → 相同 lattice(不依赖 HashMap 迭代顺序)
- 有界性:truncation 保留 fallback 边优先
- 可达性:每个位置 0 到末端均可达(当 fallback 原语存在时)

Lua 测试消费同一夹具文件(`tests/lua/sim_native_lookup.lua` 内置最小
JSON 解码器,不引入外部依赖),验证:
- 融合语义与 Rust 一致:同 (span, text) 多来源 → 单一融合候选
- 证据来源顺序确定性(与 Rust CandidateSource Ord 对齐)
- 有界截断与 fallback 保留行为一致
- 输出顺序与插入顺序无关(确定性)

## 3. 按键 Span 语义

### 3.1 半开区间

Span `[start, end)` 以 ASCII 按键数计(0-based)。
对输入 `uiorabcd`(8 键):
- 位置 0 的合法出边 span: `[0,1)`, `[0,2)`, `[0,3)`, `[0,4)`, ...
- 位置 0 永远不可预提交

### 3.2 前缀闭合

`valid prefix != commit boundary`

位置 0 的 `[0,1)` span 产生候选(如 `u → 去`)不意味着自动提交。
用户随时可继续输入后续按键,展开更长候选路径。

### 3.3 重叠 span

同一位置可有多个不同长度的出边 span:

```text
位置 0:  u(1)  ui(2)  uio(3)  uior(4)
位置 2:  o(1)  or(2)
位置 3:  r(1)
```

这些 span 同时有效,未来解码器在它们之间选择最优路径。

## 4. 候选证据与融合

### 4.1 CandidateSource

运行时可真实区分的来源:

| Source | 来源 | 运行时可观察 |
|--------|------|------------|
| `StaticTable` | 主静态词典 translator | ✅ `cand.type == "table"` |
| `FlowTable` | Flow 组句词典 translator | ✅ `cand.type == "table"` (不同 namespace) |
| `UserLearned` | 用户学习词条 | ✅ `cand.type == "user_table"` |
| `SentenceComposition` | 组句合成 | ✅ `cand.type == "sentence"` |
| `CompilerMetadata` | 离线编译器元数据 | ✅ 确定性来源 |

### 4.2 融合规则

对同一 `(span, text)` 对:
- 合并所有来源的 `SourceEvidence` 到一个 `FusedCandidate`
- `max_frequency` 取所有证据中的最大值
- `compiler_kind` 从 CompilerMetadata 证据获取(如果有)
- 证据列表按 `CandidateSource` 排序(确定性)
- 不创建重复的语义路径

### 4.3 融合键

`FusionKey = (Span, text)` — 使用 BTreeMap 保证确定性。

## 5. 有界性策略

### 5.1 默认约束

| 参数 | 默认值 | 依据 |
|------|--------|------|
| `max_key_span` | 20 | 10 汉字 × 2 键音码 |
| `max_candidates_per_span` | 32 | 静态层最大碰撞 + 组句 |
| `max_outgoing_per_position` | 64 | 4 个 span 长度 × 16 候选 |
| `max_total_edges` | 2048 | Android 内存预算 |

### 5.2 截断策略

当超出约束时:
1. 按 `max_frequency` 降序排列候选
2. **优先保留** Character/OOV fallback 边(保证可达性)
3. 截断低频率候选
4. 记录截断事件到 `LatticeBuildStats`

### 5.3 安全不变量

**有界性不得静默破坏基本文本可达性**。

当截断必要时,保留:
1. 至少一条从位置 0 到末端的完整路径(如果 fallback 原语存在)
2. 每个位置至少一条出边(如果该位置有任何候选)

## 6. 隐私行为

- `FusedCandidate::Debug` 只输出证据数量、来源列表、频率,不输出文本
- `LatticeBuildStats` 只含计数/度量,不含任何候选文本
- Lua 模块不将候选文本写入普通日志
- 构建统计可安全地作为诊断信息输出

## 7. 性能考量

### 7.1 影子模式

本里程碑中,lattice 构建以**影子模式**运行:
- 构建 lattice 并收集统计
- **不改变**当前候选菜单输出
- 验证 lattice 可达性与有界性

如果影子构建开销过高,可降级为:
- 仅在首次按键时构建(不每键重建)
- 或由下一个 scorer 里程碑再启用始终构建

### 7.2 预期开销

- 候选查询:O(positions × span_lengths) 次 translator 查询
- 融合:O(raw_candidates × log(raw_candidates)) BTreeMap 操作
- 总体:bounded by `max_total_edges`

## 8. 已知限制

1. **无上下文评分**:本里程碑不实现 contextual scorer
2. **无 Beam/Viterbi**:不实现有界搜索
3. **无用户模型改造**:不改变 `xhup_flow_user` 语义
4. **无生产重排序**:候选菜单顺序保持不变(影子模式)
5. **Prefix-Space v3 未接入**:编译器产出的精确分类暂未作为运行时源
6. **hot/extended 不可区分**:运行时视为统一的 `FlowTable` 源
7. **运行时接线为纯逻辑层**:`native_lookup.lua` 的 span 枚举与融合为
   可单测的纯逻辑;真实 Segment 构造与每键触发将随 scorer 里程碑接入
   (需要影子模式下测量真实 translator 查询开销)

## 9. 下一步

本里程碑完成后,下一个里程碑:

**Committed-Context 统计评分器与可复现本地语言模型**

验收目标:
```text
相同原始按键 + 不同已提交左上下文 = 不同首选路径
```
