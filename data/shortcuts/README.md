# XHUP Flow canonical 简码数据

## 一级简码

`level1.tsv` 是一级简码固定层的唯一事实来源:每行
`键<TAB>汉字`,恰好 26 行,按 QWERTY 物理布局顺序序列化。这是本项目
冻结的显式设计数据,不是由万象词频自动推导。小鹤官方公开更新记录确认
一级简码机制仍持续存在(机制参考:<https://www.flypy.cc/win_record.html>);
本仓库采用冻结并交叉核对后的 26 键映射。每个关系均由 generator 硬断言
符合 XHUP 声码;只增加一键 exact 别名,不替换该字的 2/3/4 码关系,
也不自动上屏。

## optimizer v2 词语简码

production 词语简码只有一套事实来源:

- `word_shortcuts_primary.tsv`:65,909 条 PRIMARY 映射;
- `word_fixed_first.tsv`:2,933 条 FIXED_FIRST 映射。

两文件必须视为同一个 canonical v2 mapping 的无交叉分区,并集恰为
68,842 条。两者文件头共享下列 provenance:

- selected operating point:
  `rk-steep|a0.25|d0.5|x1|e-conversation`;
- selected dump SHA256:
  `a451bce1187efba0edbd7d3d02bc17331fb3da9dc903625e57c3dc310b6c60ea`;
- selected mapping SHA256:
  `a8ab5f6deae91f28e02f790153cfba9ab493605ee9e84aad0abea5a660012cdc`;
- 导出工具:`export-v2-canonical`(确定性导出,禁止手改)。

### PRIMARY

每行为:

```text
词<TAB>shortcut<TAB>rank<TAB>merged_rank
```

`rank` 是同码 PRIMARY 条目之间稠密的 `1..k` 相对位次;
`merged_rank` 是与 baseline 单字/固定词候选混排后的绝对位次。PRIMARY 包含
2 键简码及 legacy `IF` 等无法用单调 FIXED_FIRST 格式表达的传统别名。

parser 严格验证词宇宙成员资格、小写键码、简码严格短于全码、词唯一、
同码 rank 稠密唯一、merged rank 严格递增及 canonical 序列化顺序;
任一损坏均 fail-fast。

### FIXED_FIRST

每行为:

```text
词<TAB>完整码<TAB>shortcut<TAB>模式
```

只有同时满足以下条件的 v2 rank-1 条目进入本文件:

- shortcut 与 baseline fixed exact code 重码;
- 按完整双拼两键 `F` /首键 `I` 机械投影;
- 模式为单调后缀缩写 `F* I*`;
- 长度至少 3 键且严格短于完整码。

2 键或非单调传统别名不会被伪装成 FIXED_FIRST,而是由 PRIMARY
的 `merged_rank` 表达。

### runtime merged ranking

PRIMARY、FIXED_FIRST 和 baseline 共用一个 Rime `table_translator`。生成器以
`merged_rank` 将每个 exact code 的完整有序菜单投影为严格唯一、连续的
正整数 weight:不依赖 TSV/import 顺序,无浮点平局,也不让 userdb
改写静态次序。Flow/学习 translator 的 `initial_quality` 只用于将动态候选
置于完整静态组之后。

## legacy v1 research fixtures

`legacy/` 下的三个文件只用于重放历史 selector、兼容性对照和研究:

- `word_zero_regression_v1.tsv`;
- `word_fixed_first_v1.tsv`;
- `word_two_key_zero_regression_v1.tsv`。

它们不会进入 production Rime/Trainer 生成管线,不是当前 canonical,
也不与 v2 形成第二套 production truth。

## 兼容与许可证

v1.0.0 发布后,68,842 条 `词 → shortcut` 及同码菜单次序是 v1.x
稳定的肌肉记忆接口;更换既有关系必须视为 breaking scheme change。

词语简码派生自 `data/words/wanxiang_base_words.tsv` 的词汇/频率证据,
适用 CC BY 4.0 署名要求。许可证全文见
[`../words/LICENSE.wanxiang`](../words/LICENSE.wanxiang);包含这些数据的生成包
必须保留署名与许可说明,不因入库而改授 LGPL。
