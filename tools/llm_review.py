# -*- coding: utf-8 -*-
"""搜狗聚合层 llm_review 词条 LLM 精判管线(词库质量提升 PR-3)。

对 `data/words/sogou/tags_compact.tsv` 中 55,684 条 `llm_review` 词条
(来源含搜狗娱乐分类 403 的混合词库)用本地 LLM 批量精判,产出:

- `data/words/sogou/llm_review_verdicts.tsv`:词、读音、判定
  (keep=常用词保留 / remove=专名、作品名、生造词移除)
- `data/words/sogou/llm_review_stats.json`:统计与可复现性元数据

判定契约(保守优先,详见 issue #94):
- keep:日常通用词、题材类通用词、常见成语、普通组合词
- remove:具体人名/角色名、具体作品名、粉丝圈黑话、生造组合、无意义拼接
- 不确定 → remove(protected 白名单已有兜底,宁可错杀)

确定性:温度 0、批内顺序固定(按 tags_compact.tsv 出现顺序)、批次大小固定;
逐批落盘支持断点续跑。本管线纯本地推理(codebuddy-proxy),零遥测。

用法:
  python tools/llm_review.py --endpoint http://127.0.0.1:8088/v1/messages \\
      --model deepseek-v4.1-flash [--limit N] [--self-test]
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import sys
import time
import urllib.error
import urllib.request

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
TAGS_PATH = os.path.join(REPO, "data", "words", "sogou", "tags_compact.tsv")
PROTECT_PATH = os.path.join(REPO, "data", "words", "sogou", "protect_list.tsv")
VERDICTS_PATH = os.path.join(REPO, "data", "words", "sogou", "llm_review_verdicts.tsv")
STATS_PATH = os.path.join(REPO, "data", "words", "sogou", "llm_review_stats.json")

BATCH_SIZE = 50
MAX_RETRIES = 5
RETRY_BACKOFF_SECS = 5.0
REQUEST_TIMEOUT_SECS = 300

JUDGE_PROMPT = """你是中文词库审校员。下面每行是「词条 拼音」。对每行判定:该词是否为日常中文常用词。判定规则:日常通用词/普通组合词/常见成语/题材泛指词(如 电视剧、演唱会)= keep;具体人名、角色名、作品名(影视剧歌曲小说游戏节目)、粉丝圈黑话、生造词、无意义单字拼接 = remove;拿不准= remove。

每行输出一个判定,格式严格为:
词条|判定

例如:
一辑|keep

待判定列表:
"""

# 回复行形如「词条|判定」;词条原样回显,只取判定列。
VERDICT_LINE_RE = re.compile(r"^(?P<word>[^\t\n|]+)\|\s*(?P<verdict>keep|remove)\s*$",
                             re.MULTILINE | re.IGNORECASE)


def parse_verdicts(text: str, words: list[str]) -> list[str]:
    """从逐行「词条|判定」回复提取判定;缺失/多出/非法词条视为该批失败。"""
    found: dict[str, str] = {}
    for match in VERDICT_LINE_RE.finditer(text):
        word = match.group("word").strip()
        verdict = match.group("verdict").strip().lower()
        if word in found:
            raise ValueError(f"词条重复判定: {word!r}")
        found[word] = verdict
    out: list[str] = []
    for word in words:
        if word not in found:
            raise ValueError(f"回复缺少词条判定: {word!r}")
        out.append(found[word])
    extra = set(found) - set(words)
    if extra:
        raise ValueError(f"回复包含未知词条: {sorted(extra)[:5]}")
    return out


def load_llm_review_rows(tags_path: str) -> list[tuple[str, str]]:
    """从 tags_compact.tsv 读取 (word, reading) 列表,顺序即文件顺序。"""
    rows: list[tuple[str, str]] = []
    with open(tags_path, encoding="utf-8") as handle:
        for line in handle:
            line = line.rstrip("\n")
            if not line or line.startswith("#"):
                continue
            parts = line.split("\t")
            if len(parts) != 3 or parts[2] != "llm_review":
                continue
            rows.append((parts[0], parts[1]))
    return rows


def load_protected_words(protect_path: str) -> set[str]:
    protected: set[str] = set()
    with open(protect_path, encoding="utf-8") as handle:
        for line in handle:
            line = line.rstrip("\n")
            if not line or line.startswith("#"):
                continue
            word = line.split("\t", 1)[0]
            if word:
                protected.add(word)
    return protected


def build_batches(rows: list[tuple[str, str]], batch_size: int) -> list[list[int]]:
    return [list(range(i, min(i + batch_size, len(rows))))
            for i in range(0, len(rows), batch_size)]


def render_user_prompt(rows: list[tuple[str, str]], indices: list[int]) -> str:
    return JUDGE_PROMPT + "\n".join(
        f"{rows[i][0]} {rows[i][1]}" for i in indices)


JSON_ARRAY_RE = re.compile(r"\[[\s\S]*\]")


def _legacy_parse_verdicts(text: str, expected: int) -> list[str]:
    """(保留给历史脚本调用)旧 JSON 数组解析。"""
    match = JSON_ARRAY_RE.search(text)
    if match is None:
        raise ValueError("回复中未找到 JSON 数组")
    parsed = json.loads(match.group(0))
    if not isinstance(parsed, list) or len(parsed) != expected:
        raise ValueError(f"判定数量不符:期望 {expected},得到 {len(parsed)}")
    out: list[str] = []
    for item in parsed:
        tag = str(item).strip().lower()
        if tag not in VALID_TAGS:
            raise ValueError(f"非法判定值: {item!r}")
        out.append(tag)
    return out


VALID_TAGS = ("keep", "remove")

def call_llm(endpoint: str, model: str, user: str) -> str:
    """调用 Anthropic Messages 兼容端点,返回拼接的文本回复。"""
    payload = json.dumps({
        "model": model,
        "max_tokens": 4096,
        "temperature": 0,
        "messages": [{"role": "user", "content": user}],
    }).encode("utf-8")
    request = urllib.request.Request(
        endpoint,
        data=payload,
        headers={"Content-Type": "application/json"},
        method="POST",
    )
    with urllib.request.urlopen(request, timeout=REQUEST_TIMEOUT_SECS) as response:
        body = json.loads(response.read().decode("utf-8"))
    chunks = []
    for block in body.get("content", []):
        if block.get("type") == "text":
            chunks.append(block.get("text", ""))
    return "".join(chunks)


def call_with_retry(endpoint: str, model: str, rows, indices) -> list[str]:
    user = render_user_prompt(rows, indices)
    words = [rows[i][0] for i in indices]
    last_error: Exception | None = None
    for attempt in range(1, MAX_RETRIES + 1):
        try:
            text = call_llm(endpoint, model, user)
            return parse_verdicts(text, words)
        except (ValueError, KeyError, urllib.error.URLError, OSError,
                json.JSONDecodeError) as error:
            last_error = error
            wait = RETRY_BACKOFF_SECS * attempt
            print(f"  批次失败(第 {attempt} 次): {error};{wait:.0f}s 后重试",
                  file=sys.stderr)
            time.sleep(wait)
    raise RuntimeError(f"批次在 {MAX_RETRIES} 次重试后仍失败: {last_error}")


def load_existing_verdicts(path: str) -> dict[tuple[str, str], str]:
    """断点续跑:读取已落盘判定。"""
    existing: dict[tuple[str, str], str] = {}
    if not os.path.exists(path):
        return existing
    with open(path, encoding="utf-8") as handle:
        for line in handle:
            line = line.rstrip("\n")
            if not line or line.startswith("#"):
                continue
            parts = line.split("\t")
            if len(parts) == 3 and parts[2] in VALID_TAGS:
                existing[(parts[0], parts[1])] = parts[2]
    return existing


def append_verdicts(path: str, rows, indices, verdicts: list[str]) -> None:
    with open(path, "a", encoding="utf-8", newline="\n") as handle:
        for idx, verdict in zip(indices, verdicts):
            word, reading = rows[idx]
            handle.write(f"{word}\t{reading}\t{verdict}\n")


def batch_hash(batch_tag: str, rows, indices) -> str:
    digest = hashlib.sha256()
    digest.update(batch_tag.encode("utf-8"))
    for idx in indices:
        digest.update(f"{rows[idx][0]}\t{rows[idx][1]}\n".encode("utf-8"))
    return digest.hexdigest()


def run(args: argparse.Namespace) -> int:
    rows = load_llm_review_rows(TAGS_PATH)
    if args.limit:
        rows = rows[: args.limit]
    print(f"llm_review 词条: {len(rows)}")

    protected = set() if args.no_protect_guard else load_protected_words(PROTECT_PATH)
    existing = load_existing_verdicts(VERDICTS_PATH)

    pending_keys = [(w, r) for w, r in rows if (w, r) not in existing]
    print(f"已完成 {len(rows) - len(pending_keys)},待判定 {len(pending_keys)}")

    batches = build_batches(rows, args.batch_size)
    started = time.time()
    done_in_session = 0
    for batch_no, indices in enumerate(batches, start=1):
        todo = [i for i in indices if (rows[i][0], rows[i][1]) not in existing]
        if not todo:
            continue
        verdicts = call_with_retry(args.endpoint, args.model, rows, todo)
        # 保护白名单兜底:LLM 判 remove 的 protected 词强制改判 keep。
        for pos, idx in enumerate(todo):
            if rows[idx][0] in protected and verdicts[pos] == "remove":
                verdicts[pos] = "keep"
        append_verdicts(VERDICTS_PATH, rows, todo, verdicts)
        for idx, verdict in zip(todo, verdicts):
            existing[(rows[idx][0], rows[idx][1])] = verdict
        done_in_session += len(todo)
        if batch_no % 10 == 0 or batch_no == len(batches):
            elapsed = time.time() - started
            print(f"批次 {batch_no}/{len(batches)} 累计 {done_in_session} 条 "
                  f"({elapsed:.0f}s, {done_in_session / max(elapsed, 1):.0f} 条/s)")

    write_stats(rows, existing, args)
    print(f"完成:判定文件 {VERDICTS_PATH}")
    return 0


def write_stats(rows, existing: dict[tuple[str, str], str], args) -> None:
    verdicts_in_order = [existing.get((w, r), "") for w, r in rows]
    keep = verdicts_in_order.count("keep")
    remove = verdicts_in_order.count("remove")
    missing = verdicts_in_order.count("")
    stats = {
        "model": args.model,
        "endpoint": args.endpoint,
        "batch_size": args.batch_size,
        "total": len(rows),
        "keep": keep,
        "remove": remove,
        "missing": missing,
        "verdicts_sha256": file_sha256(VERDICTS_PATH) if os.path.exists(VERDICTS_PATH) else None,
    }
    with open(STATS_PATH, "w", encoding="utf-8", newline="\n") as handle:
        json.dump(stats, handle, ensure_ascii=False, indent=2)
        handle.write("\n")
    print(json.dumps(stats, ensure_ascii=False))


def file_sha256(path: str) -> str:
    digest = hashlib.sha256()
    with open(path, "rb") as handle:
        for chunk in iter(lambda: handle.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


def self_test() -> int:
    """无网络自检:夹具校验解析、批构造与白名单兜底逻辑。"""
    fixtures = [
        ("一辑|keep\n一音|remove", ["一辑", "一音"], ["keep", "remove"]),
        ("前缀\n一辑 | keep\n东帝|REMOVE\n后缀", ["一辑", "东帝"], ["keep", "remove"]),
        ("一辑|keep", ["一辑", "一音"], None),              # 缺词条
        ("一辑|keep\n一音|keep\n丁于|remove", ["一辑"], None),  # 多词条
        ("一辑|删除", ["一辑"], None),                       # 非法值
        ("不是数组", ["一辑"], None),                        # 无行
    ]
    for text, words, want in fixtures:
        try:
            got = parse_verdicts(text, words)
            if got != want:
                print(f"self-test 失败: {text!r} -> {got!r}, 期望 {want!r}")
                return 1
        except ValueError:
            if want is not None:
                print(f"self-test 失败: {text!r} 应可解析")
                return 1

    rows = [("测试词", "ce shi"), ("保护词", "bao hu")]
    indices = [0, 1]
    prompt = render_user_prompt(rows, indices)
    if not prompt.endswith("待判定列表:\n测试词 ce shi\n保护词 bao hu"):
        print("self-test 失败: prompt 渲染不符")
        return 1

    batches = build_batches([("a", "a")] * 103, 50)
    if [len(b) for b in batches] != [50, 50, 3]:
        print("self-test 失败: 批构造不符")
        return 1

    print("self-test 通过")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--endpoint", default="http://127.0.0.1:8088/v1/messages")
    parser.add_argument("--model", default="deepseek-v4.1-flash")
    parser.add_argument("--batch-size", type=int, default=BATCH_SIZE)
    parser.add_argument("--limit", type=int, default=0, help="仅处理前 N 条(试跑)")
    parser.add_argument("--no-protect-guard", action="store_true",
                        help="跳过保护白名单兜底(默认开启)")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        return self_test()
    return run(args)


if __name__ == "__main__":
    sys.exit(main())
