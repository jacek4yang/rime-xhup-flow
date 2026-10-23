#!/usr/bin/env python3
"""Verify every static alias/character against real canonical IDs, weights and bounds.

Input is an already compiler-verified synthetic deployment, never a personal profile.
The private copy disables learning; the Lua audit fixture is not a package artifact.
"""
import argparse
from collections import defaultdict
import hashlib
import json
from pathlib import Path
import shlex
import shutil
import subprocess

import yaml


def dictionary(directory, name, visited):
    if name in visited:
        raise ValueError(f"cyclic dictionary import: {name}")
    visited.add(name)
    if not name or any(character not in "abcdefghijklmnopqrstuvwxyz_" for character in name):
        raise ValueError(f"invalid generated dictionary name: {name}")
    header, body = (directory / f"{name}.dict.yaml").read_text(encoding="utf-8").split("\n...\n", 1)
    for line in body.splitlines():
        if line and not line.startswith("#"):
            yield line.split("\t")
    for imported in yaml.safe_load(header).get("import_tables", []):
        yield from dictionary(directory, imported, visited)
    visited.remove(name)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--deployed", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--shared-data-dir", type=Path, default=Path("/usr/share/rime-data"))
    args = parser.parse_args()
    source, output = args.deployed.resolve(), args.output.resolve()
    if source == output or source in output.parents:
        parser.error("output must be outside the input deployment")
    output.mkdir()
    user = output / "user"
    shutil.copytree(source, user, ignore=shutil.ignore_patterns("*.userdb*", "user.yaml", "sync"))
    wanted = set()
    for text, *_ in dictionary(source, "xhup_flow", set()):
        wanted.add(text)
        wanted.update(text)
    expected = defaultdict(list)
    records = 0
    for text, value, *_ in dictionary(source, "xhup_flow_alias_projection", set()):
        if text == "__xhup_alias_identity__":
            continue
        if text not in wanted:
            raise ValueError(f"projection contains a non-static/non-character text: {text}")
        expected[text].append(value)
        records += 1
    if not wanted:
        raise ValueError("empty static inventory cannot qualify a production projection")
    expected_path = output / "expected.tsv"
    expected_path.write_text("".join(
        text + "\t" + " ".join(expected[text]) + "\n" for text in sorted(wanted)
    ), encoding="utf-8")
    root = Path(__file__).resolve().parent
    shutil.copyfile(root / "runtime_alias_projection.lua", user / "lua/runtime_alias_projection.lua")
    result_path = output / "native-result.json"
    (user / "xhup_flow.custom.yaml").write_text(yaml.safe_dump({"patch": {
        "flow/enable_user_dict": False,
        "engine/translators": ["lua_translator@*runtime_alias_projection"],
        "alias_projection_audit/expected": str(expected_path),
        "alias_projection_audit/result": str(result_path),
    }}), encoding="utf-8")
    with (output / "deploy.log").open("wb") as log:
        subprocess.run(["rime_deployer", "--compile", str(user / "xhup_flow.schema.yaml"),
                        str(user), str(args.shared_data_dir)], stdout=log, stderr=subprocess.STDOUT, check=True)
    flags = shlex.split(subprocess.check_output(["pkg-config", "--cflags", "--libs", "rime"], text=True))
    command = ["cc", "-O2", "-Wall", "-Wextra", "-Werror", "-o", str(output / "probe"),
               str(root / "runtime_alias_projection.c"), *flags]
    subprocess.run(command, check=True)
    log_dir = output / "rime-logs"
    log_dir.mkdir()
    with (output / "native.stdout").open("wb") as stdout, (output / "native.stderr").open("wb") as stderr:
        subprocess.run([str(output / "probe"), str(user), str(args.shared_data_dir), str(log_dir)],
                       stdout=stdout, stderr=stderr, check=True)
    if "lua error" in (output / "native.stderr").read_text().lower():
        raise ValueError("native Lua audit failed; see native.stderr")
    if not result_path.exists():
        stderr_tail = (output / "native.stderr").read_text(encoding="utf-8", errors="replace")[-2000:]
        logs = sorted(log_dir.iterdir())
        log_tail = ""
        if logs:
            newest = max(logs, key=lambda path: path.stat().st_mtime)
            log_tail = newest.read_text(encoding="utf-8", errors="replace")[-4000:]
        raise ValueError(
            "native audit produced no result receipt; probe stderr tail:\n"
            f"{stderr_tail}\nrime logs: {[path.name for path in logs]}\n"
            f"newest rime log tail:\n{log_tail}"
        )
    if json.loads(result_path.read_text()) != {"records": records, "texts": len(wanted)}:
        raise ValueError("native audit did not verify the complete projection inventory")
    identities = {}
    for name in ("xhup_flow_user", "xhup_flow_alias_projection"):
        for extension in ("table.bin", "prism.bin", "reverse.bin"):
            path = user / "build" / f"{name}.{extension}"
            with path.open("rb") as stream:
                identities[path.name] = hashlib.file_digest(stream, "sha256").hexdigest()
    (output / "result.json").write_text(json.dumps({
        "records": records, "texts": len(wanted), "compiled_sha256": identities,
        "build_command": command, "scope": "all static aliases/characters; exact native normalization, not quality or GUI acceptance",
    }, indent=2) + "\n")
    print(f"PASS native alias projection: {records} records, {len(wanted)} static/character texts; exact IDs, weights, reverse order and 8/64 bounds")


if __name__ == "__main__":
    main()
