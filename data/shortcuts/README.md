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
候选语义与增量宇宙:

- profile:FIXED_FIRST(简码与 baseline fixed exact code **重码**,新候选
  严格追加到全部既有固定候选之后,名次 = baseline 候选数 + 1,既有候选
  相对次序绝对不变);
- target universe:全部词目标**先移除**已持有零冲突简码的词(优化前
  排除,不是分配后过滤);
- candidate universe:只保留 baseline 候选数在 1..=8 的重码候选(更深的
  候选超出当前 selection-cost 模型的语义边界,延期到未来更细粒度的
  selection/page model);
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
