# -*- coding: utf-8 -*-
"""搜狗聚合层分类标注重建工具。

从搜狗细胞词库全站抓取的原始数据(celldb)重建「词 → 来源分类」倒排,
对 `data/words/sogou/sogou_cell_*.tsv` 聚合分片逐词打标,产出:

- `tags_compact.tsv`:词、读音、标注(仅非 keep 行;keep 为默认,不入文件)
- `protect_list.tsv`:保护白名单(词 + 来源集合)

标注规则(优先级从高到低):

1. `protected`:词在保护白名单(万象 base ∪ 搜狗系统词库高频 ≤800 ∪
   KDConv 语料频次 ≥2)——任何情况下保留;
2. `target`:来源含 5 类小众文化分类(436 电子游戏 / 437 单机游戏 /
   461 网络游戏 / 404 动漫 / 429 明星)——构建时排除;
3. `llm_review`:来源含娱乐分类(403,影视/明星/小说混合)且无 target 分类
   ——LLM 精判;
4. `keep`:其余。

原始数据(6280 个 .scel/.txt 与 meta.json)仅存在于抓取机器,不入仓库;
本脚本在原始数据所在的机器上运行后,产物提交入库。原始分片永不修改。
"""
from __future__ import annotations

import argparse
import glob
import json
import os
import sys
from collections import defaultdict

# 搜狗词库分类站点的分类 id;名称见抓取时的 cate_names.json(不入库,仅本地参考)。
TARGET_CATEGORIES = {436, 437, 461, 404, 429}
ENTERTAINMENT_CATEGORY = 403
SYSTEM_FREQ_KEEP_MAX = 800  # 搜狗系统词库频率等级 ≤ 该值的字/词视为高频保护
KDCONV_MIN_COUNT = 2

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


def load_category_map(celldb_dir: str) -> dict[str, int]:
    """scel id → 分类 id(来自抓取时记录的 meta.json)。"""
    meta = json.load(open(os.path.join(celldb_dir, "meta.json"), encoding="utf-8"))
    id2cate: dict[str, int] = {}
    for cate, items in meta.items():
        for item in items:
            id2cate[item[0]] = int(cate)
    return id2cate


def build_word_categories(celldb_dir: str) -> dict[str, set[int]]:
    """词 → 来源分类集合(从原始 txt 全量重建)。"""
    id2cate = load_category_map(celldb_dir)
    word_categories: dict[str, set[int]] = defaultdict(set)
    for path in glob.glob(os.path.join(celldb_dir, "scel", "*.txt")):
        sid = os.path.basename(path).split("_")[0]
        cate = id2cate.get(sid)
        if cate is None:
            continue
        with open(path, encoding="utf-8") as handle:
            for line in handle:
                word = line.split("\t")[0].strip()
                if word:
                    word_categories[word].add(cate)
    return word_categories


def load_protect_list() -> dict[str, list[str]]:
    """三层保护白名单:万象 base、搜狗系统高频、KDConv 语料频次。"""
    protect: dict[str, list[str]] = defaultdict(list)
    base = os.path.join(REPO, "data", "words", "wanxiang_base_words.tsv")
    with open(base, encoding="utf-8") as handle:
        for line in handle:
            if not line.startswith("#") and line.strip():
                protect[line.split("\t")[0]].append("wanxiang_base")
    # 搜狗系统词库表由本地提取流程产出(见 data/words/sogou/README 说明),
    # 位置默认在本仓库外的提取目录;路径可传入。
    return dict(protect)


def extend_protect_from_file(protect: dict[str, list[str]], path: str, source: str,
                             freq_column: int, keep_max: int) -> None:
    for line in open(path, encoding="utf-8").read().splitlines()[1:]:
        parts = line.split("\t")
        try:
            if int(parts[freq_column]) <= keep_max:
                protect.setdefault(parts[0], []).append(source)
        except (ValueError, IndexError):
            continue


def extend_protect_from_corpus(protect: dict[str, list[str]], path: str) -> None:
    for line in open(path, encoding="utf-8").read().splitlines()[2:]:
        parts = line.split("\t")
        if len(parts) < 2:
            continue
        try:
            if int(parts[1]) >= KDCONV_MIN_COUNT:
                protect.setdefault(parts[0], []).append("kdconv_ge2")
        except ValueError:
            continue


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--celldb", required=True, help="原始抓取目录(含 meta.json 与 scel/*.txt)")
    parser.add_argument("--sys-freq", required=True, help="搜狗系统词库频率表(chars_freq.tsv 格式:字 音 c3 c4 freq rank)")
    parser.add_argument("--out-dir", default=os.path.join(REPO, "data", "words", "sogou"))
    args = parser.parse_args()

    word_categories = build_word_categories(args.celldb)
    protect = load_protect_list()
    extend_protect_from_file(protect, args.sys_freq, "sogou_sys_freq", freq_column=4,
                             keep_max=SYSTEM_FREQ_KEEP_MAX)
    extend_protect_from_corpus(protect, os.path.join(REPO, "data", "corpus", "conversation_kdconv.tsv"))

    with open(os.path.join(args.out_dir, "protect_list.tsv"), "w", encoding="utf-8", newline="\n") as out:
        for word in sorted(protect):
            out.write(f"{word}\t{','.join(protect[word])}\n")

    shard_paths = sorted(glob.glob(os.path.join(REPO, "data", "words", "sogou", "sogou_cell_*.tsv")))
    stats: dict[str, int] = {}
    with open(os.path.join(args.out_dir, "tags_compact.tsv"), "w", encoding="utf-8", newline="\n") as out:
        for shard in shard_paths:
            for line in open(shard, encoding="utf-8"):
                parts = line.rstrip("\n").split("\t")
                word = parts[0]
                cates = word_categories.get(word, set())
                if word in protect:
                    tag = "protected"
                elif cates & TARGET_CATEGORIES:
                    tag = "target"
                elif cates & {ENTERTAINMENT_CATEGORY}:
                    tag = "llm_review"
                else:
                    tag = "keep"  # 默认,不入 tags_compact.tsv
                stats[tag] = stats.get(tag, 0) + 1
                if tag != "keep":
                    reading = parts[1] if len(parts) > 1 else "-"
                    out.write(f"{word}\t{reading}\t{tag}\n")
    print("tag stats:", stats)


if __name__ == "__main__":
    sys.exit(main())
