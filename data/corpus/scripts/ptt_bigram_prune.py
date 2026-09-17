#!/usr/bin/env python3
"""PTT bigram 裁剪:count 阈值过滤 + 确定性排序 + provenance 头部。

用法: python3 ptt_bigram_prune.py <完整 bigram TSV> <输出 TSV> [min_count]

背景:corpus-stats 产出的完整 PTT bigram 有 2,660,452 个转移对(41 MB),
不宜入库。裁剪依据与覆盖率权衡见 data/corpus/README.md;默认阈值 2。

确定性:输入按 (left, right) 字典序读出后重排写出,输出字节稳定。
"""

import sys
from pathlib import Path

DEFAULT_MIN_COUNT = 2

# 与其他语料产物一致的来源 pin;变更来源必须同步更新此常量与 README。
SOURCE = "Gossiping-QA-Dataset.txt"
SOURCE_SHA256 = "cf5ef0a931a8a14444a9854aa13cf6e7516b3ec4922b7cde5d45d13ed825ae79"


def main() -> int:
    if len(sys.argv) not in (3, 4):
        print(__doc__, file=sys.stderr)
        return 2
    src, out_path = Path(sys.argv[1]), Path(sys.argv[2])
    min_count = int(sys.argv[3]) if len(sys.argv) == 4 else DEFAULT_MIN_COUNT
    if min_count < 1:
        print("min_count 必须 >= 1", file=sys.stderr)
        return 2

    header: list[str] = []
    rows: list[tuple[str, str, int]] = []
    total = 0
    with src.open(encoding="utf-8") as fh:
        for line in fh:
            line = line.rstrip("\n")
            if not line:
                continue
            if line.startswith("#"):
                header.append(line)
                continue
            if line == "left\tright\tcount":
                continue
            fields = line.split("\t")
            if len(fields) != 3:
                print(f"非法行(应为三列): {line!r}", file=sys.stderr)
                return 1
            try:
                count = int(fields[2])
            except ValueError:
                print(f"非法计数: {line!r}", file=sys.stderr)
                return 1
            total += 1
            if count >= min_count:
                rows.append((fields[0], fields[1], count))

    rows.sort(key=lambda row: (row[0], row[1]))
    with out_path.open("w", encoding="utf-8", newline="\n") as out:
        out.write(f"# ptt-bigram/v1 source={SOURCE} sha256={SOURCE_SHA256}\n")
        out.write(
            f"# pairs_total={total} min_count={min_count} pairs={len(rows)}\n"
        )
        out.write("left\tright\tcount\n")
        for left, right, count in rows:
            out.write(f"{left}\t{right}\t{count}\n")
    print(f"pairs_total={total} kept={len(rows)} min_count={min_count}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
