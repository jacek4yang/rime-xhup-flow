# XHUP Flow v1.0.0 发布就绪记录

状态:**stable 1.0.0 发布基线(历史存档)**。v2.0.0 的发布就绪状态见
下方「v2.0.0 GA 就绪记录」;v1 部分保留作对照,不再更新。

本文档不再保留 RC-era 人工合并清单;
最终证据以 `main` required checks、`XHUP Flow RC Release` 正式工作流与
GitHub Release 附件为准。

## Canonical v2

- production 词语简码唯一事实来源:
  `word_shortcuts_primary.tsv` + `word_fixed_first.tsv`;
- selected point:`rk-steep|a0.25|d0.5|x1|e-conversation`;
- 68,842 条 = 65,909 PRIMARY + 2,933 FIXED_FIRST,并集内容承诺测试:
  无重复、无遗漏、无额外映射;
- PRIMARY 与 relative rank、merged rank 均由严格 parser 验证;
  baseline/PRIMARY/FIXED_FIRST 以唯一连续整数 Rime weight 在同一
  translator 内混排;
- `就是=jqu`、`知道=vdc`、`不是=buu`、`你们=nim`、`还是=hdu`、
  `因为=yww`、`如果=rgo` 由 generator、occupancy 与 librime session 门禁
  共同锁定为 runtime rank1;
- legacy v1 ZR/FIXED_FIRST/二码文件只存在于
  `data/shortcuts/legacy/`,仅 research-only 重放,不进入产品包。

## P0 输入可达性修复
- 8,105 字保留为 `CoreStandardHanzi` 规范语言学子集;生产 `InputHanzi`
  为 8,208 字,9,796 条来源证据合并为 28,851 条 2/3/4 码关系;
- `data/xhup/attested_char_codes.tsv` 来自固定
  `boomker/rime-fast-xhup@308d6d2` 快照与小范围官网 oracle,输入码事实
  不再依赖伪造 `HanziReading`;
- hot 100,000 词保持冻结,pinned 万象其余 1,301,434 条 semantic entries
  进入 secondary Flow 层;「提示词」(`tiuici`)来自上游快照而非特例;
- Flow 同一低质量语言层同时含词汇证据与 9,254 条两键单字原语,能形成
  「提嗯诶」等完全不存在于 hot/extended 词表的组合以及含语气字长句;
- Learn 通过 `import_tables` 继承完整 Flow 码表并只追加 3/4 键单字原语,
  两个 translator 共享 userdb 时不会发生 syllable-id 漂移;
- Trainer 数据契约升级为 V4,展示 core/extended scope、来源与状态。

## 可机械验证的发布门禁

- Rust:`fmt` / `check` / `clippy -D warnings` / workspace 全测试;
- replay:KdConv top-2000 基线 KSPC 1.8971、rank1 96.9544%、
  rank≤3 99.9120%、fallback 37.0093%;
- Rime:141,138 个静态 exact code 菜单前缀全量审计,全部 extended exact
  关系逐项可达,1,000 条词表外组合确定性抽样,并覆盖干净 userdb、
  学习后静态保护、无重复、真实长句、Flow 持久化、导入导出;
- runtime 哨兵:2 字 4 键、传统别名、legacy IF、prefix continuation、
  PRIMARY + FIXED_FIRST + baseline 精确混排菜单;
- 真实部署:`rime_deployer --build` 必须自然产出主词典与
  Flow/Learn table.bin;
- Trainer / trainer-core / miniapp:语义测试、严格 TypeScript 构建、
  weapp 构建与体积门禁;
- generation:双跑字节相等;Rime/Trainer 规范文件哈希写入
  `CANONICAL-SHA256SUMS.txt`;
- packaging:Windows NSIS + MSI、macOS universal DMG、Linux deb + rpm、
  Android APK、Rime ZIP、`SHA256SUMS.txt`、`BUILD-INFO.txt`;
- privacy:发布包禁含 `installation.yaml`、`user.yaml`、`sync/`、
  `*.userdb`、密钥与本机状态。

## 签名与真机状态
- Windows NSIS/MSI:**UNSIGNED**;SmartScreen 可能显示未知发布者;
- macOS universal DMG:**UNSIGNED / UNNOTARIZED**;Gatekeeper 可能需要
  右键打开或在系统设置中放行;
- Android stable publish 使用 GitHub Actions 既有 keystore secret chain,
  并在工作流中以 `apksigner verify` 验证;
- Windows 11 + Weasel 0.17.4 方案部署曾完成真机验证;
  本次 canonical v2 后 macOS/Linux/Android 真机状态仍为
  **UNVERIFIED**,不由 CI 产物构建冒充真机验证。

## 已知非阻塞限制
- 学习导出/导入依赖 librime 官方 `rime_dict_manager`;
- 学习短语码是 librime 内部派生,不读作人可读 XHUP 语义;
- Android 需手动导入平台中立 Rime 包;
- AppImage 不在 v1.0.0 产物矩阵内;
- 微信小程序由仓库构建产物交给 DevTools,不在 GitHub Release
  附件中发布。

## Stable 发布契约(v1)

`.github/workflows/xhup-flow-rc-release.yml` 以 `version=1.0.0` 运行:

1. `publish=false` 完成全平台 packaging rehearsal,不建 tag/Release;
2. `publish=true` 只允许从通过所有门禁的 `main` 运行,创建
   `xhup-flow-v1.0.0` 草稿;
3. 校验附件、SHA256、构建来源和本文档所述签名/验收状态后,
   发布为非 draft、非 prerelease 的 stable Release。

---

# v2.0.0 GA 就绪记录(#148 R8,2026-09)

## Phase 1A 工程检查点

- 审计基线: `8b2eaf1879d4649c36701182a5ab30eaccdb5752`。
- 范围仅 F02:验收 schema 2、真实 RC 附件/源 SHA 绑定、负向回归与发布门禁。
- 历史 schema 1 清单原样保留;不能用于 stable,不补造硬件证据。
- 验证:acceptance 单元 12/12、provenance integration 35/35(其中一项执行
  9/9 实际 shell 块/模拟 GitHub 回归)、历史 doc-sync 3/3 通过。
- fmt、workspace check/clippy(all-targets/locked/offline,warnings denied)通过;
  最终 affected-scope clippy 再跑通过;工作流 Bash 62 块通过/非 Bash 3 块跳过,
  发布工作流 YAML 解析通过。
- 全 workspace tests 尝试在链接期因 `Disk quota exceeded` 退出 101,不是 PASS。
  随后的 focused 尝试也因相同配额失败;保留所有缓存,改用独立磁盘 target/TMPDIR
  后上述 50 项 Rust + 9 项 Python 回归全部通过。没有运行真实发布/签名/平台验收。
- LSP 未运行成功:配置的 Rust 工具链缺 rust-analyzer;可安装该组件或修正
  pi-lsp.json 命令,本任务未改工具配置。未验证平台保持 UNVERIFIED。
- 不涉及运行时/词典/学习/Trainer 行为;不合并 PR,不发布版本。

### Phase 1A 验证命令账本

Cargo 使用隔离 target;配额失败后还将 TMPDIR 指向磁盘隔离目录,不清理原缓存。
`--offline` 使用已有依赖;新增直接 SHA256 依赖复用 lockfile 已锁定的 sha2 0.10.9。

| 命令/验证 | 结果 |
| --- | --- |
| `cargo fmt --all -- --check` | PASS(最终) |
| `cargo check --workspace --all-targets --locked --offline` | PASS |
| `cargo clippy --workspace --all-targets --locked --offline -- -D warnings` | PASS |
| `cargo clippy -p xhup-cli --all-targets --locked --offline -- -D warnings` | PASS(最终 affected scope) |
| `cargo test -p xhup-cli --lib acceptance --locked --offline` | 12 PASS,0 FAIL,21 filtered |
| `cargo test -p xhup-cli --test acceptance_provenance --test acceptance_doc_sync --locked --offline` | 35 + 3 PASS,0 FAIL(最终) |
| `XHUP_CLI=<built-cli> python3 tests/release/test_acceptance_workflow.py` | 9 PASS,0 FAIL;也由 Rust integration 调用 |
| `python3 tests/release/check_workflow_shell.py` | 62 Bash PASS,3 非 Bash skipped |
| Python/PyYAML `safe_load` 发布工作流 | PASS,4 jobs |
| `timeout 300s cargo test --workspace --all-targets --locked --offline -- --test-threads=1` | FAILED exit 101,链接时磁盘配额耗尽;未完成,非超时/PASS |
| `git diff --check` | PASS(最终) |
| LSP diagnostics | UNAVAILABLE:缺 rust-analyzer |

中间失败也保留:首次 `cargo check -p xhup-cli --offline` 暴露 hasher 未 finalize 的
编译错误,已修正并由上述检查覆盖;一次 diff-check 尾空白已修正。
全套尝试耗尽临时空间后,组合 focused 命令无法编译(exit 101);
第一次仅切换 target 的 focused 重跑仍有 30 个配额引起的测试失败。
同时切换 TMPDIR 后最终全绿,没有放宽断言。真实 GitHub 发布、签名及硬件验收未执行。

状态:**GA 冲刺中**。机器可读验收清单 `release/acceptance-v2.0.0.json`
为唯一事实来源;本节与清单、`docs/platform-acceptance.md` 由
`acceptance_doc_sync` 测试与 `xhup-cli validate-acceptance` 门禁机械
约束,不得漂移。

## 稳定范围(冻结,#148 §1)

- v1.0.0 全部能力 + 2.0 新增:Flow 连续组句、句子级本地学习持久化、
  上下文重排序(`context_ranker`)、joint lattice 守护诊断
  (`joint_decoder`)、运行时诊断模块;
- **不新增产品特性**;canonical/static 兼容、确定性输出、离线/隐私、
  `xhup_flow_static` 回退、既有 accepted mappings 与 OOV 可达性全部保持;
- 实验运行时特性保持守护/默认关闭(`context_ranker`、`joint_decoder`
  的开关默认重置 0 = 严格透传/无行为变化);
- 任何已发布代码/schema/运行时/打包变更在最终 RC 验证后 → 必须重切
  `2.0.0-rc.N+1`(#148 §7),已验证 RC 的 tag/产物不可变。

## 兼容性与不变量

- canonical v2 词语简码(68,842 条)与静态层映射在 2.0 内字节级冻结;
  CI 以 `v1_baseline_snapshot` 归一版本戳对比冻结 v1 哈希,词表/映射
  内容漂移会直接失败;
- OOV 可达性:`提示词`(`tiuici`)等词表外组合由上游事实层 + 两键
  原语生成,不依赖特例;librime 全量审计与抽样审计持续门禁;
- 确定性:同 Cargo.lock + 源码 → 产物字节相等(`CANONICAL-SHA256SUMS.txt`);
- 隐私:全部产物本地运行,无遥测、无账号、userdb 仅本机;发布包隐私
  文件禁含清单在 CI 强制。

## 运行时默认与守护开关

| 特性 | 默认状态 | 承诺 |
| ---- | ---- | ---- |
| `context_ranker` | 关闭(重置 0) | 关闭态 = 严格透传,逐字节等价 |
| `joint_decoder` | 关闭(重置 0) | 关闭态 = 无行为变化 |
| `user_memory`(学习) | 启用 | 快照/重启持久化;RC 验收通过后另行决策生产翻转 |
| `xhup_flow_static` | 独立方案 | 无 Lua、纯静态回退,始终可用 |

## 基准与运行时审计(2.0 周期)

- replay 基线与 runtime 哨兵同 v1(见上),Full Regression 工作流在
  main 持续强制;librime 全量/抽样审计通过;
- Android 端到端延迟与内存:R6 阶段完成构建形态实测
  (universal 582.1/551.0 MiB → arm64-only 139.6 MiB,−74.7%,
  见 `docs/performance-baseline.md`);运行时帧内指标未建立,列为
  已知限制而非阻塞。

## 打包矩阵与签名状态

- Windows:NSIS + MSI(UNSIGNED);
- macOS:universal DMG(UNSIGNED / UNNOTARIZED);
- Linux:deb + rpm;AppImage 暂不构建;
- Android:**arm64-only APK(主产物,SIGNED)** + universal APK
  (兼容附加,SIGNED);versionCode 单调派生;
- 平台中立 Rime 源包 zip(stable 原字节晋升时内嵌版本保持接受 RC);
- 封存与验收:`BUILD-MANIFEST.json` / `ACCEPTANCE.json`;
- 原有清单三件套:`SHA256SUMS.txt` / `CANONICAL-SHA256SUMS.txt` /
  `BUILD-INFO.txt`(逐产物如实标注签名状态);
- 签名策略(#148 §9):不以 Windows/macOS 签名缺失阻塞 GA;
  元数据如实标注;绝不为发布弱化 OS 安全控制或提交签名凭据。

## 平台验收(#148 §4)

- 唯一事实来源:`release/acceptance-v2.0.0.json`(schema_version 1,
  状态严格四档 PASS/FAIL/UNVERIFIED/N/A);
- 文档映射:`docs/platform-acceptance.md` 由 doc-sync 测试机械约束;
- 四平台 × 12 检查项(干净安装/升级/部署/静态冒烟/Flow 组句/学习
  持久化/OOV/两开关中性/static 回退/Trainer 生命周期/隐私);
- RC 发布(publish=true,`*-rc.N`)允许 UNVERIFIED;**稳定发布
  (publish=true,`2.0.0`)要求 schema 2 必查 PASS、唯一有理由 Android N/A、
  精确源/构建清单/附件字节绑定**,
  失败逐行输出阻塞平台/检查项,无人工绕过(#148 §3);
- 硬件不可用的平台保持 UNVERIFIED 并如实列为外部阻塞;CI 构建成功
  不冒充真机验证。

## 已知非阻塞限制(v2 新增)

- 学习导出/导入依赖 `rime_dict_manager`(同 v1);
- AppImage、微信小程序 Release 附件不在矩阵(同 v1);
- Android 运行时帧内指标(内存/延迟)未建立基线;
- `joint_decoder` 仅守护诊断(证据收集 + 有界提升),完整有界波束
  接入真实菜单为 2.0 后演进;
- `context_ranker`/`user_memory` 生产翻转决策在 GA 后单独评估。

## RC → GA 晋升规则(#148 §7/§8)

1. 最终 RC 验证后冻结 shipped 内容;发现缺陷 → PR + CI/Full
   Regression → 递增 `2.0.0-rc.N` 重切,受影响验收重跑;
   **文档/证据类变更不需重切**;
2. GA:`publish=true, version=2.0.0` 仅原字节晋升已接受 RC,不重建;
   schema 2 绑定确切源 SHA、BUILD-MANIFEST.json 摘要及完整附件,
   独立解析 RC tag 并校验实际下载字节;详见 `docs/platform-acceptance.md`。
   历史 schema 1 不满足此门禁;附件名称及内嵌版本保持 RC;
3. 创建 `xhup-flow-v2.0.0` 草稿(非 prerelease),发布说明的
   平台状态取自清单真实数据;人工复核后发布;
4. 全程无人工绕过;最终证据回填 #83。

## 验证分层声明

| 层级 | 覆盖 | 例子 |
| ---- | ---- | ---- |
| 自动化验证(CI) | 词典/映射/学习/打包/升级语义 | Full Regression、`v1_to_v2_*` fixture 测试、doc-sync |
| 真机验证(人工) | 部署 + 输入手感 + 前端交互 | `release/acceptance-v2.0.0.json` 平台条目 |
| 已知限制 | 不阻塞 GA 的缺口 | 本节列表 |
| 发布阻塞 | 缺一不可发布 | UNVERIFIED/FAIL 的平台验收项 |
