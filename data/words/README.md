# 规范高频词语数据

`wanxiang_base_words.tsv` 是本项目的**固定高频词语层语义数据**:每个
`(词, 规范读音序列)` semantic entry 一行,分数来自万象 / RIME-LMDG 基础
词库,按项目规范无调读音归一化并聚合全部声调变体。

它不是 Rime 词典,不包含编码或 Rime 权重;`xhup-generator` 在生成期用
`DoublePinyinLayout` 把逐字规范读音推导为精确词码(2 字 → 4 键,3 字 →
6 键,4 字 → 8 键),再投影为确定性的静态 Rime 权重。规范汉字与规范读音
的成员资格仍由 `data/hanzi/` 唯一承载,本文件只提供词语语义与排名证据。

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
5. **collision policy(二字词)**:按 semantic entry(词 + 读音序列)
   推导 4 键词码,与规范单字全码集(`xhup_flow_chars` 的 4 码层)冲突的
   该条 semantic entry 被排除;同一词形的不冲突读音序列不受影响;
6. 各词长独立按 `(分数降序, 词 Unicode 升序, 读音序列升序)` 选取
   前 50,000(2 字)/ 30,000(3 字)/ 20,000(4 字)条,合计 100,000 条;
   合法候选不足目标时提取器直接失败,不静默缩水;
7. 最终按 `(词长, 词, 读音序列)` 升序序列化,UTF-8、LF、恰好一个
   末尾换行、无 BOM、无时间戳/路径/主机信息。

## 覆盖审计(提取时实测)

- 源三字段行:1,405,859
- 二字词 semantic entries(collision 过滤前):159,878
- 因 FullCode 冲突排除的二字词 semantic entries:8,220
  (涉及 2,790 个 distinct 全码;高频示例如「但是 djui」「比如 biru」
  「选择 xrze」「数据 uuju」——这些词码恰是某些单字的规范全码,
  按 policy 让位给单字,后续由自适应/组句层处理)
- 合法候选(过滤后):2 字 151,658 / 3 字 597,275 / 4 字 645,765
- 入库 semantic entries:100,000(2 字 50,000 + 3 字 30,000 + 4 字 20,000)
- 上游 pinned 源每个词形恰好对应一个读音序列(经全量审计,不存在同一词形
  多个读音序列的行),因此真实数据中不存在「同一词形部分读音序列碰撞、部分
  保留」的样本;collision 过滤的 semantic-entry 粒度由 fixture 单元测试锁定
