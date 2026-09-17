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

## 已知限制(不谎报)

- 本结论只覆盖 `xhup-decoder` 的 lattice 级 scorer,不涉及 librime 实机菜单。
- KDConv 是任务导向对话(电影/音乐/旅游),域偏差已知;计数 ≥1 在会话域
  噪声水平附近,不宜作为生产重排的唯一依据。
- `harmful_reorder_rate = 0` 目前是「上下文几乎不触发」的结果,不是「上下文
  被充分验证安全」的结果。补足证据源后该指标必须重新测量。

## 下一步(依赖关系显式)

1. 引入第二转移证据源(§25 第 6 步),目标:让 5308 个歧义实例中有证据的
   比例显著高于 1.6%;
2. 证据到位后,按 §6 为弱证据设计显式降级阈值,并把 `harmful_reorder_rate`
   纳入 CI 门禁(与 misleading-hint rate 同等地位);
3. fixture 扩展时保持 development/evaluation 分离,避免用 evaluation case 调参。
