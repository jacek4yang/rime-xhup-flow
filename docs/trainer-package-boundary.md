> 历史设计记录：当前 Trainer 已移除输入法管理及内嵌安装包，本文描述的安装/导出功能不再提供。当前安装方式见[手动教程](install-guide.zh-CN.md)。

# Trainer immutable package boundary (F13)

Build/CI: source → Rust generator → immutable artifacts and SHA-256 manifest in
Cargo OUT_DIR → embedded Trainer package. `xhup-generator` is now a **build**
dependency for package management; normal status/install/repair/uninstall does
not invoke it. Explicit developer CLI regeneration remains available.

Runtime: embedded file index → process-wide read-only `Arc` cache → existing
plan/apply installer. Caller-visible files remain immutable, version is derived
from the bundled schema, and existing ownership/integrity/rollback contracts are
unchanged. Status still reads installed files for integrity checks; it does not
compile or regenerate any corpus. It neither loads untrusted loose package files
nor depends on the current working directory for bundled content.

The build manifest records filename, SHA-256 and exact byte length for every
artifact. Tests verify the entire embedded package against that manifest and
prove repeated accesses share the same allocation. Package generation is moved,
not removed: build resource use still requires measurement. This change does not
claim compiler/deployer optimization or complete installer safety (F14/F15).

The CLI dependency still exposes explicit developer analysis/generation commands;
that transitive code is not a second runtime package authority. Trainer practice
logic and runtime input-method ranking remain separate.
