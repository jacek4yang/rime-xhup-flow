#!/usr/bin/env python3
"""PTT 八卦版问答语料 → 纯文本句子(每行一句,简体),供 corpus-stats 统计。

用法: python3 ptt_to_sentences.py <Gossiping-QA-Dataset.txt> <输出 sentences.txt>

来源:zake7749/Gossiping-Chinese-Corpus `data/Gossiping-QA-Dataset.txt`
(Apache-2.0;2015–2017 八卦版文章标题 + 推文配对,每行 `标题<TAB>推文`)。
许可与用途决策见 docs/data-pipeline.md §1.1 与 data/corpus/README.md。

只输出发言文本(派生统计的中间产物,本地使用,**不入库**);聚合统计由
corpus-stats 产出,入库的只有聚合计数。

确定性:
- 逐行顺序读取,不做任何并行或采样;
- 繁体→简体使用 OpenCC `t2s`(版本需在 provenance 中记录,不同版本输出
  可能不同,故生成时须固定);
- 断句规则与 kdconv_to_sentences.py 同源(同一 SENT_SPLIT 正则语义)。

注意:PTT 为**繁体**来源且噪声较高(推文口语、网语、错别字)。转简后并入
会话域统计,用于提升词级转移证据覆盖;不做任何词级人工修正。
"""

import re
import sys
from pathlib import Path

try:
    from opencc import OpenCC
except ImportError:  # pragma: no cover - 环境缺依赖时给出明确指引
    print(
        "缺少 opencc-python-reimplemented;请先 `pip install "
        "opencc-python-reimplemented`(生成端依赖,运行时/构建端不依赖)",
        file=sys.stderr,
    )
    sys.exit(3)

# 与 kdconv_to_sentences.py 保持同一断句语义(中文/英文句读 + 换行)。
SENT_SPLIT = re.compile(r"[。!?！？;；\n\r]+")

# 每行固定两列:标题<TAB>推文。
EXPECTED_COLUMNS = 2


def main() -> int:
    if len(sys.argv) != 3:
        print(__doc__, file=sys.stderr)
        return 2
    src, out_path = Path(sys.argv[1]), Path(sys.argv[2])
    if not src.is_file():
        print(f"缺失输入: {src}", file=sys.stderr)
        return 1

    converter = OpenCC("t2s")
    count = 0
    skipped = 0
    with src.open(encoding="utf-8") as fh, out_path.open(
        "w", encoding="utf-8", newline="\n"
    ) as out:
        for line in fh:
            line = line.rstrip("\n")
            if not line:
                continue
            fields = line.split("\t")
            if len(fields) != EXPECTED_COLUMNS:
                # 结构不符的行显式计数跳过(不猜测),便于审计输入质量。
                skipped += 1
                continue
            for field in fields:
                # 先转简,再断句,保证输出与 kdconv 流同域同形。
                simplified = converter.convert(field)
                for sentence in SENT_SPLIT.split(simplified):
                    sentence = sentence.strip()
                    if sentence:
                        out.write(sentence + "\n")
                        count += 1
    print(f"sentences={count} skipped_lines={skipped}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
