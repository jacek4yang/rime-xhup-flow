# 一级简码映射

`level1.tsv` 是 XHUP Flow **一级简码固定层的唯一事实来源**:每行
`键<TAB>汉字`,恰好 26 行,按 QWERTY 物理布局顺序
(`qwertyuiop` / `asdfghjkl` / `zxcvbnm`)序列化。

## 数据性质

一级简码属于 XHUP Flow 兼容的小鹤方案**显式设计数据**,不是由万象词频
自动推导得到,也不随候选排名数据变化。

小鹤官方公开更新记录确认一级简码机制仍持续存在(机制参考:
<https://www.flypy.cc/win_record.html>);本仓库采用本项目冻结并交叉核对
后的 26 键一级简码映射。

## 声码一致性

每个 `键 → 汉字` 关系必须与 XHUP 声码语义一致:该汉字至少存在一个规范
读音,其双拼码的**首键**等于简码键(例如 `q → 去(qu)`、
`u → 是(shi → ui)`、`v → 这(zhe → ve)`、零声母 `a → 啊`)。
该一致性由 `xhup-generator` 在解析时硬断言,矛盾即构建失败。

## 兼容策略

该 26 键映射作为**稳定的用户肌肉记忆兼容接口**维护:发布后 `q` 永远是
「去」、`w` 永远是「我」,依此类推。未来如需修改,应视为 breaking
scheme change。

一级简码只是一键精确候选的**新增别名**,不是替换:这些字的 2/3/4 码
规范关系全部保留(不实现「出简让全」);运行时只提供 exact candidate,
不自动上屏。一简词、二简字重排等概念不属于本层。

本文件不涉及第三方数据许可证;26 个离散键值事实为本项目维护的方案
设计数据。

# 词语简码(高稳健零冲突层)

`word_zero_regression.tsv` 是 XHUP Flow **词语简码层的唯一事实来源**:
每行 `词<TAB>完整码<TAB>简码<TAB>模式`(模式为逐字 F/I 投影,
F = 完整双拼两键,I = 双拼首键),按 `简码长度 → 简码 → 词` 的
canonical 顺序序列化(LF、无 BOM、恰好一个末尾换行)。

## 数据性质

与 `level1.tsv` 的显式设计数据不同,本文件是由
`data/words/wanxiang_base_words.tsv` 的词语/频率证据经 `xhup-analyzer`
的 production selection policy(`zero-regression-high-v1`)**确定性导出**
的选择集:

- candidate grammar:**legacy-any-fi-v1(冻结)**——任意含 I 的 F/I
  组合。枚举期同时应用 `最短 3 键` 过滤(该过滤是冻结枚举规格的一
  部分,不属于语法本身);
- profile:ZERO_REGRESSION(简码在 baseline fixed exact-code 空间中完全
  空闲,不与一级简码、单字 2/3/4 码、固定词 4/6/8 键冲突);
- reference run:balanced operating point × normalized(char:word = 50:50,
  3 码单字 conservative 归属假设);
- robustness gate:30 次 normalized sensitivity 运行中同码票数 ≥ 4/5
  (整数票数比较,非浮点),且最多票码与 reference assignment 码一致。

简码是**新增别名**,不替换完整码:每个词的完整码关系在固定词层完整保留。
本层不包含任何与既有编码冲突的关系(例如「时间」的 `uij`/`ujm` 与 3 码
单字冲突,明确不属于本层)。

## 兼容策略

本文件一旦发布即属于**稳定的用户肌肉记忆兼容接口**。更新它必须经由显式
analyzer export + diff review + policy version review;analyzer 算法的演进
不得自动删除或更换已发布的 `词 → 简码` 关系——修改已发布关系视为
breaking scheme change,与一级简码的肌肉记忆保护原则一致。

## Legacy 冻结映射(候选语法技术债)

本层 44,448 条映射由冻结的 legacy-any-fi-v1 语法生成,其中包含大量
**非单调**模式(44,448 条中 13,281 条:`IIF` 4,632、`IFI` 3,625、
`IF` 2,981、`IFF` 1,538、`IIIF` 446、`FIF` 40、`IFII` 14、`IIFI` 5)。

这些是 **legacy-v1 冻结映射,不是无效数据**:它们是已发布的用户肌肉
记忆,无限期保持支持,不删除、不「清理」、不由新语法重算。新的
production policy(candidate grammar
monotone-suffix-initials-v2,见 FIXED_FIRST 层)**不再生成**这些形式的
新映射;两类映射的分层由
`xhup-generator/tests/word_shortcut_grammar_audit.rs` 全量审计锁定。
未来如需迁移,必须经由显式 breaking scheme version,绝不静默进行。

## 许可证

本文件派生自万象词库证据,与 `data/words/wanxiang_base_words.tsv` 同源,
适用 CC BY 4.0 署名要求,许可证全文见
[`../words/LICENSE.wanxiang`](../words/LICENSE.wanxiang);再分发(包括由它
生成的 `xhup_flow_word_shortcuts.dict.yaml` 等数据产物)须保留该署名与
许可信息。该数据不因入库而改授 LGPL。

# 词语简码(高稳健 FIXED_FIRST 层)

`word_fixed_first.tsv` 是 XHUP Flow **第二层词语简码的唯一事实来源**:
格式与 canonical 顺序同 `word_zero_regression.tsv`(每行
`词<TAB>完整码<TAB>简码<TAB>模式`)。

## 数据性质

本文件由同一万象词语/频率证据经 `xhup-analyzer` 的 production selection
policy(`fixed-first-high-v1`)**确定性导出**,与零冲突层的差别在于
候选语法、候选语义与增量宇宙:

- candidate grammar:**monotone-suffix-initials-v2**——单调后缀缩写
  `F* I*`(至少一个 I;一旦某字缩写,其后所有字都缩写)。语法理论
  全集允许 2-key 候选(如 `时间 → uj/II`,audit-only);非单调模式
  (如 `IF`/`IFI`,首字缩写、末字全码)**结构性非法**,不再生成;
- production 最短长度:**3 键**(policy 过滤,不是语法语义;1/2 键
  空间保留给一级简码与单字双拼);
- profile:FIXED_FIRST(简码与 baseline fixed exact code **重码**,新候选
  严格追加到全部既有固定候选之后,名次 = baseline 候选数 + 1,既有候选
  相对次序绝对不变);
- target universe:全部词目标**先移除**已持有零冲突简码的词(优化前
  排除,不是分配后过滤);
- candidate universe:只保留与 baseline fixed exact code 重码
  (baseline 候选数 > 0)的候选,**不设上限**;selection-cost 模型对任意
  深度都有定义(rank 1 / 2..=9 / >=10 三档),深度分布只在 analyzer
  audit 中如实报告;
- reference run 与 robustness gate 与零冲突层相同(balanced ×
  normalized 50:50 conservative,30 次运行同码票数 ≥ 4/5)。

运行时本层由方案中独立的第二 `table_translator`(词典
`xhup_flow_fixed_first_shortcuts`,`initial_quality: 0`)加载;
primary translator 的 `initial_quality: 1000000` 只是 translator 间
优先级栅栏(常数同时加到全部 primary 候选,其内部相对次序不变),
不是候选词频。

## 兼容策略

与零冲突层相同:一旦发布即属于**稳定的用户肌肉记忆兼容接口**。
更新必须经由显式 analyzer export + diff review + policy version review;
analyzer 算法演进不得自动删除或更换已发布的 `词 → 简码` 关系,修改已
发布关系视为 breaking scheme change。

## 许可证

与 `word_zero_regression.tsv` 相同:派生自万象词库证据,适用 CC BY 4.0
署名要求,许可证全文见
[`../words/LICENSE.wanxiang`](../words/LICENSE.wanxiang)(包括由它生成的
`xhup_flow_fixed_first_shortcuts.dict.yaml` 等数据产物);不因入库而改授
LGPL。

# 词语简码(二码零冲突层)

`word_two_key_zero_regression.tsv` 是 XHUP Flow **第三层词语简码的唯一
事实来源**:格式 `词<TAB>完整码<TAB>简码<TAB>模式`(每行模式恒为
`II`),canonical 顺序 `简码 → 词`(全部 2 键,长度无差异)。

## 数据性质

本文件由万象词语/频率证据经 `xhup-analyzer` 的 production selection
policy(`two-key-zero-regression-v1`)**确定性导出**:

- candidate grammar:**monotone-suffix-initials-v2**(2 字词 × `II`
  模式 × 2 键;两字双拼首键,如 `时间 uijm → uj`,但 `uj` 为占用码,
  结构性不在本层);
- candidate universe:**仅完全空闲的 2 键 exact code**(baseline
  2/3/4 码单字、4/6/8 键固定词与既有词语简码层全部未使用)—— 新词
  是该码唯一的 exact 候选(rank 1),严格零冲突,与
  `word_zero_regression.tsv` 同构但作用于 2 键空间;
- target universe:无既有 ZR/FIXED_FIRST 简码的 2 字词(一词最多一条
  简码的现行政策);
- 竞争:每个 2 键码恰选一词,确定性排序(净收益 DESC → 频率 DESC →
  词 ASC → 完整码 ASC);
- robustness gate:30 次 normalized 敏感性网格中同码同词胜出票数
  ≥ 4/5(整数交叉乘法判定),且 reference 运行净收益为正;
- reference run:balanced × normalized(50:50,Conservative)。

**占用码(char fanout > 0)的 2 键候选不在本层**:对占用码的
SAFE_APPEND(词追加到既有 2 键单字之后)与 OPTIMAL_INSERT(允许重排
的理论上限)仅存在于 analyzer 研究报告(`static-shortcut-audit
--study-two-key`),是否生产化属未来独立 policy 决策。

## 运行时

本层词典 `xhup_flow_two_key_shortcuts` 由顶层词典 import(与 ZR 层
同构):选定码在既有 exact-code 空间完全空闲,新增候选是该码唯一
exact 候选,不存在次序影响;安全来自「码本来就空」,不依赖 import
顺序。

## 兼容策略

与零冲突层相同:一旦发布即属于**稳定的用户肌肉记忆兼容接口**。
更新必须经由显式 analyzer export + diff review + policy version
review;analyzer 算法演进不得自动删除或更换已发布的 `词 → 简码`
关系,修改已发布关系视为 breaking scheme change。

## 许可证

与 `word_zero_regression.tsv` 相同:派生自万象词库证据,适用 CC BY
4.0 署名要求,许可证全文见
[`../words/LICENSE.wanxiang`](../words/LICENSE.wanxiang)(包括由它
生成的 `xhup_flow_two_key_shortcuts.dict.yaml` 等数据产物);不因
入库而改授 LGPL。
