#!/usr/bin/env python3
"""KdConv 对话语料 → 纯文本句子(每行一句),供 corpus-stats 统计。

用法: python3 kdconv_to_sentences.py <输入目录(含 film/music/travel 的
train/dev/test JSON)> <输出 sentences.txt>

只输出发言文本(派生统计的中间产物,本地使用,不入库);聚合统计由
corpus-stats 产出。确定性:按 (域名, split, 对话序, 发言序) 固定遍历。
"""

import json
import re
import sys
from pathlib import Path

DOMAINS = ("film", "music", "travel")
SPLITS = ("train", "dev", "test")

# 粗略断句:中文/英文句读符号;保留原句不再清洗(统计层处理边界字符)。
SENT_SPLIT = re.compile(r"[。!?!?;;\n\r]+")


def utterances(path: Path):
    with path.open(encoding="utf-8") as fh:
        dialogues = json.load(fh)
    for dialogue in dialogues:
        messages = dialogue.get("messages") or dialogue.get("conversation") or []
        for message in messages:
            if isinstance(message, dict):
                # KdConv 实际结构:messages[].message(发言文本)。
                text = (
                    message.get("message")
                    or message.get("utterance")
                    or message.get("text")
                    or ""
                )
            else:
                text = str(message)
            yield text


def main() -> int:
    if len(sys.argv) != 3:
        print(__doc__, file=sys.stderr)
        return 2
    src_dir, out_path = Path(sys.argv[1]), Path(sys.argv[2])
    count = 0
    with out_path.open("w", encoding="utf-8", newline="\n") as out:
        for domain in DOMAINS:
            for split in SPLITS:
                path = src_dir / f"{domain}_{split}.json"
                if not path.is_file():
                    print(f"缺失输入: {path}", file=sys.stderr)
                    return 1
                for text in utterances(path):
                    for sentence in SENT_SPLIT.split(text):
                        sentence = sentence.strip()
                        if sentence:
                            out.write(sentence + "\n")
                            count += 1
    print(f"句子数: {count} -> {out_path}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
