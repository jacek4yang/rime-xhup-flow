#!/usr/bin/env python3
"""KdConv 语料 → 回放夹具(去重后高频 top-N 句,每行一句,入库)。

用法: python3 kdconv_replay_fixture.py <输入目录(含 film/music/travel 的
train/dev/test JSON,与 kdconv_to_sentences.py 相同布局)> <输出文件> [N]

句子定义与断句规则沿用 kdconv_to_sentences.py(从该模块导入,单一来源)。
统计全部句子的出现次数,去重后按 (出现次数降序, 句子字典序) 取前 N 句
(默认 2000)。输出 UTF-8、LF、每行一句,字节级确定性。
"""

import sys
from collections import Counter
from pathlib import Path

from kdconv_to_sentences import DOMAINS, SPLITS, SENT_SPLIT, utterances

DEFAULT_TOP_N = 2000


def sentences(src_dir: Path):
    for domain in DOMAINS:
        for split in SPLITS:
            path = src_dir / f"{domain}_{split}.json"
            if not path.is_file():
                print(f"缺失输入: {path}", file=sys.stderr)
                sys.exit(1)
            for text in utterances(path):
                for sentence in SENT_SPLIT.split(text):
                    sentence = sentence.strip()
                    if sentence:
                        yield sentence


def main() -> int:
    if len(sys.argv) not in (3, 4):
        print(__doc__, file=sys.stderr)
        return 2
    src_dir, out_path = Path(sys.argv[1]), Path(sys.argv[2])
    top_n = int(sys.argv[3]) if len(sys.argv) == 4 else DEFAULT_TOP_N

    counts = Counter(sentences(src_dir))
    top = sorted(counts.items(), key=lambda kv: (-kv[1], kv[0]))[:top_n]
    with out_path.open("w", encoding="utf-8", newline="\n") as out:
        for sentence, _ in top:
            out.write(sentence + "\n")
    print(
        f"总句数: {sum(counts.values())}  去重: {len(counts)}  "
        f"夹具: {len(top)} -> {out_path}"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
