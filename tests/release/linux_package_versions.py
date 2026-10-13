#!/usr/bin/env python3
"""Represent SemVer RCs in Debian/RPM ordering without changing application bytes."""
import argparse
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import subprocess
import tarfile
import tempfile

from check_product_version import VERSION


def native_versions(version):
    if not VERSION.fullmatch(version):
        raise ValueError("expected a canonical product version")
    core, separator, number = version.partition("-rc.")
    return {
        "deb": f"{core}~rc.{number}" if separator else core,
        "rpm_version": core,
        "rpm_release": f"0.rc.{number}" if separator else "1",
    }


def payload_identity(package):
    """Compare installed paths, content, permissions, owners and link targets."""
    process = subprocess.Popen(["dpkg-deb", "--fsys-tarfile", str(package)], stdout=subprocess.PIPE)
    rows = []
    try:
        with tarfile.open(fileobj=process.stdout, mode="r|") as archive:
            for member in archive:
                path = PurePosixPath(member.name)
                if path.is_absolute() or ".." in path.parts:
                    raise ValueError("unsafe payload path")
                if str(path) == ".":
                    continue
                digest = None
                if member.isfile():
                    with archive.extractfile(member) as stream:
                        digest = hashlib.file_digest(stream, "sha256").hexdigest()
                rows.append((str(path), member.type, member.mode, member.uid, member.gid,
                             member.linkname, member.size, digest))
    finally:
        process.stdout.close()
        status = process.wait()
    if status:
        raise subprocess.CalledProcessError(status, process.args)
    return sorted(rows)


def normalize_deb(source, output, version):
    expected = native_versions(version)["deb"]
    if output.exists():
        raise FileExistsError(output)
    actual = subprocess.check_output(["dpkg-deb", "--field", str(source), "Version"], text=True).strip()
    if actual != version:
        raise ValueError(f"input DEB version {actual!r} differs from source {version!r}")
    before = payload_identity(source)
    output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="xhup-deb-", dir=output.parent) as temporary:
        work = Path(temporary)
        extracted = work / "package"
        subprocess.run(["dpkg-deb", "--raw-extract", str(source), str(extracted)], check=True)
        control = extracted / "DEBIAN/control"
        lines = control.read_text(encoding="utf-8").splitlines(keepends=True)
        positions = [index for index, line in enumerate(lines) if line.startswith("Version:")]
        if len(positions) != 1:
            raise ValueError("expected exactly one Version field")
        lines[positions[0]] = f"Version: {expected}\n"
        control.write_text("".join(lines), encoding="utf-8")
        rebuilt = work / "normalized.deb"
        subprocess.run(["dpkg-deb", "--build", "--root-owner-group", str(extracted), str(rebuilt)], check=True)
        actual = subprocess.check_output(["dpkg-deb", "--field", str(rebuilt), "Version"], text=True).strip()
        if actual != expected or payload_identity(rebuilt) != before:
            raise ValueError("normalization changed package payload or produced the wrong version")
        # Same filesystem and exclusive publication; never overwrite an old artifact.
        os.link(rebuilt, output)
    return expected


def verify_headers(deb, rpm, version):
    expected = native_versions(version)
    actual_deb = subprocess.check_output(["dpkg-deb", "--field", str(deb), "Version"], text=True).strip()
    actual_rpm = subprocess.check_output([
        "rpm", "--query", "--package", "--queryformat", "%{VERSION}\n%{RELEASE}\n", str(rpm)
    ], text=True).splitlines()
    if actual_deb != expected["deb"] or actual_rpm != [expected["rpm_version"], expected["rpm_release"]]:
        raise ValueError(f"native package versions disagree: DEB={actual_deb!r}, RPM={actual_rpm!r}")
    return expected


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)
    config = sub.add_parser("rpm-config")
    config.add_argument("--version", required=True)
    config.add_argument("--output", type=Path, required=True)
    deb = sub.add_parser("deb")
    deb.add_argument("--version", required=True)
    deb.add_argument("--input", type=Path, required=True)
    deb.add_argument("--output", type=Path, required=True)
    check = sub.add_parser("check")
    check.add_argument("--version", required=True)
    check.add_argument("--deb", type=Path, required=True)
    check.add_argument("--rpm", type=Path, required=True)
    describe = sub.add_parser("describe")
    describe.add_argument("--version", required=True)
    args = parser.parse_args()
    if args.command == "rpm-config":
        versions = native_versions(args.version)
        # Only `tauri bundle` consumes this; never compile the application with it.
        with args.output.open("x", encoding="utf-8") as stream:
            json.dump({"version": versions["rpm_version"], "bundle": {"linux": {
                "rpm": {"release": versions["rpm_release"]}}}}, stream, indent=2)
            stream.write("\n")
    elif args.command == "deb":
        print(normalize_deb(args.input.resolve(), args.output.resolve(), args.version))
    elif args.command == "describe":
        print(json.dumps(native_versions(args.version)))
    else:
        print(json.dumps(verify_headers(args.deb, args.rpm, args.version)))


if __name__ == "__main__":
    main()
