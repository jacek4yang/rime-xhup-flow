# 规范读音频率数据

`wanxiang_reading_scores.tsv` 是本项目的**候选排名证据数据**：每个
`(规范汉字, 规范读音)` 关系一行，分数来自万象 / RIME-LMDG 单字频率数据，
按项目规范无调读音归一化并聚合全部声调变体。

它不是 Rime 词典，不包含编码、短码或 Rime 权重；`xhup-generator` 在生成期
把分数投影为确定性的静态 Rime 权重。规范汉字与规范读音的成员资格仍由
`data/hanzi/` 唯一承载，本文件只提供排名证据。

## 来源

- 来源仓库：[`amzxyz/rime-wanxiang`](https://github.com/amzxyz/rime-wanxiang)
- 固定提交：`7ec998b28c9a5c57260d2ba24b264c1c1820e0ef`
- 来源文件：`dicts/zi.dict.yaml`
- 来源 Git Blob SHA：`9a69cb891f2e0c158313d14e0ea6c3925ca081ef`
- 语义上游：[`amzxyz/RIME-LMDG`](https://github.com/amzxyz/RIME-LMDG)
- 许可证：CC BY 4.0（全文见 [`LICENSE.wanxiang`](LICENSE.wanxiang)）

选择万象数据的关键原因：它按 `(汉字, 读音)` 区分频率证据，而不是把多音字
折叠成单一数字——这对「行 / 长 / 重 / 乐」等多音字的候选排序至关重要。

## 提取与复现

提取器是仓库自带的确定性工具（不访问网络，输入为本地源文件）：

```console
cargo run -p xhup-generator --example extract_wanxiang_frequency -- \
    /path/to/zi.dict.yaml > wanxiang_reading_scores.tsv
```

对 pin 住的源文件重新提取，输出必须与本目录入库 TSV 字节级一致。

提取规则（与 TSV 注释头一致）：

1. 仅保留三字段源行 `汉字<TAB>带调拼音<TAB>分数`；
2. 带调拼音归一化为项目规范无调读音（去声调；`ü` 族 → `v`；`ńňǹ` → `n`；
   `ḿ` → `m`），归一化后仍含非 `a-z` 字符的行显式忽略；
3. 仅保留汉字属于规范 8105 清单、且归一化读音等于该字某个规范读音的源行，
   不发明新读音；
4. 落到同一 `(汉字, 规范读音)` 的全部源行分数按 u64 校验和聚合。

## 覆盖审计（提取时实测）

- 规范读音关系：8580
- 匹配关系（TSV 行数）：8544（覆盖率 99.58%）
- 缺失关系：36，均为万象未单列的罕用读音（如「呒 wu」「哼 hng」「欸 ea」）

缺失关系在生成期按分数 `0` 处理：它们仍是合法的规范编码条目，只是在同码
候选中自然排到队尾。分数 `0` 只表示「没有万象频率证据」，不表示读音无效。
