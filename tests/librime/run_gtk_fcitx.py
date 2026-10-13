#!/usr/bin/env python3
"""Actual GTK/Fcitx X11 smoke, not complete platform acceptance or a latency test.

Requires GTK3 development files, Fcitx5/Rime/GTK3, Xvfb, xauth, xdotool and D-Bus.
--prefix can point at authenticated distribution packages extracted without root.
The optional cold-control burst retains the original startup failure scenario.
"""
from pathlib import Path
import argparse
import hashlib
import json
import shlex
import subprocess
import sys
import sysconfig
import tempfile
import threading
import time


def run(command, **options):
    return subprocess.run([str(item) for item in command], check=True, **options)


def wait_for(condition, description, timeout=15):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if condition():
            return
        time.sleep(0.05)
    raise RuntimeError(f"timed out: {description}")


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def outer(args):
    output = args.output
    output.mkdir(parents=True, exist_ok=False)
    home = output / "home"
    for name in ("config", "data", "cache", "runtime"):
        (home / name).mkdir(parents=True, mode=0o700)
    (home / "config/fcitx5").mkdir()
    (home / "config/fcitx5/profile").write_text("""[Groups/0]
Name=Default
Default Layout=us
DefaultIM=rime

[Groups/0/Items/0]
Name=keyboard-us
Layout=

[Groups/0/Items/1]
Name=rime
Layout=

[GroupOrder]
0=Default
""")
    # Accept a release package, never an existing installation/user dictionary.
    sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "release"))
    from rime_archive import payloads
    contents = payloads(args.package, args.source_root)
    destination = home / "data/fcitx5/rime"
    destination.mkdir(parents=True)
    for name, data in contents.items():
        path = destination / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(data)
    probe = Path(__file__).with_name("gtk_input_probe.c")
    flags = shlex.split(subprocess.check_output(["pkg-config", "--cflags", "--libs", "gtk+-3.0"], text=True))
    compile_command = ["cc", "-O2", "-Wall", "-Wextra", "-Werror", str(probe),
                       "-o", str(output / "gtk-input-probe"), *flags]
    run(compile_command)
    libraries = args.prefix / "lib" / sysconfig.get_config_var("MULTIARCH")
    environment = dict(LANG="C.UTF-8", HOME=str(home), XDG_CONFIG_HOME=str(home / "config"),
        XDG_DATA_HOME=str(home / "data"), XDG_CACHE_HOME=str(home / "cache"),
        XDG_RUNTIME_DIR=str(home / "runtime"), PATH=f"{args.prefix}/bin:/usr/local/bin:/usr/bin:/bin",
        LD_LIBRARY_PATH=str(libraries), GTK_IM_MODULE="fcitx", XMODIFIERS="@im=fcitx",
        FCITX_ADDON_DIRS=str(libraries / "fcitx5"), FCITX_DATA_DIRS=str(args.prefix / "share/fcitx5"),
        XDG_DATA_DIRS=f"{args.prefix}/share:/usr/share", GTK_IM_MODULE_FILE=str(output / "gtk-immodules.cache"),
        GDK_BACKEND="x11", TMPDIR=str(output))
    if args.sync_mode:
        environment["FCITX_ENABLE_SYNC_MODE"] = "1"
    query_roots = {libraries, Path("/usr/lib") / sysconfig.get_config_var("MULTIARCH")}
    queries = {path.resolve() for root in query_roots for path in root.glob("libgtk-3*/gtk-query-immodules-3.0")}
    if len(queries) != 1:
        raise ValueError(f"expected one GTK3 module query tool, found {len(queries)}")
    with (output / "gtk-immodules.cache").open("w") as cache:
        run([queries.pop(), libraries / "gtk-3.0/3.0.0/immodules/im-fcitx5.so"], env=environment, stdout=cache)
    identity = dict(scope="Linux GTK/Fcitx X11 smoke only; not full platform acceptance",
        package={name: hashlib.sha256(data).hexdigest() for name, data in contents.items()},
        compile_command=compile_command, probe_sha256=digest(probe), driver_sha256=digest(Path(__file__)),
        fcitx_sha256=digest(args.prefix / "bin/fcitx5"),
        source_root=str(args.source_root),
        producer_head=subprocess.check_output(
            ["git", "-C", str(args.source_root), "rev-parse", "HEAD"], text=True).strip(),
        producer_dirty=bool(subprocess.check_output(
            ["git", "-C", str(args.source_root), "status", "--porcelain"], text=True).strip()),
        cold_control_burst=args.cold_control_burst,
        gtk_key_delivery="sync" if args.sync_mode else "distribution-default",
        lua_loading="distribution librime plugin loader; no LD_PRELOAD override")
    (output / "identity.json").write_text(json.dumps(identity, indent=2) + "\n")
    # Short, private socket path; no host service directories or secret agents.
    with tempfile.TemporaryDirectory(prefix="xhgui-", dir="/tmp") as bus_dir:
        bus_config = output / "session-bus.conf"
        bus_config.write_text(f"""<!DOCTYPE busconfig PUBLIC "-//freedesktop//DTD D-Bus Bus Configuration 1.0//EN"
 "http://www.freedesktop.org/standards/dbus/1.0/busconfig.dtd">
<busconfig>
  <type>session</type>
  <listen>unix:tmpdir={bus_dir}</listen>
  <auth>EXTERNAL</auth>
  <policy context="default">
    <allow own="*"/>
    <allow send_destination="*"/>
    <allow receive_sender="*"/>
  </policy>
</busconfig>
""")
        command = [args.prefix / "bin/xvfb-run", "-a", "-s", "-screen 0 1024x768x24 -nolisten tcp",
            "dbus-run-session", "--config-file", bus_config, "--", sys.executable, Path(__file__).resolve(),
            "--inside", "--prefix", args.prefix, "--package", args.package, "--output", output]
        if args.cold_control_burst:
            command.append("--cold-control-burst")
        if args.sync_mode:
            command.append("--sync-mode")
        return subprocess.run([str(item) for item in command], env=environment).returncode


def inside(args):
    output = args.output
    bin_dir = args.prefix / "bin"
    ready = threading.Event()
    frontend = subprocess.Popen([str(bin_dir / "fcitx5"), "--disable=all",
        "--enable=keyboard,rime,dbus,dbusfrontend,xcb,xim,classicui", "--verbose=rime=5"],
        stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)

    def log_frontend():
        with (output / "fcitx.log").open("w") as log:
            for line in frontend.stdout:
                log.write(line)
                log.flush()
                if "Notification:" in line and " deploy success" in line:
                    ready.set()

    logger = threading.Thread(target=log_frontend, daemon=True)
    logger.start()
    probe = None
    report = dict(result="FAIL", scope="Linux GTK/Fcitx X11 smoke only", cases=[])
    try:
        with (output / "gtk.log").open("w") as log:
            probe = subprocess.Popen([str(output / "gtk-input-probe"), str(output)], stdout=log, stderr=subprocess.STDOUT)
            window = subprocess.check_output([str(bin_dir / "xdotool"), "search", "--sync", "--onlyvisible",
                "--name", "^XHUP isolated Fcitx input probe$"], text=True, timeout=15).splitlines()[0]
            run([bin_dir / "xdotool", "windowfocus", "--sync", window])
            wait_for(lambda: subprocess.run([str(bin_dir / "fcitx5-remote")], capture_output=True).returncode == 0,
                     "private Fcitx controller")
            run([bin_dir / "fcitx5-remote", "-s", "rime"])
            run([bin_dir / "fcitx5-remote", "-o"])
            wait_for(ready.is_set, "frontend deployment success", timeout=240)
            if args.cold_control_burst:
                run([bin_dir / "xdotool", "key", "--clearmodifiers", "Escape", "ctrl+a", "BackSpace"])
            cases = (
                ("q", "去"), ("jqu", "就是"), ("jbzqu", "进去"),
                ("nihcnzqu", "你好去"), ("nihcnznihcnzqu", "你好你好去"),
                ("nihcvegeuurufawojtdesuduhduikeyi", "你好这个输入法我觉得速度还是可以"),
            )
            for index, (keys, expected) in enumerate(cases):
                case = dict(keys=keys, expected=expected, result="FAIL", screenshot=f"screen-{index}.png")
                try:
                    if index:
                        run([bin_dir / "xdotool", "key", "--clearmodifiers", "Escape", "ctrl+a", "BackSpace"])
                        wait_for(lambda: (output / "committed.txt").read_text() == "", "clear committed text")
                    run([bin_dir / "xdotool", "type", "--clearmodifiers", "--delay", "100", keys])
                    wait_for(lambda: (output / "preedit.txt").read_text().replace(" ", "") == keys,
                             f"GTK preedit retains {keys}")
                    if (output / "committed.txt").read_text() != "":
                        raise AssertionError("IME committed before selection")
                    (output / "capture.request").write_text("capture without keyboard side effects\n")
                    wait_for(lambda: (output / f"screen-{index}.png").exists(), "actual candidate screenshot")
                    case["preedit_before_selection"] = (output / "preedit.txt").read_text()
                    run([bin_dir / "xdotool", "key", "space"])
                    wait_for(lambda: (output / "committed.txt").read_text() == expected, f"{keys} commits {expected}")
                    wait_for(lambda: (output / "preedit.txt").read_text() == "", "no remaining composition")
                    case["result"] = "PASS"
                except Exception as error:
                    case["error"] = str(error)
                case["committed"] = (output / "committed.txt").read_text()
                case["remaining_preedit"] = (output / "preedit.txt").read_text()
                report["cases"].append(case)
                print(json.dumps(case, ensure_ascii=False), flush=True)
            if any(case["result"] != "PASS" for case in report["cases"]):
                raise AssertionError("one or more actual frontend cases failed; all results retained")
            report["result"] = "PASS"
    except Exception as error:
        report["error"] = str(error)
        raise
    finally:
        for process in (probe, frontend):
            if process is not None and process.poll() is None:
                process.terminate()
                try:
                    process.wait(timeout=15)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait()
        logger.join(timeout=5)
        (output / "result.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n")
    return 0


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--package", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--prefix", type=Path, default=Path("/usr"))
    parser.add_argument("--source-root", type=Path, default=Path(__file__).resolve().parents[2],
                        help="trusted producer checkout for independent artifact verification")
    parser.add_argument("--cold-control-burst", action="store_true")
    parser.add_argument("--sync-mode", action="store_true",
                        help="diagnose with upstream synchronous GTK key delivery; not the default")
    parser.add_argument("--inside", action="store_true", help=argparse.SUPPRESS)
    args = parser.parse_args()
    args.package, args.output, args.prefix = args.package.resolve(), args.output.resolve(), args.prefix.resolve()
    args.source_root = args.source_root.resolve()
    return inside(args) if args.inside else outer(args)


if __name__ == "__main__":
    raise SystemExit(main())
