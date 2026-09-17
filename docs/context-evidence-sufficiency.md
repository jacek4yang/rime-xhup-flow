# Committed-Context 证据充分性度量(2026-09-17)

Issue #83 §20 / §24 要求「committed left context 能真实改变**合理歧义样例**的
首选路径」,并把 `false/harmful reorder rate` 列入必测指标。本文档记录该里程碑
的度量口径与**真实数据实测结论**。

## 度量口径(已可执行)

`crates/xhup-analyzer/src/contextual_benchmark.rs` 的 runner 已泛型化,同一
fixture 可在两个 scorer 上跑同一套指标:

```bash
cargo run --locked -p xhup-analyzer --bin contextual-bench -- \
  --input data/benchmarks/contextual-v1.json \
  --bigram data/corpus/kdconv_bigram.tsv \
  --compare-context
```

输出(`contextual_benchmark::scores`):

| 指标 | 定义 |
|---|---|
| `context_gain` | baseline **未**命中 top1 路径、上下文 scorer **命中** 的 case 数 |
| `harmful_reorder_rate` | baseline 命中、上下文 scorer **未**命中的 case 占比(§20 `harmful reorder rate`) |
| `net_gain` | `context_gain_rate − harmful_reorder_rate`(可为负) |

「命中」严格定义为 top1 路径的 edge 序列 == 期望 edge 序列,不是文本相等;
这样「表内组句碰巧文本一样」不会被误计为上下文收益。

`--bigram` 必须是显式路径:不隐式读取生产数据、不做网络回退(离线红线)。

## 实测结论 1:现有 4 例 fixture 的上下文增益为 0

`data/benchmarks/contextual-v1.json` 的决胜转移在真实 KDConv bigram 中
**全部计数为 0**:

| case | 上下文 | 需要的转移 | KDConv 计数 |
|---|---|---|---|
| research-life-dev | 这个课题关注 | 研究→生命 | 0 |
| graduate-prefix-eval | 这名 | 研究生→命、这名→研究生 | 0 |
| university-life-dev | 丰富多彩 | 大学→学生 | 0 |
| student-prefix-eval | 这位 | 大学生→生、这位→大学生 | 0 |

因此真实 bigram scorer 在该 fixture 上既无增益也无退化
(`context_gain_rate = 0.0000`,`harmful_reorder_rate = 0.0000`)。

这不是实现缺陷,而是 **fixture 的期望来自人工语言学判断,而非证据**。
`data/benchmarks/README.md` 已声明首批 fixture「只锁定联合分段基础,不冒充
完整 2.0 评测集」;本文档把该声明量化。

回归哨兵:若 fixture 或证据来源变化导致读数改变,`tests/context_scores.rs`
中的对应断言会显式失败,而不是静默漂移。

## 实测结论 2:KDConv 会话域对前缀空间歧义的覆盖极稀疏

对 canonical v2 PRIMARY 码表全量枚举「同一 4+ 键码可由两个已有更短码串接
而成」的歧义实例(即前缀空间里真正会竞争的切分):

```
可二分歧义实例:        5308
其中有任何 in-path 转移证据:   83  (1.6%)
其中 in-path 证据 count >= 2: 30
其中 in-path 证据 count >= 5:  9
```

证据计数分布(capped at 10):`0 → 5225`,`1 → 53`,`2 → 13`,`3 → 7`,
`4 → 1`,`5 → 1`,`7 → 1`,`9 → 1`,`10 → 6`。

**结论:98.4% 的候选歧义在 KDConv 中没有可用的词级转移证据。** 单靠
`kdconv-bigram/v1` 无法支撑「上下文真实消歧」这一产品承诺,必须先补证据源
(§5 的 spoken/subtitle 域、§25 第 6 步的第二证据源)。

## 实测结论 3:链路本身是通的,证据够时上下文能真实翻转首选

在上述 5308 个实例中确有强证据样本,例如:

| 上下文 | 支持 | 不支持 | KDConv 计数 |
|---|---|---|---|
| 这个 (vege) | 景点 (jd) | 乐队 (yd) | 2019 vs 146 |

两路右侧码均为 2 键,故切分总键数相同、baseline 分数只由词频决定,决策
完全交给转移证据。`tests/context_scores.rs::real_transition_evidence_flips_ranking_toward_context_supported_path`
以真实 XML/真实码表断言:

- baseline(忽略上下文)选词频更高的「乐队」;
- `KdconvBigramScorer` 选证据支持的「景点」。

即「committed context 改变首选路径」的实现链路已验证,缺的是**覆盖**。

## 实测结论 4:真实句子回放给出正收益(+8.0pp)

`contextual-v1.json` 的 4 例人工 fixture 读不到增益(结论 1),但**真实句子
回放**可以。`context-replay-bench` 对入库夹具
`data/corpus/replay_fixture.txt`(2000 句 KdConv 派生,Apache-2.0,SHA256 pin)
最大匹配分词后逐 token 回放,每个 token 用「已提交前文 + 该 token 的
canonical 词码」构造**生产真实菜单**:

```bash
cargo run --release --locked -p xhup-analyzer --bin context-replay-bench -- \
  --sentences data/corpus/replay_fixture.txt \
  --bigram data/corpus/kdconv_bigram.tsv \
  --baseline data/benchmarks/context-replay-baseline.json
```

| 指标 | 值 |
|---|---|
| tokens(有 canonical 词码) | 5729 |
| ambiguous(同码歧义) | 2701 (47.1%) |
| baseline rank1 | 5085 (88.76%) |
| contextual rank1 | **5535 (96.61%)** |
| context_gain | **+458 (+7.99pp)** |
| harmful_reorder | 8 (0.14%) |
| net_gain | **+0.0785** |

**这是上下文解码收益的首个真实语料量化证据。**

8 个 harmful 经诊断为**同一处真实近义歧义**:`你知道` + code `tade`
(`他的` 227 vs `它的` 230),不是系统性退化。

### 为什么 fixture 读不到而句子回放能读到

- fixture 用的是**人工选定的切分对**,其决胜转移恰好都不在 KDConv 中;
- 句子回放用的是**语料里真实相邻的词对**,天然落在 KDConv 覆盖内。

这解释了 1.6% 的「码表内歧义证据覆盖率」为何不阻碍真实回放取得收益:
两者度量的对象不同。**但不代表覆盖率问题已解决** —— 生产运行时面对的是
用户实际按键序列,不是语料分词结果。

## 实测结论 5:有界 beam 与穷举排序在 beam≥32 时完全一致

`decode_beam`(生产路径)与 `rank_paths` + 全路径枚举(参考路径)在同一
2701 个歧义 token 上的 top1 一致性:

| beam_width | top1_agreement | truncated |
|---|---|---|
| 2 | 0.9563 | 0.5639 |
| 4 | 0.9959 | 0.1807 |
| 8(当前 `DecodeConfig` 默认) | 0.9981 | 0.0111 |
| 16 | 0.9996 | 0.0022 |
| **32** | **1.0000** | **0.0000** |
| 64 | 1.0000 | 0.0000 |

结论:

- 真实菜单扇出未超过 32,故 `beam_width ≥ 32` 无截断且与穷举**逐 token 一致**;
- 当前默认 `beam_width = 8` 有 **0.19% 的 top1 偏差**(约 5/2701);
  该代价此前从未被量化;
- 窄 beam(2/4)会显著截断并丢失一致性,不能当作「免费优化」。

## 已知限制(不谎报)

- 本结论只覆盖 `xhup-decoder` 的 lattice 级 scorer,不涉及 librime 实机菜单。
- **只回放有 canonical 词码的 token**(词码层 4/6/8 键);无词码 token 走单字
  组句,由 `replay` 静态层与 open-composition 可达性测试覆盖。因此
  **+7.99pp 不等于全链路 KSPC/rank 改进**。
- KDConv 是任务导向对话(电影/音乐/旅游),域偏差已知。
- `harmful_reorder_rate = 0.14%` 尚未成为 CI 硬门禁(先度量、后设门);
  它提示 §6 需要显式的弱证据降级策略。
- `data/corpus/replay_fixture.txt` 是**高频句**夹具(去重后按出现次数降序取
  top-2000),不等于随机语料分布。

## 下一步(依赖关系显式)

1. 引入第二转移证据源(§25 第 6 步),目标:提升 5308 个码表内歧义的证据
   覆盖率(当前 1.6%),因为生产运行时面对的是用户按键序列而非语料分词;
2. 证据到位后,按 §6 为弱证据设计显式降级阈值,并把 `harmful_reorder_rate`
   纳入 CI 门禁(与 misleading-hint rate 同等地位);
3. 把 `decode_beam` 接入实际生产路径(当前只在基准中对比),并评估把
   `beam_width` 从 8 提到 32 的延迟代价(§22);
4. fixture 扩展时保持 development/evaluation 分离,避免用 evaluation case 调参。

