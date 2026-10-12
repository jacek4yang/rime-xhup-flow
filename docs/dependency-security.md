# Dependency security review

An audit exit code is not a claim of no risk. Initial workspace pnpm audit
reported 51 advisories; the October 2 locked graph reached zero, but that result
is **superseded** by newly reviewed October 4 advisories. The current raw audit
still reports **one high braces advisory**, with an exact local source mitigation.
It is not a zero-advisory result. CI retains the complete raw JSON and fails on
any unmitigated/new/changed advisory or source drift. No pnpm advisory-ignore
configuration is used; the explicit source-bound disposition is described below.

## v2.1 构建前复核

新的完整 `pnpm audit` 报告10条通告（3 critical、4 high、3 moderate），不能沿用下方历史结论。除已有源码绑定缓解的 braces 通告外，9条通告涉及7个包，本轮固定到已发布修复版本：

| 包 | 固定版本 |
| --- | --- |
| proxy-addr | 2.0.8 |
| vm2 | 3.12.2 |
| shell-quote | 1.11.0 |
| source-map-js | 1.2.2 |
| compression | 1.8.2 |
| joi | 17.13.8 |
| postcss-selector-parser | 7.1.6 |

`postcss-selector-parser` 包含6→7跨主版本迁移，实际桌面CSS和Taro微信生产构建均已完成；不能只凭版本范围推断兼容。Taro仍为4.2.1。没有新增通告豁免、关闭校验或放宽发布年龄策略。

实际验证：当前main基线的core184、桌面72、小程序55项测试通过，三包TypeScript检查和两端生产构建通过；15项安全/适配集成测试通过。干净worktree第一次小程序测试因缺少规范数据JSON退出，随后通过本worktree的Rust CLI生成数据，再完成测试和构建，没有复制旧生成资源。Chromium153实际打开生产构建、进入练习并输入错键，页面和CSS正常渲染，无页面异常。此分支不包含另一个PR的盲打修复，不把这次冒烟当作答案保密或Tauri/微信真机验收。

未修改的 `frontend-audit.cjs` 现报告 `rawAdvisories=1, sourceBoundMitigations=1, unmitigated=0`，完整原始JSON保存在 `artifacts/security/pnpm-audit.json`。braces仍有一条原始高危通告，必须继续验证原补丁和十文件源码身份；**不是零通告**。Vite超过500kB的chunk警告仍存在。未测量新旧依赖的配对性能，不宣称提速。

`pnpm install --frozen-lockfile` 通过；`pnpm peers check` 仍以退出码1报告既有 `@tarojs/plugin-framework-react@4.2.1` 要求Vite `^4`、实际7.3.6的不匹配。没有将它写成PASS，也没有改动peer检查策略；本次成功的微信构建使用Webpack。


## October 5 follow-up

- `http-cache-semantics` is upgraded to 4.3.0 (GHSA-ch52-4w7c-c8xp). Testing the
  actual published 4.3.0 still reproduced reuse of non-storable responses when a
  caller supplied `max-stale`. A separate lockfile-bound patch makes
  `evaluateRequest` refuse such responses before freshness evaluation. Regression
  covers private/no-store/authenticated responses and permits ordinary public
  caching. A published version number alone was not accepted as proof.
- `braces` has no published fixed version for GHSA-vfj7-8cjw-p6xm. A local patch
  bounds the three recursive AST walks (compile/expand/stringify) at depth 64,
  including caller-supplied ASTs. Excessive nesting raises a controlled RangeError
  rather than an engine stack overflow. Callers still must handle invalid input;
  this is not a claim of unrestricted glob expansion safety. Normal alternatives,
  ranges, escapes and shallow nesting are retained. Tests include 4,096 nesting
  levels and cyclic/deep ASTs.
- `tests/security/frontend-audit.cjs` runs the full audit, retains its original
  output and verifies **all reachable installed braces copies**. It reverses only
  the exact three guard changes and checks the complete ten-file upstream SHA256
  inventory, including LICENSE. The pinned registry archive SHA512 matches the
  lockfile; all ten files were independently compared with that archive. Missing
  guards, extra files, symlinks or unrelated edits fail. Mutation tests exercise
  these refusals and audit schema/count/status failures.
- The sole disposition requires this exact advisory/module/range/severity/CWE,
  unchanged 3.0.3 findings and proven source bytes. A newly published upstream fix
  also fails this disposition so that the local patch must be retired in favor
  of a reviewed upgrade. This is an explicit local mitigation, **not** suppression
  of all braces findings or an assertion that the advisory database is empty.

No minimum-release-age policy was relaxed. All patches retain upstream licenses.
On this lockfile, both production builds, all workspace typechecks, 351 frontend
unit/component tests (184 core, 112 Trainer, 55 miniapp), seven depth tests,
four audit/cache policy tests and four existing adapter integration tests passed.
The live gate reports one raw advisory, one source-bound mitigation and zero
unmitigated findings. Older zero-audit and packaging results cannot qualify this
dependency change; final binary packaging must be rerun.

## Frontend remediation

The exact versions and integrity bindings are in `pnpm-lock.yaml` and
`pnpm-workspace.yaml`. Besides same-major patches, the remediation upgrades
Webpack/dev-server/middleware, esbuild, glob, Swiper, serialization, adm-zip,
URI decoding, UUID, PostCSS and the package-update checker. Taro remains 4.2.1.
The obsolete webpackbar presentation plugin stays disabled; compiler errors,
optimization and validation are not disabled.

Two adapters under `patches/` are applied by pnpm with lockfile-bound patch hashes:

- `download-git-repo`: replace `git-clone` with option-delimited `execFile` Git
  calls, refuse option-like/control-character checkout refs and custom executables,
  preserve legitimate checkout and `.git` removal; dynamically load maintained
  `@xhmikosr/downloader`, mapping archive options into `decompress` and HTTP options
  into `got`. The vulnerable legacy archive/downloader chain is no longer installed.
  Numeric legacy timeouts are mapped to request timeouts. Callers needing other
  HTTP controls use `got`; extraction controls use `decompress`.
- Taro Webpack runner: use `html-minifier-terser` instead of abandoned
  `html-minifier`, and await async compression in both normal and independent
  package build hooks. Compilation tasks are held in a WeakMap, isolated between
  compilations. Errors reject the build; incomplete assets are not accepted.

No minimum-release-age exclusions were added. An initially investigated newer
minifier was rejected rather than relaxing that protection. Production miniapp
builds enable XML minification so the patched path is exercised, not dormant.

Validation: both production builds, workspace typechecks and frontend tests
(Trainer 111, miniapp 55, plus shared core) passed. Four local security integration
tests cover legitimate Git checkout and injection rejection, actual local HTTP
archive extraction and traversal rejection, concurrent independent-package
compression, and minifier error propagation. Test archives contain only public
synthetic data and are deterministic. See `tests/security/frontend-dependencies.test.cjs`.

## Rust — separate, not audit-cleared by the frontend result

Initial `cargo audit 0.22.2` (RustSec database revision `6de4455`) reported zero
known vulnerability entries but seven warnings: GLib unsoundness plus six
unmaintained-package warnings. The GLib output-pointer bug (RUSTSEC-2024-0429)
is mitigated by merged PR #172's exact upstream backport to vendored 0.18.5.
`tests/security/check_glib_backport.py` checks 121 upstream file identities,
exact patched source, Cargo binding and bundled notices. Debug and optimized
iterator regressions passed; [provenance](../vendor/glib/XHUP-BACKPORT.md) records
the scope. Version-based audit may still flag that unchanged version; no global
ignore is warranted.

A refreshed Rust audit on 2026-10-02 reports zero vulnerabilities and six
unmaintained-package warnings: `proc-macro-error` 1.0.4 in GTK macros, plus
`unic-char-property`, `unic-char-range`, `unic-common`, `unic-ucd-ident` and
`unic-ucd-version` 0.9.0 under Tauri's URLPattern parser. The local source-patched
GLib is not listed as a registry warning; its source gate remains mandatory.
The [source-bound maintenance migrations](rust-maintenance-patches.md) now remove
all six warnings rather than ignoring them: maintained macro diagnostics and
Unicode identifier predicates replace the retired dependency chains. The refreshed
Rust audit on this revision reports **zero vulnerabilities and zero warnings**.
CI denies warnings and checks exact vendor provenance; no blanket audit exemption
is used. This does not retroactively qualify the earlier unpatched revision. This frontend remediation does not itself close F16, native
decoder qualification, or stable-release acceptance. Windows/macOS manual testing
is delegated to users as described in README; untested results remain unknown.
