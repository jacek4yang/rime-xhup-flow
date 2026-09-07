# 数据管线:语料证据的来源、许可与导入(Input Model v2)

状态:**已定来源,导入实施中**。调研结论见本文件;运行时分层见
docs/lua-runtime.md;优化目标见 docs/input-model-v2.md。

## 0. 原则

- canonical 语义数据(`data/`)是唯一真源;语料证据是**派生统计**,
  只入聚合计数,原句/原文绝不入库;
- 每个来源必须:许可证允许再分发派生物、版本可 pin(仓库+提交+文件
  哈希)、导入过程可复现、署名义务在 NOTICE 中履行;
- 证据缺失显式可见(EvidenceCoverage 审计),缺失 ≠ 零。

## 1. 来源清单(2026-09 调研结论)

### 1.1 Canonical 证据层(派生统计可入库)

| 来源 | 许可 | 角色 | 状态 |
|---|---|---|---|
| amzxyz/rime-wanxiang `dicts/jichu.dict.yaml` | CC BY 4.0 | unigram 主证据(词频) | **已 pin 入库**(data/words,data/frequency;blob SHA 见各 README) |
| amzxyz/RIME-LMDG releases | CC BY 4.0 | 扩展词库/语法模型来源 | 可扩;语法模型策略见 lua-runtime §5 |
| thunlp/THUOCL | MIT(GitHub 副本) | 分类域 DF 分布(IT/财经/法律等 12 类) | 待导入;NOTICE 加学术引用声明(官网旧条款与 MIT 并存,按 MIT 执行) |
| fxsjy/jieba `dict.txt` | MIT | unigram 交叉验证(书面语偏向,年份偏旧) | 待导入 |
| thu-coai/KdConv | Apache-2.0 | 会话域(4500 对话/8.6 万发言,任务型) | 待导入 |
| zake7749/Gossiping-Chinese-Corpus(PTT) | Apache-2.0 | 会话域(繁体问答,OpenCC 转简后入派生统计) | 待导入 |
| WenetSpeech 转写文本 | 仓库 Apache-2.0;数据集 CC BY 4.0 待官网坐实 | 口语域 | 坐实许可前不动 |

### 1.2 仅本地参考(不入库、不入派生物)

| 来源 | 许可 | 用途 |
|---|---|---|
| lotem/rime-octagram-data essay(.gram) | LGPL-3.0 | bigram sanity check;离线解码可行(自写 darts walker,BSD 源码可参考),产出是剪枝后 log 概率而非原始计数 |
| LCCC(thu-coai/CDial-GPT) | 仓库 MIT,但上游微博/豆瓣未清理 | 本地参考;是否允许「不可逆词形计数聚合」入库待所有者拍板,默认不 |

### 1.3 红线(不得入库、不得派生入库)

- 任何搜狗系词库(SogouWan/scel,用户协议限非商业);
- NLPCC / SMP 竞赛数据(协议限研究用途);
- 豆瓣语料、chinese-chatbot-corpus 合集(无许可);
- essay .gram 的派生数据文件(LGPL);
- 中文维基派生 n-gram(CC BY-SA 4.0 传染性)不得混入 MIT 文件树;
  若需要百科域,自建并单独标记许可。

## 2. 管线形态

```text
来源(本地缓存,不入库)
  -> 预处理脚本(去标记、转简、断句;脚本入库)
  -> corpus-stats(crates/xhup-analyzer,每行一句 → 派生统计)
  -> data/corpus/<domain>.tsv(聚合计数,入库;README 记 provenance)
  -> LexicalEvidence 语料信号(sentence_coverage / context_diversity /
     conversation_frequency / …)
  -> optimizer v2 目标函数
```

- 域(domain)由语料文件归属决定:conversation / formal / technical 等,
  文件名即域标签,统计按域分文件产出;
- 分词与生产码表同源(canonical 词表最大匹配,见 corpus 模块);
- 派生 TSV 的再生成必须字节级一致(确定性门禁)。

## 3. 验收

- 每个入库统计文件:来源、版本、输入哈希、生成命令在 README 可复查;
- EvidenceCoverage 审计随信号落地更新(绊线设计);
- top-100/top-1000 常用词门禁(已在 PR #51)与语料证据交叉一致:
  高频词必须在会话域证据中可见,否则标记数据源偏差。
