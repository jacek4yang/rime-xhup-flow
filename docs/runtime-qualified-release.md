# 正式版完整平台与同源运行时资格

当前策略为 `full-platform-rebuilt-release-v1`。它补充平台验收，不能豁免平台验收。
v2.0.0 的 `runtime-qualified-user-platform-testing-v1` 是历史特批；
旧记录保留，但新工作流已移除此选项，旧授权不能用于新版本。

## 两阶段发布

1. 在 main 固定目标**正式版本**源码，统一全部产品版本。
2. 对同一完整 SHA 运行 CI、Full Regression 和独立 Product Packaging；
   打包启用 Android 发布签名及 ARM64 主包，得到正式版本的准确字节。
3. 下载封存包和 `release-acceptance-template`，对这些字节执行各平台实际验收。
   未具备硬件、权限或签名条件时保留 UNVERIFIED；不得假装签名或平台验证已完成。
4. 提交真实验收及资格记录。随后 dispatch RC Release，传入正式 `version`、
   `package_run` 与 `publish=true`。该阶段只核验和发布已验收字节，不再次构建。

产品、manifest 与 tag 版本必须一致。前序 RC 只用于记录版本关系；
正式源码必须在已公开 RC 源码之后，不复用 RC 安装包。
构建附件保留 30 天；过期或丢失时必须重新构建并重新验收，不能补造旧字节身份。

### Linux 原生版本与升级顺序

产品源码、应用内版本、tag 和附件名仍使用完整 SemVer。
Linux 包管理器不能直接把 `2.1.0-rc.1` 当成预发布：DPKG/RPM 会把它排在
`2.1.0` 之后。因此 DEB 使用 `2.1.0~rc.1`，RPM 使用 Version `2.1.0`、
Release `0.rc.1`；正式版分别使用 `2.1.0` 和 `2.1.0-1`。
映射来自 `linux_package_versions.py`，BUILD-INFO 记录具体原生字段。

先以完整产品版本编译未打包应用，保存原始二进制，随后分别打包 DEB 和 RPM。
RPM 的 core-only 配置只传给 `tauri bundle`，不得传给 Rust 编译；
各格式打包前使用原始二进制，避免前一个格式消耗 Tauri 的 bundle-type 标记。
DEB 控制字段转换后重新比较全部安装路径、文件内容、权限、属主和链接，
不覆盖已有输出。两种实际安装包均提取运行 WebKit、Rust 版本 IPC、
训练持久化和许可证门禁，不能仅凭文件名判断版本正确。

Ubuntu 24.04 的 RPM 4.18.2 `rpm2cpio` 要求 `LONGARCHIVESIZE`，当前构建器没有
写入该可选字段；即使已输出完整 payload，该工具也以 1 退出。
打包门禁先用 `rpmkeys --checksig` 严格验证头部与 payload 摘要，再用同版本
`rpm2archive --nocompression` 与 tar 提取；任何一步非零都失败，不忽略退出码。
原生 4.18.2 工具的实际验证包括损坏 payload 拒绝、21 个安装文件的内容/权限/
链接一致，以及提取应用的 26 项 WebKit 检查。此结果不是 Windows/macOS/Android 验收。

## 必需证明

- [schema 3 平台收据](platform-acceptance.md)：四个平台全部检查项 PASS，
  `tested_build` 与当前版本、源码及 BUILD-MANIFEST 摘要一致。FAIL、UNVERIFIED、
  N/A、旧 schema、无证据均拒绝。
- CI、Full Regression、Product Packaging 三次不同成功运行，来自本仓库 main，
  精确绑定同一构建 SHA；打包必须是独立 workflow_dispatch。
- CI 六个必需任务全部成功，拒绝缺失、分页截断、跳过和不同源任务。
- 下载完整 native-audit-input、16 个分区的原始日志和收据，用现有收集器重新
  collect，比较 coverage.json，并核验 revision/run attempt。
  全量 Static、Extended 和 OOV 覆盖不能用 focused 回放替代。
- 独立读取 GitHub repository owner，并匹配实际 github.actor；
  JSON 中的 `approved_by` 不能替代工作流身份。
- 当前版本 BUILD-MANIFEST schema 2、全部封存文件实际字节和精确清单均通过验证。
  来源校验不执行附件提供的程序；实际平台使用验收另在目标环境中运行受验安装包。

## CI 测试发现与实际 CLI

CI 的发布行为套件使用 `test_*.py` 统一发现，不维护容易失效的单文件名列表。
2026-10-14 的运行 `38497295414` 仍调用已删除的
`test_packaging_runtime_gates.py`，Python 报 `NO TESTS RAN` 并退出 5；
这是工作流缺陷，不是可忽略的环境失败。

执行前构建当前源码的 `xhup-cli`，通过 `XHUP_CLI` 显式传入绝对路径。
测试实际调用 CLI 校验来源、封存字节和平台授权；缺少 CLI 不能跳过。
不恢复只检查工作流文本的旧测试，不把零测试退出码改成成功。
修复后重新运行同源 CI，保留此前失败记录，不改写旧运行的源 SHA 或结果。


## 资格记录

`release/qualification-v<VERSION>.json`：

- `schema_version: 2`，`policy: full-platform-rebuilt-release-v1`；
- `version`、`accepted_rc`、`source_commit`、`build_manifest_sha256`；
- `repository`、`approved_by`、真实 UTC `approved_at`；
- `rationale`、非空 `limitations`，如尚未提供桌面签名，但不得将未验平台列为豁免；
- `ci_run`、`runtime_run`、`package_run`，且 package_run 等于 dispatch 输入。

记录是审阅决定，不制造 PASS。正式版附加 QUALIFICATION.json 和
QUALIFICATION-PROOFS.tar.gz，另以 RELEASE-SHA256SUMS.txt 覆盖公开附件。
生成草稿不代表已经公开；维护者仍须核对当前资产、版本、限制与下载后哈希。

## 验收边界

Linux librime 回放不能证明 Windows Weasel、macOS Squirrel 或 Android 前端实际输入正常。
编译、合成 DOM、虚拟组件宿主也不能替代当前安装包的真实目标平台验收。
不能要求用户关闭系统安全防护。正式门禁未满足时，只能保留草稿或发布如实标注的 RC。
