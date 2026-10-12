#!/usr/bin/env python3
"""Create/check the public Rime ZIP against an immutable trusted compiler snapshot."""
import argparse
from pathlib import Path
import stat
import zipfile
from check_generated_runtime_sources import compile_reference, verify as verify_runtime


def payloads(package, root, reference=None):
    if reference is None:
        with compile_reference(root) as compiled:
            return payloads(package, root, compiled)
    verify_runtime(package, root, reference)
    # Never reopen caller-controlled files after verification. The private
    # compiler output supplies the immutable snapshot, including notices.
    return {name: (reference.directory / name).read_bytes() for name in reference.artifacts}


def check(archive, contents):
    with zipfile.ZipFile(archive) as zipped:
        names = zipped.namelist()
        if len(names) != len(set(names)) or set(names) != set(contents):
            raise ValueError("archive inventory mismatch or duplicate name")
        for name, expected in contents.items():
            if zipped.read(name) != expected:
                raise ValueError(f"archive byte mismatch: {name}")
            member = zipped.getinfo(name)
            mode = member.external_attr >> 16
            if (member.create_system != 3 or not stat.S_ISREG(mode)
                    or stat.S_IMODE(mode) != 0o644 or member.flag_bits & 1):
                raise ValueError(f"unsafe archive member metadata: {name}")
    return len(contents)


def create(archive, contents):
    # Exclusive output and fixed metadata: no stale archive update or timestamp drift.
    with zipfile.ZipFile(archive, "x", compression=zipfile.ZIP_DEFLATED) as zipped:
        for name, data in sorted(contents.items()):
            info = zipfile.ZipInfo(name, date_time=(1980, 1, 1, 0, 0, 0))
            info.create_system = 3
            info.external_attr = (stat.S_IFREG | 0o644) << 16
            info.compress_type = zipfile.ZIP_DEFLATED
            zipped.writestr(info, data, compresslevel=9)
    return check(archive, contents)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("mode", choices=("verify", "create", "check"))
    parser.add_argument("package", type=Path)
    parser.add_argument("archive", type=Path, nargs="?")
    args = parser.parse_args()
    if args.mode != "verify" and args.archive is None:
        parser.error("create/check require an archive path")
    contents = payloads(args.package, Path(__file__).resolve().parents[2])
    if args.mode == "verify":
        print(f"PASS Rime package: {len(contents)} exact compiled/generated file identities")
        return
    count = (create if args.mode == "create" else check)(args.archive, contents)
    print(f"PASS Rime archive: {count} exact generated/document/license file identities")


if __name__ == "__main__":
    main()
