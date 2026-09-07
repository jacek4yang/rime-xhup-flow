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
