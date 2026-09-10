# XHUP Flow v1.0.0 发布就绪记录

状态:**stable 1.0.0 发布基线**。本文档不再保留 RC-era 人工合并清单;
最终证据以 `main` required checks、`XHUP Flow RC Release` 正式工作流
及 GitHub Release 附件为准。当前无已知 P0/P1 blocker。

## Canonical v2

- production 词语简码唯一事实来源:
  `word_shortcuts_primary.tsv` + `word_fixed_first.tsv`;
- selected point:`rk-steep|a0.25|d0.5|x1|e-conversation`;
- 68,842 条 = 65,909 PRIMARY + 2,933 FIXED_FIRST,并集内容承诺测试无
  重复、无遗漏、无额外映射;
- PRIMARY 的 relative rank 和 merged rank 均由严格 parser 验证;
  baseline/PRIMARY/FIXED_FIRST 以唯一连续整数 Rime weight 在同一
  translator 内混排;
- `就是=jqu`、`知道=vdc`、`不是=buu`、`你们=nim`、`还是=hdu`、
  `因为=yww`、`如果=rgo` 由 generator、occupancy 与 librime session 门禁
  共同锁定为 runtime rank1;
- legacy v1 ZR/FIXED_FIRST/二码文件只留在
  `data/shortcuts/legacy/` 供 research-only 重放,不进入产品包。

## P0 输入可达性修复

- 8,105 字保留为 `CoreStandardHanzi` 规范语言学子集；生产 `InputHanzi`
  为 8,208 字，9,796 条来源证据合并为 28,851 条 2/3/4 码关系；
- `data/xhup/attested_char_codes.tsv` 来自固定
  `boomker/rime-fast-xhup@308d6d2` 快照与小范围官网 oracle，输入码事实
  不再依赖伪造 `HanziReading`；
- hot 100,000 词保持冻结，pinned 万象其余 1,301,434 条 semantic entries
  进入 secondary Flow 层；「提示词」(`tiuici`)来自上游快照而非特例；
- Flow 同一低质量语言层同时含词汇证据与 9,254 条两键单字原语，能形成
  「提嗯诶」等完全不存在于 hot/extended 词表的组合以及含语气字长句；
- Learn 通过 `import_tables` 继承完整 Flow 码表并只追加 3/4 键单字原语，
  两个 translator 共享 userdb 时不会发生 syllable-id 漂移；
- Trainer 数据契约升级到 V4，展示 core/extended scope、来源与状态。

## 可机械验证的发布门禁

- Rust:`fmt` / `check` / `clippy -D warnings` / workspace 全测试;
- replay:KdConv top-2000 基线 KSPC 1.8971、rank1 96.9544%、
  rank≤3 99.9120%、fallback 37.0093%;
- Rime:141,138 个静态 exact code 菜单前缀全量审计，全部 extended exact
  关系逐项可达，1,000 条词表外组合确定性抽样，并覆盖干净 userdb、
  学习后静态保护、无重复、真实长句、Flow 持久化/导入导出；
- runtime 哨兵:2–5 键、传统别名、legacy IF、prefix continuation、
  PRIMARY + FIXED_FIRST + baseline 精确混排菜单;
- 真实部署:`rime_deployer --build` 必须自然产出主词典与
  Flow/Learn table.bin；
- Trainer / trainer-core / miniapp:语义测试、严格 TypeScript 构建、
  weapp 构建与体积门禁;
- generation:双跑字节相等;Rime/Trainer 规范文件哈希写入
  `CANONICAL-SHA256SUMS.txt`;
- packaging:Windows NSIS + MSI、macOS universal DMG、Linux deb + rpm、
  Android universal APK、Rime ZIP、`SHA256SUMS.txt`、`BUILD-INFO.txt`;
- privacy:发布包禁止 `installation.yaml`、`user.yaml`、`sync/`、
  `*.userdb`、密钥与本机状态。

## 签名与真机状态

- Windows NSIS/MSI:**UNSIGNED**;SmartScreen 可能显示未知发布者;
- macOS universal DMG:**UNSIGNED / UNNOTARIZED**;Gatekeeper 可能需要
  右键打开或在系统设置中放行;
- Android stable publish 使用 GitHub Actions 既有 keystore secret chain,
  并在工作流中用 `apksigner verify` 验证;
- Windows 11 + Weasel 0.17.4 方案部署曾完成真机验收;
  本次 canonical v2 的 macOS/Linux/Android 真机状态仍是
  **UNVERIFIED**,不由 CI 产物构建冒充真机验证。

## 已知非阻塞限制

- 学习导出/导入依赖 librime 官方 `rime_dict_manager`;
- 学习短语码是 librime 内部派生,不读作人可读 XHUP 语义;
- Android 需手动导入平台中立 Rime 包;
- AppImage 不在 v1.0.0 产物矩阵内;
- 微信小程序由仓库构建产物交给 DevTools,不在 GitHub Release
  附件中发布。

## Stable 发布契约

`.github/workflows/xhup-flow-rc-release.yml` 以 `version=1.0.0` 运行:

1. `publish=false` 完成全平台 packaging rehearsal,不建 tag/Release;
2. `publish=true` 只允许从通过所有门禁的 `main` 运行,创建
   `xhup-flow-v1.0.0` 草稿;
3. 校验附件、SHA256、构建来源和本文档所述签名/验收状态后,
   发布为非 draft、非 prerelease 的 stable Release。
