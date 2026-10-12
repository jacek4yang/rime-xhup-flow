#!/usr/bin/env python3
"""Recompile trusted sources; reject stale, modified, or extra runtime artifacts."""
import argparse
from contextlib import contextmanager
from dataclasses import dataclass
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import shlex
import stat
import subprocess
import tempfile

MANIFEST = "xhup_flow.runtime-manifest.json"


def identity(path):
    with path.open("rb") as stream:
        digest = hashlib.file_digest(stream, "sha256").hexdigest()
    return {"bytes": path.stat().st_size, "sha256": digest}


def relative_path(name):
    path = PurePosixPath(name)
    if (not name or str(path) != name or path.is_absolute()
            or ".." in path.parts or "\\" in name or ":" in name or "\0" in name):
        raise ValueError(f"unsafe manifest path: {name!r}")
    return Path(*path.parts)


def inventory(directory, ignore_caches=False):
    if directory.is_symlink() or not directory.is_dir():
        raise ValueError(f"unsafe or missing package directory: {directory}")
    names = set()
    for path in directory.rglob("*"):
        relative = path.relative_to(directory)
        if ignore_caches and ("__pycache__" in relative.parts or path.suffix == ".pyc"):
            continue
        mode = path.lstat().st_mode
        if stat.S_ISDIR(mode):
            continue
        if not stat.S_ISREG(mode):
            raise ValueError(f"non-regular file in package: {path}")
        name = path.relative_to(directory).as_posix()
        relative_path(name)
        names.add(name)
    return names


@dataclass(frozen=True)
class RuntimeReference:
    # Constructed only from the trusted compiler, never from the supplied package.
    artifacts: dict
    inputs: dict
    receipt: dict
    directory: Path
    registry_builder: Path

    def check_sources(self, root):
        root = root.resolve()
        for name, expected in self.inputs.items():
            relative = relative_path(name)
            path = root / relative
            if (any((root / parent).is_symlink() for parent in (relative, *relative.parents))
                    or not path.is_file() or identity(path) != expected):
                raise ValueError(f"compiler input mismatch: {name}; rebuild from current sources")
        for directory in self.receipt["compiler_input_directories"]:
            prefix = directory + "/"
            expected = {name.removeprefix(prefix) for name in self.inputs if name.startswith(prefix)}
            if inventory(root / relative_path(directory), ignore_caches=True) != expected:
                raise ValueError(f"compiler input inventory mismatch: {directory}")
        registry = json.loads((root / "rime/lua/modules.json").read_text(encoding="utf-8"))
        registered = set(registry["production"]) | set(registry["excluded"])
        actual = {name for name in inventory(root / "rime/lua", ignore_caches=True) if name.endswith(".lua")}
        if actual != registered:
            raise ValueError("Lua registry mismatch: unregistered or missing source module")

    def verify(self, package, root):
        self.check_sources(root)
        actual = inventory(package)
        expected = set(self.artifacts)
        if actual != expected:
            raise ValueError(f"generated inventory mismatch: missing={sorted(expected-actual)}, extra={sorted(actual-expected)}")
        for name, expected_identity in self.artifacts.items():
            if identity(package / relative_path(name)) != expected_identity:
                raise ValueError(f"compiled artifact mismatch: {name}")
        return len(actual)


def _make_words(value):
    # Make escapes whitespace but does not use shell quotation.
    lexer = shlex.shlex(value, posix=True)
    lexer.whitespace_split = True
    lexer.quotes = ""
    lexer.commenters = ""
    return [word.replace("$$", "$") for word in lexer]


def _dependency_inputs(depfile, root):
    # rustc emits one empty Make rule per input. Do not split the long target rule
    # on whitespace: checkout paths may contain escaped spaces. These gates run
    # on POSIX build hosts; portable output does not contain host-specific paths.
    if os.name != "posix":
        raise ValueError("compiler dependency qualification requires a POSIX build host")
    inputs = set()
    lines = depfile.read_text(encoding="utf-8").splitlines()
    for line in lines:
        if line and not line.startswith("#") and line.endswith(":"):
            names = _make_words(line[:-1])
            if len(names) != 1:
                raise ValueError(f"ambiguous compiler dependency rule: {depfile}")
            inputs.add((root / names[0]).resolve())
    # Cargo's aggregate executable depfile has a target rule rather than phony
    # rules; it includes transitive workspace source and embedded data inputs.
    if not inputs:
        for line in lines:
            if line and not line.startswith("#") and ": " in line:
                inputs.update((root / name).resolve() for name in _make_words(line.split(": ", 1)[1]))
    if not inputs:
        raise ValueError(f"missing compiler dependency evidence: {depfile}")
    return inputs


@contextmanager
def compile_reference(root, target_dir=None):
    """Build with Cargo's locked graph, then independently generate a reference.

    Cargo may reuse compiled objects. Embedded input hashes and rustc dependency
    coverage are checked independently, so an obsolete cached executable fails
    closed instead of making an obsolete package appear current.
    """
    root = root.resolve()
    environment = os.environ.copy()
    if target_dir is not None:
        environment["CARGO_TARGET_DIR"] = str(target_dir.resolve())
    command = ["cargo", "build", "--locked", "-p", "xhup-cli", "--message-format=json"]
    built = subprocess.run(command, cwd=root, env=environment, capture_output=True, text=True)
    if built.returncode:
        raise ValueError(f"runtime compiler build failed:\n{built.stderr}\n{built.stdout}")
    messages = [json.loads(line) for line in built.stdout.splitlines() if line.strip()]
    executable = None
    depfiles = {}
    out_dirs = {}
    build_scripts = {}
    generator_package = None
    for message in messages:
        if message.get("reason") == "build-script-executed":
            out_dirs[message["package_id"]] = Path(message["out_dir"])
        if message.get("reason") != "compiler-artifact":
            continue
        if message["target"]["kind"] == ["custom-build"]:
            build_scripts[message["package_id"]] = Path(message["filenames"][0])
        target = message["target"]["name"].replace("-", "_")
        if target == "xhup_cli" and message.get("executable"):
            executable = Path(message["executable"])
            depfiles["xhup_cli"] = executable.with_suffix(".d")
        if target in ("xhup_core", "xhup_generator"):
            libraries = [Path(name) for name in message["filenames"] if name.endswith(".rlib")]
            if target == "xhup_generator":
                generator_package = message["package_id"]
            if len(libraries) != 1:
                raise ValueError(f"missing normal library dependency evidence: {target}")
            library = libraries[0]
            depfiles[target] = library.with_name(library.name.removeprefix("lib")).with_suffix(".d")
    generated_source = out_dirs.get(generator_package)
    registry_builder = build_scripts.get(generator_package)
    if (executable is None or set(depfiles) != {"xhup_core", "xhup_generator", "xhup_cli"}
            or generated_source is None or registry_builder is None):
        raise ValueError("incomplete runtime compiler build evidence")
    generated_source /= "runtime_sources.rs"
    with tempfile.TemporaryDirectory(prefix="xhup-compiled-reference-") as directory:
        output = Path(directory) / "package"
        generated = subprocess.run([str(executable), "generate", "rime", "--output", str(output)],
                                   cwd=root, capture_output=True, text=True)
        if generated.returncode:
            raise ValueError(f"runtime compilation failed:\n{generated.stderr}\n{generated.stdout}")
        manifest = json.loads((output / MANIFEST).read_text(encoding="utf-8"))
        if manifest["schema_version"] != 1:
            raise ValueError("unsupported runtime manifest version")
        inputs = {}
        artifacts = {}
        for collection, destination in ((manifest["inputs"], inputs), (manifest["artifacts"], artifacts)):
            for item in collection:
                name = item["path"]
                relative_path(name)
                if name in destination:
                    raise ValueError(f"duplicate compiler manifest path: {name}")
                destination[name] = {key: item[key] for key in ("bytes", "sha256")}
        if MANIFEST in artifacts:
            raise ValueError("runtime manifest must not hash itself")
        artifacts[MANIFEST] = identity(output / MANIFEST)
        dependencies = set().union(*(_dependency_inputs(path, root) for path in depfiles.values()))
        dependencies.discard(generated_source.resolve())
        directories = {path for path in dependencies if path.is_dir()}
        dependencies -= directories
        if any(not path.is_relative_to(root) for path in directories):
            raise ValueError("compiler watches an unbound external source directory")
        directory_roots = sorted(path.relative_to(root).as_posix() for path in directories
                                 if not any(parent in directories for parent in path.parents))
        unbound = sorted(str(path) for path in dependencies
                         if not path.is_relative_to(root) or path.relative_to(root).as_posix() not in inputs)
        if unbound:
            raise ValueError(f"unbound compiler dependencies: {unbound}; extend the build input registry")
        receipt = {
            "schema_version": 1,
            "compiler_command": command,
            "compiler_executable": identity(executable),
            "registry_builder_executable": identity(registry_builder),
            "build_environment": {key: value for key, value in sorted(environment.items())
                                  if key in ("RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "CARGO_BUILD_TARGET",
                                             "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER")
                                  or key.startswith("CARGO_PROFILE_")},
            "rustc": subprocess.check_output(["rustc", "-Vv"], cwd=root, text=True).strip(),
            "cargo": subprocess.check_output(["cargo", "--version"], cwd=root, text=True).strip(),
            "compiler": manifest["compiler"],
            "inputs": inputs,
            "artifacts": artifacts,
            "compiler_repository_dependencies": sorted(path.relative_to(root).as_posix() for path in dependencies),
            "compiler_input_directories": directory_roots,
        }
        reference = RuntimeReference(artifacts, inputs, receipt, output, registry_builder)
        reference.verify(output, root)
        yield reference
        reference.check_sources(root)


def verify(package, root, reference=None):
    if reference is not None:
        return reference.verify(package, root)
    with compile_reference(root) as built:
        return built.verify(package, root)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("package", type=Path)
    parser.add_argument("--target-dir", type=Path)
    parser.add_argument("--evidence", type=Path)
    parser.add_argument("--require-clean", action="store_true")
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[2]
    with compile_reference(root, args.target_dir) as reference:
        count = reference.verify(args.package, root)
        if args.evidence or args.require_clean:
            commit = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=root, text=True).strip()
            status = subprocess.check_output(["git", "status", "--porcelain", "--untracked-files=normal"], cwd=root, text=True)
            if args.require_clean and status:
                raise ValueError("release compilation requires a clean source checkout")
            if args.evidence:
                receipt = dict(reference.receipt, source_commit=commit, source_tree_clean=not bool(status))
                with args.evidence.open("x", encoding="utf-8") as stream:
                    json.dump(receipt, stream, ensure_ascii=False, sort_keys=True, indent=2)
                    stream.write("\n")
    print(f"PASS source -> compiler -> artifact: {count} exact file identities")


if __name__ == "__main__":
    main()
