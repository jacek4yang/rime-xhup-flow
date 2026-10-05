#!/usr/bin/env python3
"""Real training-only Linux WebKit smoke in private HOME/XDG. No mocked app APIs.
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
        sentinel = rime / "default.custom.yaml"
        def profile_snapshot():
            if not rime.exists():
                return None
            return {p.relative_to(rime).as_posix(): ("dir" if p.is_dir() else sha(p))
                    for p in sorted(rime.rglob("*"))}
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
                wait(lambda: "小鹤音形训练" in text())
                check(execute("return location.protocol") == "tauri:", "packaged local frontend loads under real WebKit")
                check(not rime.exists(), "training starts without creating a Rime profile")
                check(not execute("return [...document.querySelectorAll('button')].some(e=>"
                    "['输入法','安装','修复','卸载','导出包'].includes(e.textContent.trim()))"),
                    "no input-method management entry or installer onboarding")
                removed_commands = ["product_status", "product_plan", "product_execute",
                    "product_diagnostics", "product_redeploy", "product_export_package",
                    "learning_export", "learning_import", "learning_reset",
                    "explain_word", "explain_hint", "explain_words_batch"]
                for command in removed_commands:
                    rejected = execute("const done=arguments[arguments.length-1];"
                        "window.__TAURI_INTERNALS__.invoke(arguments[0],{})"
                        ".then(()=>done({ok:true}),e=>done({ok:false,error:String(e)}));", [command], True)
                    check(not rejected["ok"] and "not found" in rejected.get("error", "").lower()
                        and command in rejected.get("error", ""), f"removed IPC {command} is not registered")
                check(not rime.exists(), "removed IPC probes never create input-method state")

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
                check(not rime.exists(), "practice and shutdown require no Rime installation")
                rime.mkdir(parents=True)
                sentinel.write_bytes(b"# synthetic user configuration, preserve me\n")
                (rime / "other.schema.yaml").write_bytes(b"schema: unrelated\n")
                (rime / "xhup_flow_user.userdb").mkdir()
                (rime / "xhup_flow_user.userdb/synthetic").write_bytes(b"private learning sentinel\n")
                before_profile = profile_snapshot()
                session = start()
                wait(lambda: "小鹤音形训练" in text())
                after = execute("return JSON.parse(localStorage.getItem('xhup-flow.trainer.v2')).state.progress")
                check(progress == after, "practice progress survives actual application restart")
                check("欢迎使用 XHUP Flow" not in text(), "restart never opens installer onboarding")
                for destination in ("错题", "统计", "键位", "学习", "设置"):
                    click(destination)
                check("导出进度备份" in text(), "training progress backup remains available")
                check(execute("return document.querySelector('[data-testid=training-data-version]')?.textContent.trim()") == args.version,
                    "packaged training dataset version matches the expected artifact version")
                check("输入法学习数据" not in text(), "settings do not manage Rime user dictionaries")
                check(profile_snapshot() == before_profile, "all preexisting Rime files and learning bytes remain unchanged")
                call("DELETE", "/session/" + session)
                session = None
                check(profile_snapshot() == before_profile, "shutdown leaves the entire Rime profile unchanged")
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
