# 分层词语数据

`wanxiang_base_words.tsv` 是本项目的 **hot 固定高频层**；
`wanxiang_extended_words.tsv` 是其后续 **extended 冷词层**。每个
`(词, 规范读音序列)` semantic entry 一行,分数来自万象 / RIME-LMDG 基础
词库,按项目规范无调读音归一化并聚合全部声调变体。

它不是 Rime 词典,不包含编码或 Rime 权重;`xhup-generator` 在生成期用
`DoublePinyinLayout` 把逐字规范读音推导为精确词码(2 字 → 4 键,3 字 →
6 键,4 字 → 8 键),再投影为确定性的静态 Rime 权重。规范汉字与规范读音
的 core 语言学关系仍由 `data/hanzi/` 承载；两个词表只提供词语语义与排名
证据，**不决定一个合法组合是否能输入**。任何逐字有两键输入音码的文本都可经
隔离的 `xhup_flow_flow` 单字原语参与开放组句。

产品不变量：

```text
hot static lexicon ⊂ pinned extended lexicon ⊂ reachable phrases
```

“词库决定排多前，不决定能不能打。”例如“提示词”在 pinned 万象源中但位于
旧 30,000 截断之外，现在由 extended 层以 `ti + ui + ci = tiuici` 提供 exact
候选；“提嗯诶”等两个 TSV 都不存在的组合则由相同逐字音码原语组句。

## 来源

- 来源仓库:[`amzxyz/rime-wanxiang`](https://github.com/amzxyz/rime-wanxiang)
- 固定提交:`4618d67a978ff4f41b165c10b35558d38e333ab1`
- 来源文件:`dicts/jichu.dict.yaml`
- 来源 Git Blob SHA:`a0f66e2fc6130f3f1c9b2e5109644c8b893477b0`
- 语义上游:[`amzxyz/RIME-LMDG`](https://github.com/amzxyz/RIME-LMDG)
- 许可证:CC BY 4.0(全文见 [`LICENSE.wanxiang`](LICENSE.wanxiang))

## 提取与复现

提取器是仓库自带的确定性工具(不访问网络,输入为本地源文件):

```console
cargo run -p xhup-generator --example extract_wanxiang_words -- \
    /path/to/jichu.dict.yaml > wanxiang_base_words.tsv
cargo run -p xhup-generator --example extract_wanxiang_words -- \
    /path/to/jichu.dict.yaml --extended > wanxiang_extended_words.tsv
```

对 pin 住的源文件重新提取,输出必须与本目录入库 TSV 字节级一致。
声调归一化与字频提取器共用同一实现
(`examples/common/wanxiang.rs`),规则:去声调五组;`ü` 族 → `v`;
`ńňǹ` → `n`;`ḿ` → `m`;归一化后仍含非 `a-z` 字符的源行显式忽略。

## 选择与过滤规则(与 TSV 注释头一致)

1. 仅保留三字段源行 `词<TAB>带调拼音序列<TAB>分数`;
2. 仅保留 2/3/4 个 Unicode 标量的词,且拼音数与字数一致;
3. 逐字规范校验:每个字属于规范 8105 清单、归一化读音等于该字某个规范
   读音、且该读音可编码为 XHUP 输入音节;不发明新读音;
4. 落到同一 `(词, 规范读音序列)` 的全部源行分数按 u64 校验和聚合;
5. **collision policy(二字词)**:4 键词码可能与规范单字全码碰撞
   (如 什么 = ufme 与生僻字「𬳽」同码)。**碰撞不删除任何 semantic
   entry**——词与字在同码上合法共存;碰撞码的候选次序由生成器
   `merged_ranking` 按同源万象频率证据跨表仲裁(高频词排在生僻字之前),
   权重跨表唯一、无平局。词汇存在性绝不因码碰撞被剥夺;
6. **shortcut protection**:被规范简码数据(`data/shortcuts/*.tsv`)
   引用的 `(词, 完整码)` 与 FIXED_FIRST shortcut 目标码上的词层占用者
   不受 top-N 截断影响,必然入选;必要时从该词长池频率尾部逐出等量
   未受保护条目腾位;受保护 `(词, 完整码)` 在源数据中无匹配时提取器
   直接失败(简码悬空不容静默);
7. hot 层各词长独立按 `(分数降序, 词 Unicode 升序, 读音序列升序)` 选取
   前 50,000(2 字)/ 30,000(3 字)/ 20,000(4 字)条(含保护递补),
   合计 100,000 条;合法候选不足目标时提取器直接失败,不静默缩水;
8. extended 层在完整应用 hot 选择与简码保护后，保留其余全部通过校验且
   未与 hot `(词,码)` 重合的候选；不再施加第二个 Top-N；
9. 最终按 `(词长, 词, 读音序列)` 升序序列化,UTF-8、LF、恰好一个
   末尾换行、无 BOM、无时间戳/路径/主机信息。

## 覆盖审计(提取时实测)

- 源三字段行:1,405,859
- 二字词 semantic entries:159,878
- 其中与规范单字全码碰撞(共存保留,不再排除):8,220
  (涉及 2,790 个 distinct 全码;高频示例如「但是 djui」「比如 biru」
  「选择 xrze」「数据 uuju」「什么 ufme」——这些词码恰是某些单字的
  规范全码,碰撞码上词与单字按频率证据共存排序)
- 简码保护:宿主 `(词, 完整码)` 68,842 条 + FIXED_FIRST 目标码 2,933 个;
  二字池保护递补 1,213 条(逐出等量频率尾部条目)
- 合法候选:2 字 159,878 / 3 字 597,275 / 4 字 645,765
- 入库 semantic entries:100,000(2 字 50,000 + 3 字 30,000 + 4 字 20,000)
- extended semantic entries:1,301,434(2 字 108,394 / 3 字 567,275 /
  4 字 625,765)，与 hot `(词,码)` 无重复；“提示词”来自该固定上游快照
- 上游 pinned 源每个词形恰好对应一个读音序列(经全量审计,不存在同一词形
  多个读音序列的行)
