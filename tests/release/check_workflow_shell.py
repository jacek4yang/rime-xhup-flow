#!/usr/bin/env python3
"""守卫:`.github/workflows/*.yml` 中每个 `run:` 脚本块的 shell 语法必须可解析。

背景(真实回归):`xhup-flow-rc-release.yml` 的「组装发布说明」步骤在 stable
分支里漏了一个闭合双引号(`banner="…` 由下一行的 `acceptance="` 提前闭合),
使该步骤的整段脚本在 `bash -n` 层面即不可解析。bash 在执行前解析整段脚本,
因此**与运行分支无关**:RC 发布(publish=true)在「创建草稿 Release」阶段直接
失败(`unexpected EOF while looking for matching '"'`),2.0.0-rc.2 草稿无法创建。

本脚本把每个 `run:` 块按 YAML 块标量规则还原为脚本,并用 `bash -n` 逐一解析;
毫秒级、无第三方依赖,可在本地与 CI 拦截该类缺陷。非 bash 块按 GitHub
Actions 的 shell 解析规则跳过:显式 `shell:`(pwsh/cmd/python…)或
windows runner 上的默认 pwsh。

用法: python3 tests/release/check_workflow_shell.py [仓库根目录]
"""

from __future__ import annotations

import os
import re
import subprocess
import sys

RUN_BLOCK_RE = re.compile(r"^(?P<indent>[ ]*)run:[ ]*(?P<indicator>[|>][-+]?)?[ ]*$")
RUNS_ON_RE = re.compile(r"^[ ]*runs-on:[ ]*(?P<value>.+?)[ ]*$")
SHELL_RE = re.compile(r"^(?P<indent>[ ]*)shell:[ ]*(?P<shell>[A-Za-z0-9_./-]+)[ ]*$")
BASH_SHELLS = ("bash", "sh")


def indent_of(line: str) -> int:
    return len(line) - len(line.lstrip(" "))


def block_scalar(lines: list[str], start: int, base_indent: int) -> tuple[str, int]:
    """还原 `run:` 之后的块标量内容(返回脚本与块结束行号)。

    YAML 规则:块内容 = 缩进大于 `run:` 的连续行;整体按最小缩进去缩进。
    """
    body: list[str] = []
    index = start
    while index < len(lines):
        line = lines[index]
        if line.strip() == "":
            body.append("")
            index += 1
            continue
        if indent_of(line) <= base_indent:
            break
        body.append(line)
        index += 1
    non_empty = [indent_of(line) for line in body if line.strip()]
    cut = min(non_empty) if non_empty else 0
    dedented = [line[cut:] if len(line) >= cut else line for line in body]
    return "\n".join(dedented), index


def unfold(script: str) -> str:
    """`>` 折叠:连续非空行以空格连接,空行保留为换行分隔。"""
    folded: list[str] = []
    group: list[str] = []
    for line in script.split("\n"):
        if line.strip():
            group.append(line)
            continue
        if group:
            folded.append(" ".join(group))
            group = []
        folded.append("")
    if group:
        folded.append(" ".join(group))
    return "\n".join(folded)


def run_blocks(lines: list[str]) -> list[tuple[int, str, str]]:
    """返回 [(run: 行号(0 基), 指示符, 还原后的脚本), ...]。"""
    found: list[tuple[int, str, str]] = []
    index = 0
    while index < len(lines):
        match = RUN_BLOCK_RE.match(lines[index])
        if not match:
            index += 1
            continue
        run_line = index
        base = indent_of(lines[index])
        indicator = match.group("indicator") or "|"
        script, index = block_scalar(lines, index + 1, base)
        if indicator.startswith(">"):
            script = unfold(script)
        found.append((run_line, indicator, script))
    return found


def declared_shell(lines: list[str], run_line: int) -> str | None:
    """返回与 `run:` 同级缩进的最近 `shell:` 值(同一 step 映射内)。"""
    base = indent_of(lines[run_line])
    for index in range(run_line - 1, -1, -1):
        line = lines[index]
        if not line.strip():
            continue
        match = SHELL_RE.match(line)
        if match and indent_of(line) == base:
            return match.group("shell").lower()
        if indent_of(line) < base:
            break
    return None


def runs_on_windows(lines: list[str], run_line: int) -> bool:
    """`run:` 所在 job 是否跑在 windows runner(其默认 shell 是 pwsh)。"""
    for index in range(run_line, -1, -1):
        match = RUNS_ON_RE.match(lines[index])
        if match:
            return "windows" in match.group("value").lower()
        if lines[index].startswith("jobs:"):
            break
    return False


def is_bash_block(lines: list[str], run_line: int) -> bool:
    declared = declared_shell(lines, run_line)
    if declared:
        return declared in BASH_SHELLS
    return not runs_on_windows(lines, run_line)


def check(workflows_dir: str) -> int:
    failures = 0
    checked = 0
    skipped = 0
    for name in sorted(os.listdir(workflows_dir)):
        if not name.endswith((".yml", ".yaml")):
            continue
        path = os.path.join(workflows_dir, name)
        with open(path, encoding="utf-8") as handle:
            source_lines = handle.read().split("\n")
        for run_line, _indicator, script in run_blocks(source_lines):
            if not is_bash_block(source_lines, run_line):
                skipped += 1
                continue
            checked += 1
            # 经 stdin 以字节交给 bash:避免 Windows 下临时文件路径转换,
            # 也避免 text 模式的 \n -> \r\n 翻译制造假失败。
            result = subprocess.run(
                ["bash", "-n"],
                input=(script + "\n").encode("utf-8"),
                capture_output=True,
            )
            if result.returncode != 0:
                failures += 1
                print(
                    f"FAIL {name}: run 块 shell 语法不可解析(第 {run_line + 1} 行 run:)"
                )
                stderr = result.stderr.decode("utf-8", "replace")
                for output_line in stderr.strip().split("\n"):
                    if output_line:
                        print(f"  {output_line}")
    if failures:
        print(
            f"工作流 run 块语法守卫失败:{failures} 个块不可解析"
            f"(检查 {checked} 个,跳过非 bash {skipped} 个)"
        )
        return 1
    print(
        f"工作流 run 块语法守卫通过:{checked} 个块全部可解析"
        f"(跳过非 bash {skipped} 个)"
    )
    return 0


def main() -> int:
    root = sys.argv[1] if len(sys.argv) > 1 else os.getcwd()
    workflows_dir = os.path.join(root, ".github", "workflows")
    if not os.path.isdir(workflows_dir):
        print(f"找不到工作流目录: {workflows_dir}", file=sys.stderr)
        return 2
    return check(workflows_dir)


if __name__ == "__main__":
    raise SystemExit(main())
