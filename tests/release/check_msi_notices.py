#!/usr/bin/env python3
"""Administrative extraction of our built MSI, never application/user-data setup."""
import argparse
from pathlib import Path
import subprocess
from check_bundle_notices import verify

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("msi", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=False)
    log = args.output / "msi-extraction.log"
    result = subprocess.run(["msiexec.exe", "/a", str(args.msi.resolve()), "/qn",
                             "TARGETDIR=" + str(args.output.resolve()), "/l*v", str(log.resolve())],
                            check=False)
    if result.returncode:
        if log.exists():
            print(log.read_text(encoding="utf-16", errors="replace"))
        raise SystemExit(result.returncode)
    directories = [path for path in args.output.rglob("licenses") if path.is_dir()]
    if len(directories) != 1:
        raise ValueError(f"expected one extracted license tree, found {len(directories)}")
    count = verify(directories[0].parent, Path(__file__).resolve().parents[2])
    print(f"PASS actual MSI extracted notices: {count} exact source identities")
