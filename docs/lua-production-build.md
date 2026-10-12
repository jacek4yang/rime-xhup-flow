# Lua 与 Rime 生产构建

## 数据流与边界

规范码表、规则、许可来源 → Rust generator → 严格渲染的 YAML、词典、提示数据；
`rime/lua/modules.json`、可读 Lua 源码、schema 根模块 → AST 检查、依赖闭包、可移植 Lua；
两条路径汇合为 `xhup_flow.runtime-manifest.json` 和独立构建目录，再经过语义回归、原生部署与 ZIP 校验。

- `rime/lua/` 是源码，不是安装目录。CLI 拒绝向源码树及其子目录生成，包括符号链接、尚不存在的输出目录和其他 checkout 的源码树。
- `rime/templates/` 保留便携 YAML 模板。占位符缺失、重复、未知或未闭合直接失败；用户安装后仍由 Rime 完成环境相关的部署和词典编译。
- `rime/lua/modules.json` 登记生产模块和明确排除的研究模块。未登记、缺失、重名、越界路径和符号链接失败；不以旧产物兜底。
- `native_lookup.lua` 不进入生产包。已有安装拥有的 `joint_decoder` 路径保留迁移 shim；会话隐私回归验证它清除旧选项且不成为第二个学习写入器。它不是联合解码实现。
- Trainer 数据仍由同一 Rust 规范数据生成；本次没有把 Rime 安装管理放入 Trainer，也没有更改用户词典 syllable-id。

## 编译形式

`crates/xhup-generator/src/lua_build.rs` 使用固定的 full_moon 3.0.0、上游提交
`83b6f2957ed63132abcb270dd6f75e685273c02b`，关闭默认语言扩展，检查 Lua 5.1 语法子集。
这里的语法配置不代表所有旧版 librime-lua ABI 都已验收。

编译只删除 AST 确认的注释文本，保留分隔和换行；不改标识符、字符串、数字、闭包、调用或生命周期。
输出重新解析，并逐一比较全部非 trivia token 的原始字节。每个模块带源码 SHA-256 头；运行时代码行号为源码行号加一。
不使用正则压缩，不进行表达式优化、函数级死代码删除或跨模块内联。

依赖只允许可检查的 `xhup_flow` 命名空间字面量加载，包括 `pcall(require, ...)`；动态/别名加载器及绕过加载图的环境接口被拒绝。
依赖缺失或成环失败。schema 使用 `@*module`，按依赖图输出可达模块；不是把源码目录中的所有文件复制到安装包。
这是可信仓库源码的构建约束，不是运行任意恶意 Lua 的安全沙箱。
提示数据、原生词典的前缀/码长投影和 `build_info` 在构建期生成，版本来自 workspace，而不是重复维护的 Lua 常量。

**不发布字节码。** 没有 Lua 实现、版本、架构之间的字节码兼容或启动收益证据。最终包没有 Rust、Node、AST 解析器、构建脚本或在线运行依赖。

### 解析器依赖维护

full_moon 的 MPL-2.0 许可证文本和原始 crate 来源清单保存在 `vendor/full_moon/`。
原始 crate SHA-256：`5bc70b300f9b09c409fa0a68a9f0c3e6b43573e47680d16932a0f8dade11a193`。
该归档与 Cargo registry 缓存中的 checksum 核对，源码提交与上述固定提交一致。

上游依赖的 `paste 1.0.15` 触发 `RUSTSEC-2024-0436`，不能通过现有拒绝警告门禁。
沿用仓库已有的来源绑定维护方式：两份 Cargo manifest 迁移到固定 `pastey 0.2.3`，并为只供 serde 使用的 helper 补齐 feature 条件。解析算法和上游测试样本不改，不用 lint 豁免掩盖无用代码。
`tests/security/check_rust_maintenance.py` 反向核对精确补丁、原始文件哈希、许可证和 Cargo 绑定，并拒绝旧依赖链回归；不增加 advisory 豁免。
完整上游源码在 GitHub 按 vendored 文件折叠，维护补丁可单独审查。它是生成器依赖，不是 Rime 包或 Trainer 的运行时依赖。

## 来源与可复现性

manifest 记录构建策略、解析器提交、Lua 语法目标、版本、Cargo.lock 身份、规范输入及每个输出的长度和 SHA-256。
它不自我散列；独立编译回执和最终 ZIP 的哈希绑定 manifest 自身。

`tests/release/check_generated_runtime_sources.py` 不相信待验包自带的清单：

1. 在可信源码上执行 `cargo build --locked -p xhup-cli`，从 Cargo JSON 取得真实编译器与 build script。
2. 独立调用编译器生成临时参考包，验证输入、输出清单和字节身份。
3. 核对 core、generator 与 CLI 聚合 depfile；未绑定的源码或嵌入数据直接失败。目录依赖核对完整文件清单，而不是当作普通文件跳过。
4. 比较待验包的全部文件与独立参考；改写包内 manifest 不能使篡改通过。缺少文件、同数量的外来替换、符号链接和陈旧源码均失败。
5. 记录 rustc/Cargo 版本、编译器/build script 哈希、参数、相关构建环境和源码提交；`--require-clean` 拒绝脏 checkout。结束时再次检查输入身份。

当前依赖取证运行在 POSIX 构建主机；生成的 Lua/YAML 不含主机路径，也没有平台专用字节码。Windows 的输入文本通过 `.gitattributes` 保持确定的原始字节。
编译器的只读输入明确包含实际嵌入的 KDConv 数据和许可，不扫描整个语料目录；完整官方码表及 Sogou 受限载荷不进入正常编译依赖或生产身份清单。

`rime/package/notices.json` 是安装说明、归属声明和许可证的规范登记表；生成器原样嵌入对应文本，拒绝空文件、符号链接及重复输出。这些文件与 Lua、YAML、词典一起纳入产物身份清单，不再由归档器追加。当前生成目录和 ZIP 都包含 34 个文件，其中 33 项由 manifest 记录，manifest 自身由外部回执绑定。

ZIP 清单来自独立参考编译器，而不是第二份手工文件列表。校验待验目录后，归档器仅使用私有参考目录取得的不可变字节快照，不再重读外部目录。实际 CLI 冒烟在校验后替换 `native_tail.lua`，最终 ZIP 仍保留可信编译字节；旧实现的同场景回归先失败，修复后通过。

归档使用固定顺序、时间和普通文件权限；重复成员、额外成员、加密/符号链接元数据、字节差异均拒绝。Trainer 的资源检查也使用登记表确认必需声明，不能通过同时删除资源映射与文件绕过。不要用单次 manifest 自检代替独立重建，也不要手工编辑生成文件。

## 本地验证入口

```sh
cargo run --locked -p xhup-cli -- generate rime --output /tmp/xhup-rime
python3 tests/release/check_generated_runtime_sources.py /tmp/xhup-rime \
  --target-dir /tmp/xhup-independent-target --evidence /tmp/compiler-evidence.json --require-clean
python3 tests/lua/run_runtime_suite.py /tmp/xhup-rime
python3 tests/release/rime_archive.py create /tmp/xhup-rime /tmp/xhup-rime.zip
python3 tests/security/check_rust_maintenance.py
cargo audit --deny warnings
```

证据和 ZIP 使用独占创建，不覆盖已公开的同名对象。独立 target 目录不应放进证据归档，避免把编译缓存当作发布证据。
CI、打包、全量回归及本地资格检查使用同一源产物验证和双轮 Lua 语义回归；原有原生全量分片及收集机制保留。

旧版 v1 的其他静态字节冻结不变。Lua 提示表由固定旧版原始模块执行后导出的完整语义基线约束：68,842 项，排序后的 `word<TAB>code<LF>` SHA-256 为
`53737646a32fa5239f54b4deab73f84a9b12a491682d57bf3156dcc2f59aa8ed`。
原始模块的旧哈希保存在 `data/benchmarks/v1-quick-hint-values.json`；测试不会更新它。

## 已测量的加载范围

`tests/lua/measure_module_load.py` 在同一 Linux 主机上交替启动两个包的独立 Lua 5.4.8 VM，各 100 次；按真实 schema 根模块顺序加载，禁用外部 Lua 初始化与模块路径。
记录解释器、脚本、schema 和每个 Lua 文件的哈希及每次原始样本。分位数为 nearest-rank，p50 表中使用中位数。

| 指标 | v2.0.0 固定原始包 | portable-lua-v1 首次测量包 |
|---|---:|---:|
| 全部 Lua 文件字节 | 1,805,232 | 1,796,779 |
| fresh-VM require CPU 中位数 / p95 / p99，ms | 40.4955 / 46.154 / 46.521 | 40.2135 / 48.045 / 50.604 |
| 强制 GC 后新增存活堆，KiB | 7,883.9736 | 7,884.0029 |
| 已加载根模块再次 require 一轮，中位数，µs | 0.1795 | 0.1820 |

减少 8,453 字节，约 0.47%；**没有稳定的加载加速证据，p95/p99 反而更高**。不据此扩大压缩范围。
这是温热 OS 缓存下的独立 Lua 模块加载，不是磁盘冷启动、librime session 初始化、每键延迟或 GUI 首候选显示。运行时热点优化必须另做原生配对测量。

当前管线验证不是 v2.1.0 Release，也不是 Full Regression PASS。Windows Weasel、macOS Squirrel、Android 前端的实际输入验收不能由此替代。
