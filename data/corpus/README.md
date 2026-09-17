# data/corpus — 语料派生统计(会话域)

语料证据层的入库产物:**只有聚合计数**(词 → 计数/句子覆盖/上下文多样性),
原始语料与句子绝不入库。来源与许可决策见 docs/data-pipeline.md。

## conversation_kdconv.tsv(会话域)

- **来源**:[thu-coai/KdConv](https://github.com/thu-coai/KdConv) 三个域
  (film/music/travel)的 train/dev/test 对话发言,共 92,558 句。
- **许可**:Apache-2.0(LICENSE.kdconv 为 vendor 副本)。知识库文件
  (kb_*.json)**未**使用。
- **版本 pin**:commit `653db76432de09a004ba708a68f8bbd5500e6bec`。
- **来源文件 SHA256**(下载后校验):

  | 文件 | SHA256 |
  |---|---|
  | film_train.json | b48a491c30ace856914c6863679ba5c8ea95d7e3a1734f45177f69afbcd88e74 |
  | film_dev.json | 2f156d8d34bdd3eac31c196772156d36e017f8cb2255a2c312bd98e367ed363e |
  | film_test.json | 5ba2b79f9791bedbe5fe8a1bf5d7cbf277697743ee426ecd05459d9e28bcf21b |
  | music_train.json | d5835bb6a61da0067ab2d0dc3ba90059f07cfa781a70206a8266ecafd22641ad |
  | music_dev.json | ee34a4af167195db43fdcc910f8d125956222517f0572d248a069057493d557f |
  | music_test.json | 46f048c5db9dde45127cf6630b2fa06b685be369b71c5bc2dc3022d3f4bbad65 |
  | travel_train.json | b582a7171f8a7bb6b579bbf8d31a06cebd0966e520c95301a143b1629d20fb2f |
  | travel_dev.json | 8d1f31295888cf61eff398bab8aa042f7fccd04a088bd63c939f0b79625a9093 |
  | travel_test.json | 730426b0997b8375c8477c7e658c0ec645fced6ab8d546adda114b21cf30faac |

- **生成命令**(可复现,字节级确定性):

  ```bash
  python3 data/corpus/scripts/kdconv_to_sentences.py <kdconv数据目录> /tmp/sentences.txt
  cargo run --locked -p xhup-analyzer --bin corpus-stats -- \
    --input /tmp/sentences.txt --output data/corpus/conversation_kdconv.tsv
  ```

- **格式**:`词<TAB>计数<TAB>句子数<TAB>左上下文数<TAB>右上下文数`
  (结构见 xhup-analyzer `corpus` 模块;分词与生产码表同源)。
- **用途**:LexicalEvidence 的会话域信号(conversation_frequency /
  sentence_coverage / context_diversity),供 optimizer v2 目标函数;
  不直接参与 production 码表生成。
- **域偏差说明**:KdConv 是任务导向对话(电影/音乐/旅游推荐),
  「知道/什么」类引导词高频而「我们」类主语词低频;它提供会话域
  证据但不代表移动聊天全貌,后续以 PTT(转简)等来源补充。

## kdconv_bigram.tsv(会话域 bigram 转移,2026-09-15)

- **来源**:与 conversation_kdconv.tsv 完全相同的 92,558 句(相同
  SHA-256 pin)与同一最大匹配分词流(Segmenter::build)。
- **生成命令**(可复现,字节级确定性):

  ```bash
  python3 data/corpus/scripts/kdconv_to_sentences.py <kdconv数据目录> /tmp/sentences.txt
  cargo run --locked -p xhup-analyzer --bin corpus-stats -- \
    --input /tmp/sentences.txt --output /tmp/unigram.tsv \
    --bigram data/corpus/kdconv_bigram.tsv
  ```

- **格式**:`left<TAB>right<TAB>count`(按 (left, right) 字典序;
  句首/句尾边界 `<s>`/`</s>`;头部注释记录句数/token 数/转移对数)。
  232,987 个转移对,3.7MB。
- **消费方**:xhup-decoder `KdconvBigramScorer`(`kdconv-bigram/v1`,
  committed-context 评分,见 docs/architecture-v2.md scorer 契约节)。
- **许可**:派生自 Apache-2.0 的 KdConv,聚合计数,随源同许可。

## replay_fixture.txt(回放夹具,入库)

- **用途**:CI 语料回放回归门禁的输入(见 `.github/workflows/ci.yml`
  Rust job 的「语料回放回归门禁」步骤);全量 92,558 句太大不入库,
  夹具取去重后按 (出现次数降序, 句子字典序) 的前 2000 句。
- **生成命令**(可复现,字节级确定性;断句规则与 kdconv_to_sentences.py
  同源,从该模块导入):

  ```bash
  python3 data/corpus/scripts/kdconv_replay_fixture.py <kdconv数据目录> \
    data/corpus/replay_fixture.txt
  ```

- **行数**:2000;**SHA256**:
  `b5441ed3f92e685b217f8656f9f554410731267c593d247e3280901e52a98d30`。

## replay_baseline.json(回放基线,入库)

- **用途**:replay-bench `--baseline` 的断言基准。含四项指标
  (kspc / rank1_rate / top3_rate / fallback_rate)的基线值、
  允许回退容差与改善方向;rate 类以百分点(pp)计,kspc 为键/字。
- **再生成**(canonical 映射发生预期内变更时,需 PR 说明理由):

  ```bash
  cargo run --locked -p xhup-analyzer --bin replay-bench -- \
    --input data/corpus/replay_fixture.txt \
    --write-baseline data/corpus/replay_baseline.json
  ```

- **容差语义**:kspc +2%(相对基线值),rank1/top3 −0.5pp,
  fallback +1pp;改善方向不限。canonical v2 基线实测:KSPC 1.8971、
  rank1 96.9544%、rank≤3 99.9120%、兜底 37.0093%(夹具为高频句,
  与全量 92,558 句基线 2.068/95.3%/99.4%/37.9% 略有差异属预期)。

## conversation_ptt.tsv 与 ptt_bigram.tsv(PTT 八卦版会话域,2026-09-17)

Issue #83 §25 第 6 步「committed-context 第二证据源」的落地数据。KDConv
(92,558 句)覆盖不足:canonical v2 PRIMARY 的 5,308 个可二分歧义实例中,
仅 1.6% 有任何 in-path 转移证据。PTT 为**独立**会话域来源,语料量大 9.3 倍。

- **来源**:[zake7749/Gossiping-Chinese-Corpus](https://github.com/zake7749/Gossiping-Chinese-Corpus)
  `data/Gossiping-QA-Dataset.txt`(PTT 八卦版 2015–2017 文章标题 + 推文配对,
  每行 `标题<TAB>推文`,418,202 行)。
- **许可**:**Apache-2.0**(`LICENSE.ptt` 为 vendor 副本)。
- **版本 pin**:commit `65b7e3630a560223a2b4d702d78d120d5ff1e8dd`。
- **来源文件 SHA256**:
  `cf5ef0a931a8a14444a9854aa13cf6e7516b3ec4922b7cde5d45d13ed825ae79`。
- **预处理**:繁体→简体使用 OpenCC `t2s`(`opencc-python-reimplemented 0.1.7`);
  这是**生成端依赖**,运行时与构建端不依赖 OpenCC。
- **生成命令**(可复现,字节级确定性):

  ```bash
  python3 data/corpus/scripts/ptt_to_sentences.py <Gossiping-QA-Dataset.txt> /tmp/ptt_sentences.txt
  cargo run --locked -p xhup-analyzer --bin corpus-stats -- \
    --input /tmp/ptt_sentences.txt --output data/corpus/conversation_ptt.tsv \
    --bigram /tmp/ptt_bigram_full.tsv
  python3 data/corpus/scripts/ptt_bigram_prune.py /tmp/ptt_bigram_full.tsv \
    data/corpus/ptt_bigram.tsv 2   # count >= 2,见下「裁剪依据」
  ```

- **入库文件 SHA256**:
  - `ptt_bigram.tsv`:`ae1960ce5a3d1f4ddd459f1863c9de4264263748bb006b81f3e619fdf1f67e05`
  - `conversation_ptt.tsv`:`eb4a73b835c797623d973f184aa1163e23bad4ae5b2d83335df827d1a938f9bc`

- **规模**:861,745 句 / 5,965,161 token / 75,749 词;完整转移对 2,660,452。
- **`ptt_bigram.tsv` 裁剪依据**(count ≥ 2,604,171 行 ≈ 8.7 MB):
  对 5,308 个码表内歧义的证据覆盖率,按 PTT 侧计数阈值扫描(与 KDConv
  按**求和**合并):

  | PTT 阈值 | 有证据 | ≥2 | ≥5 | PTT 行数 | 体积 |
  |---|---|---|---|---|---|
  | 无 KDConv | 83 (1.56%) | 30 | 9 | — | 3.7 MB |
  | ≥1 | 287 (5.41%) | 103 | 27 | 2,660,452 | 41.0 MB |
  | **≥2** | **143 (2.69%)** | **97** | **27** | **604,171** | **8.7 MB** |
  | ≥3 | 117 (2.20%) | 66 | 27 | 318,681 | 4.5 MB |
  | ≥5 | 96 (1.81%) | 44 | 25 | 161,195 | 2.2 MB |

  取 ≥2:用 21% 体积保留 ≥2/≥5 两档几乎全部增益(97/103、27/27);
  降到 ≥3 会损失 32% 的 ≥2 证据。孤立转移对(count=1)在噪声大的 PTT
  上更可能是分词或转简伪影,故丢弃。
- **格式**:与 `kdconv_bigram.tsv` 完全一致(`left<TAB>right<TAB>count`,
  头部注释记来源/规模/阈值)。
- **域偏差**:PTT 为繁体问答 + 推文口语,噪声明显高于 KdConv(网语、错别字、
  推文格式残留);转简可能引入个别误转。用于**补充转移证据覆盖**,不作为
  唯一会话域代表。
- **用途**:`KdconvBigramScorer` 的第二证据源(与 KDConv 求和合并),提升
  committed-context 的真实消歧覆盖率。
