#!/usr/bin/env python3
"""Real Linux WebKit/IPC smoke in private HOME/XDG. No mocked product APIs.
Run inside dbus-run-session. Requires Xvfb, WebKitWebDriver and a built binary.
This does not qualify fcitx5 desktop integration or Android user acceptance.
"""
import argparse
import errno
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import socket
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.request

ELEMENT = "element-6066-11e4-a52e-4f735466cecf"


class SettledTemporaryDirectory(tempfile.TemporaryDirectory):
    """Allow terminated WebKit helpers to finish cache writes; never ignore failure."""

    def cleanup(self):
        for attempt in range(20):
            try:
                return super().cleanup()
            except OSError as error:
                if error.errno != errno.ENOTEMPTY or attempt == 19:
                    raise
                time.sleep(.1)


def sha(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--source-root", type=Path, required=True)
    parser.add_argument("--syllables", type=Path, required=True)
    parser.add_argument("--version", required=True)
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--xvfb", default="Xvfb")
    parser.add_argument("--driver", default="WebKitWebDriver")
    args = parser.parse_args()
    root = args.source_root.resolve()
    binary = args.binary.resolve(strict=True)
    sys.path.insert(0, str(root / "tests/release"))
    from rime_archive import GENERATED, NOTICES, verify_runtime
    report = {
        "binary_sha256": sha(binary), "expected_version": args.version,
        "source_commit": subprocess.check_output(["git", "-C", root, "rev-parse", "HEAD"], text=True).strip(),
        "syllables_sha256": sha(args.syllables), "checks": [], "passed": False,
        "scope": "actual Linux WebKit frontend and Rust IPC; isolated synthetic profile; not fcitx5/Android manual acceptance",
    }
    def check(condition, label):
        if not condition:
            raise AssertionError(label)
        report["checks"].append(label)
        print("PASS", label, flush=True)

    with SettledTemporaryDirectory(prefix="xhup-gui-") as temporary:
        work = Path(temporary)
        env = os.environ.copy()
        for name in ("HTTP_PROXY", "HTTPS_PROXY", "ALL_PROXY", "http_proxy", "https_proxy", "all_proxy"):
            env.pop(name, None)
        for key, name in (("HOME", "home"), ("XDG_CONFIG_HOME", "config"),
                          ("XDG_DATA_HOME", "data"), ("XDG_CACHE_HOME", "cache")):
            path = work / name
            path.mkdir()
            env[key] = str(path)
        env.update(TAURI_WEBVIEW_AUTOMATION="true", GDK_BACKEND="x11")
        rime = work / "data/fcitx5/rime"
        rime.mkdir(parents=True)
        sentinel = rime / "default.custom.yaml"
        sentinel.write_bytes(b"# synthetic user configuration, preserve me\n")
        export_parent = work / "export"
        export_parent.mkdir()
        driver = None
        session = None
        x = None
        opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
        with socket.socket() as sock:
            sock.bind(("127.0.0.1", 0))
            port = sock.getsockname()[1]

        def call(method, path, body=None):
            payload = None if body is None else json.dumps(body).encode()
            request = urllib.request.Request(f"http://127.0.0.1:{port}" + path,
                data=payload, method=method, headers={"Content-Type": "application/json"})
            try:
                with opener.open(request, timeout=60) as response:
                    return json.load(response)["value"]
            except urllib.error.HTTPError as error:
                raise RuntimeError(error.read().decode()) from error

        def execute(script, arguments=None, asynchronous=False):
            return call("POST", f"/session/{session}/execute/" + ("async" if asynchronous else "sync"),
                        {"script": script, "args": arguments or []})

        def wait(probe):
            deadline = time.monotonic() + 30
            while time.monotonic() < deadline:
                result = probe()
                if result:
                    return result
                time.sleep(.1)
            raise TimeoutError("UI condition was not satisfied")

        def element(label):
            return execute("return [...document.querySelectorAll('button')].find(e=>"
                "(e.textContent.trim()===arguments[0] || e.getAttribute('aria-label')===arguments[0]) "
                "&& e.getClientRects().length)", [label])

        def click(label):
            ref = wait(lambda: element(label))
            call("POST", f"/session/{session}/element/{ref[ELEMENT]}/click", {})
            time.sleep(.2)

        def text():
            return execute("return document.body.innerText")

        def type_into(selector, value):
            ref = wait(lambda: execute("return document.querySelector(arguments[0])", [selector]))
            call("POST", f"/session/{session}/element/{ref[ELEMENT]}/value", {"text": value})

        def start():
            result = call("POST", "/session", {"capabilities": {"alwaysMatch": {
                "browserName": "wry", "webkitgtk:browserOptions": {"binary": str(binary)}}}})
            report["webdriver"] = result["capabilities"]
            return result["sessionId"]

        try:
            with (work / "xvfb.log").open("wb") as xlog, (work / "driver.log").open("wb") as dlog:
                x = subprocess.Popen([args.xvfb, "-displayfd", "1", "-screen", "0",
                    "1280x900x24", "-nolisten", "tcp"], stdout=subprocess.PIPE, stderr=xlog, env=env)
                display = x.stdout.readline().decode().strip()
                if not display:
                    raise RuntimeError("Xvfb failed to start")
                env["DISPLAY"] = ":" + display
                driver = subprocess.Popen([args.driver, "--host=127.0.0.1", f"--port={port}"],
                    stdout=dlog, stderr=dlog, env=env)
                for attempt in range(100):
                    try:
                        call("GET", "/status")
                        break
                    except (OSError, RuntimeError):
                        if driver.poll() is not None:
                            raise RuntimeError("WebDriver exited")
                        time.sleep(.1)
                session = start()
                wait(lambda: "欢迎使用 XHUP Flow" in text())
                check(execute("return location.protocol") == "tauri:", "packaged local frontend loads under real WebKit")
                click("跳过,稍后再说")
                click("输入法")
                wait(lambda: args.version in text())
                check("未验证（Unknown）" in text(), "live capability is not inferred from files")
                click("安装")
                wait(lambda: "xhup_flow.sources.tsv" in text())
                check(not (rime / "xhup_flow.schema.yaml").exists(), "install plan does not write before confirmation")
                click("确认执行")
                wait(lambda: (rime / "lua/xhup_flow/data/quick_hints.lua").is_file())
                check(all((rime / name).is_file() for name in GENERATED), "UI confirmed install writes every nested package file")
                check(sentinel.read_bytes() == (root / 'rime/package/default.custom.yaml').read_bytes(), "installed Rime schema list is exclusively XHUP Flow")
                saved = json.loads((rime / '.xhup-flow-default-backup.json').read_text(encoding='utf-8'))
                check(bytes(saved['original']) == b"# synthetic user configuration, preserve me\n", "original shared configuration is backed up byte for byte")
                check("未验证（Unknown）" in text(), "installation is not misreported as live runtime qualification")

                type_into("#export-destination", str(export_parent))
                click("导出包")
                target = export_parent / ("xhup-flow-rime-v" + args.version)
                wait(lambda: target.is_dir() and all((target / name).is_file() for name in NOTICES))
                actual = {p.relative_to(target).as_posix() for p in target.rglob("*") if p.is_file()}
                check(actual == GENERATED | set(NOTICES), "export has exact public inventory and no user state")
                check(all((target / n).read_bytes() == (rime / n).read_bytes() for n in GENERATED), "export and installed immutable payload bytes match")
                check(all((target / n).read_bytes() == (root / src).read_bytes() for n, src in NOTICES.items()), "all exported license/document bytes match source")
                verify_runtime(target, root)
                check(True, "export runtime modules and schemas match source")
                before = {name: sha(target / name) for name in actual}
                duplicate = execute("const done=arguments[arguments.length-1];"
                    "window.__TAURI_INTERNALS__.invoke('product_export_package',{destination:arguments[0]})"
                    ".then(()=>done({ok:true}),e=>done({ok:false,code:e.code}));", [str(export_parent)], True)
                check(not duplicate["ok"] and duplicate.get("code") == "io", "existing export is explicitly refused")
                check(before == {name: sha(target / name) for name in actual}, "refused duplicate export leaves every byte unchanged")

                click("卸载")
                confirm = wait(lambda: execute("return [...document.querySelectorAll('[role=dialog] button')]"
                    ".find(e=>e.textContent.trim()==='卸载' && e.getClientRects().length)"))
                check((rime / "xhup_flow.schema.yaml").is_file(), "uninstall requires explicit dialog confirmation")
                call("POST", f"/session/{session}/element/{confirm[ELEMENT]}/click", {})
                wait(lambda: not (rime / "xhup_flow.schema.yaml").exists())
                check(sentinel.read_bytes() == b"# synthetic user configuration, preserve me\n", "uninstall restores the exact pre-install shared configuration")
                check(not (rime / '.xhup-flow-default-backup.json').exists(), "successful restoration releases managed shared-file ownership")

                click("今日")
                click("开始练习")
                click("开始练习")
                wait(lambda: "编码输入区" == execute("return document.querySelector('input')?.getAttribute('aria-label')"))
                codes = dict(line.split("\t") for line in args.syllables.read_text(encoding="utf-8").splitlines() if not line.startswith("#"))
                reading = next(line.split(" / ")[0] for line in text().splitlines() if re.fullmatch("[a-z]+( / [a-z]+)*", line))
                type_into("input", codes[reading])
                wait(lambda: "1 / 30" in text())
                check(True, "real WebDriver keyboard events complete a double-pinyin question")
                # Default on-error hint is visible during feedback only. Pause is
                # defined for the next question, not the short feedback phase.
                wait(lambda: execute("return document.querySelector('[aria-live=polite]')?.textContent.trim()") == "")
                click("暂停")
                wait(lambda: "已暂停" in text())
                check(True, "practice pause works")
                click("继续练习")
                progress = execute("return JSON.parse(localStorage.getItem('xhup-flow.trainer.v2')).state.progress")
                check(sum(p["correct"] for p in progress.values()) == 1, "correct answer is persisted locally")
                call("DELETE", "/session/" + session)
                session = None
                session = start()
                wait(lambda: "小鹤音形训练" in text())
                after = execute("return JSON.parse(localStorage.getItem('xhup-flow.trainer.v2')).state.progress")
                check(progress == after, "practice progress survives actual application restart")
                check("欢迎使用 XHUP Flow" not in text(), "onboarding choice survives restart")
                report["passed"] = True
        except Exception as error:
            report["error"] = str(error)
            raise
        finally:
            cleanup_error = None
            if session:
                try:
                    call("DELETE", "/session/" + session)
                except Exception as error:
                    cleanup_error = error
                    report["passed"] = False
                    report["cleanup_error"] = str(error)
            for process in (driver, x):
                if process:
                    process.terminate()
                    try:
                        process.wait(timeout=10)
                    except subprocess.TimeoutExpired:
                        process.kill()
                        process.wait(timeout=10)
            args.report.parent.mkdir(parents=True, exist_ok=True)
            args.report.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
            for name in ("xvfb.log", "driver.log"):
                if (work / name).exists():
                    shutil.copyfile(work / name, args.report.with_name(args.report.stem + "-" + name))
            if cleanup_error:
                raise RuntimeError("WebDriver session cleanup failed") from cleanup_error
    print(f"PASS actual Linux desktop smoke: {len(report['checks'])} checks")


if __name__ == "__main__":
    main()
