# XHUP 输入编码事实

本目录保存“这个字符可由哪些小鹤音码/形码输入”的事实。它与
`data/hanzi/readings.tsv` 的语言学读音、`data/pinyin/` 的 406 音节集合以及
`data/shape/` 的规范 8105 字形码相互独立：规范 8105 字只是
`CoreStandardHanzi` 子集，不是输入法字符全集。
当前 8,208 只是 pinned 证据快照的覆盖数，不是把旧 8,105 上限换成另一个
永久上限；新增有许可、可复现的编码证据即可机械扩展生产成员集合。

## 数据层

- `official_char_code_oracle.tsv`：通过小鹤官网查形页及其
  `ixdata.json` 核验的少量兼容性事实。官网没有明确的可再分发数据许可，故这里只
  记录回归所需的事实、稳定 oracle/parser 版本与完整响应哈希，不镜像全站数据，
  也不把抓取时间写入 canonical 输出。`-`、`+` 等页面标记被保存为 status 语义，
  不篡改为语言学拼音。
- `attested_char_codes.tsv`：生产使用的确定性并集，由仓库 extractor 从下列已固定
  输入生成。每行是 `字符、两键音码、两键形码、来源权重、source、status`；同一
  编码可保留多份来源证据，生成候选时再按 `(字符, 音码, 形码)` 合并。

## 可复现来源

历史兼容层来自 `boomker/rime-fast-xhup`：

- commit: `308d6d29c5a612fec7282923e49d3cd5453bab48`
- path: `cn_dicts/flypy_chars.dict.yaml`
- git blob: `3c2773335c8108bbe896b9af588d619e22045132`
- raw SHA-256: `5eeda7a9976cf7d8ed1bc487bdccc02d7f423425515d814309328ff61b6b4291`
- license: LGPL-3.0（上游 `LICENSE` blob
  `0a041280bd00a9d068f503b8ee7ce35214bd24a1`）

离线重生成（命令本身不访问网络）：

```bash
cargo run -p xhup-generator --example extract_attested_char_codes -- \
  /path/to/pinned/flypy_chars.dict.yaml \
  data/xhup/official_char_code_oracle.tsv \
  > data/xhup/attested_char_codes.tsv
```

提取器严格验证输入格式、唯一性与顺序，输出使用固定 provenance、固定排序且不写入
运行时间。构建、测试、Rime deploy 和输入法运行只读取已提交 TSV，完全离线。

## 覆盖与运行时语义

- historical 9,792 行 + official oracle 4 行 = 9,796 evidence 行；
- 8,208 个 `InputHanzi`，其中 8,105 个 core + 103 个扩展字符；
- 去重并投影 2/3/4 键后共 28,851 条生产关系（9,254 / 9,724 /
  9,873），涉及 414 / 5,013 / 9,027 个 distinct code；
- 官网 oracle 冻结「嗯」`ogkx` / `onkx` / `enkx` 与「诶」`eiyu`；
  `official-yield-full` 表示页面的“出简让全”，不是编码字符的一部分；
- 新关系遇到既有 static code 时只追加，不删除或重排旧候选；全部关系由
  static menu manifest 通过真实 librime 逐项验证可达。
