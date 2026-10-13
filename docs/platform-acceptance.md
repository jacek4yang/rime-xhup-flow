# XHUP Flow v2.0.0 平台验收与发布范围

机器可读事实来源：`release/acceptance-v2.0.0.json`（schema 2）。
发布决定：`release/qualification-v2.0.0.json`。

## v2.0.0 历史范围（不得沿用于新发布）

- 产品源提交：`2c54d99f3a6f0792cf345450186dddc7b490325c`。
- 正式版从 `2.0.0-rc.3` 原样晋升，不重新构建安装包。
- 仓库所有者批准 `runtime-qualified-user-platform-testing-v1` 范围：
  必须通过同一源提交的 CI、16 分片全量原生回归和 RC 发布构建；
  具体运行及原始证明由正式发布流程独立抓取和校验。
- 所有人工平台用例仍为 **UNVERIFIED**，不代表全面真机验收通过。
  完整平台验收策略仍是默认；本次使用明确授权的受限发布范围。
- Trainer 只负责练习和训练记录。系统输入法和方案安装、部署、
  更新及卸载由用户按[手动安装指南](install-guide.zh-CN.md)完成。
- Windows 未签名，macOS 未签名且未公证；Android 使用现有发布签名。
- 原 RC.1 记录原字节保存在 `release/history/acceptance-v2.0.0-rc.1.json`，
  不把其中的旧 PASS 套用到本次产物，也不沿用历史 Android N/A。

## v2.0.0 逐平台人工验收记录

PASS 需要对应平台证据和验证时间；FAIL 不可被此策略覆盖；
UNVERIFIED 表示未验。当前 schema 2 的训练器生命周期覆盖四个平台，
不接受 N/A。自动化 Linux WebKit 测试与原生 Rime 回归另行保存，
不把它们冒充用户设备的安装、更新或输入验收。

| 用例 | Windows(Weasel) | Linux(fcitx5-rime) | macOS(Squirrel) | Android(Fcitx5/Rime) |
| ---- | ---- | ---- | ---- | ---- |
| 干净安装（全新用户目录） | UNVERIFIED | UNVERIFIED | UNVERIFIED | UNVERIFIED |
| 升级安装（保留用户词库及无关配置） | UNVERIFIED | UNVERIFIED | UNVERIFIED | UNVERIFIED |
| 方案部署（启用 xhup_flow） | UNVERIFIED | UNVERIFIED | UNVERIFIED | UNVERIFIED |
| 静态层冒烟（一级简码 / 全码 / 词语简码） | UNVERIFIED | UNVERIFIED | UNVERIFIED | UNVERIFIED |
| Flow 连续组句（整句及逐键） | UNVERIFIED | UNVERIFIED | UNVERIFIED | UNVERIFIED |
| 本地学习持久化（输入、重启及排名保持） | UNVERIFIED | UNVERIFIED | UNVERIFIED | UNVERIFIED |
| OOV 组合可达性 | UNVERIFIED | UNVERIFIED | UNVERIFIED | UNVERIFIED |
| context_ranker 默认关闭时严格透传 | UNVERIFIED | UNVERIFIED | UNVERIFIED | UNVERIFIED |
| joint_decoder 默认关闭时无行为变化 | UNVERIFIED | UNVERIFIED | UNVERIFIED | UNVERIFIED |
| xhup_flow_static 无 Lua 回退 | UNVERIFIED | UNVERIFIED | UNVERIFIED | UNVERIFIED |
| 训练器练习、进度、备份和重启恢复 | UNVERIFIED | UNVERIFIED | UNVERIFIED | UNVERIFIED |
| 隐私（本地运行、无遥测） | UNVERIFIED | UNVERIFIED | UNVERIFIED | UNVERIFIED |

## 当前发布契约：独立版本构建与重新验收

以上 v2.0.0 记录保留历史事实，不是后续版本的授权或验收。
新正式版不能通过重命名旧 RC 附件发布，不能继承旧平台 PASS 或豁免。

### 先构建，再验收同一批字节

1. 将 Cargo workspace、Tauri、Trainer、小程序和 trainer-core 的完整版本统一到
   目标版本。`check_product_version.py` 在所有平台构建之前拒绝版本覆盖和漂移。
2. RC 由 RC Release 工作流构建；正式版本先独立运行 Product Packaging，
   开启 `sign_android`、`arm64_default`。Windows/macOS 签名状态仍须如实记录。
3. 打包完成时 `xhup-cli seal-build` 写入不可覆盖的 `BUILD-MANIFEST.json`
   （schema 2）。固定清单为 8 个包和 SHA256SUMS、CANONICAL-SHA256SUMS、
   BUILD-INFO 三个元数据文件。每个文件必须存在、非空、非符号链接，字节 SHA-256 受约束。
4. `xhup-cli init-acceptance` 重新核对实际封存目录，生成 schema 3 验收模板；
   所有平台初始为 UNVERIFIED，不读取旧验收记录。模板作为独立
   `release-acceptance-template` artifact 下载，不混入已封存负载。
5. 对这些准确字节完成真实平台验收，然后提交 `release/acceptance-v<core>.json`
   和 `release/qualification-v<version>.json`。验收记录提交可以晚于产品构建；
   `source_commit` 必须仍指向实际构建源码，不是记录提交。

产品版本升级后必须重新构建和重新验收。验收完成后则不能再次打包：
时间戳、签名或其他构建差异都可能改变字节，不能用“源码基本相同”替代身份验证。

### schema 3 平台收据

顶层保留 `version`、`source_commit`、`build_manifest_sha256`、完整 `artifacts`
和四个平台的十二个检查项。正式版的 `accepted_rc` 仅记录同 core 的前序 RC，
不代表其字节或平台验收可被继承。

任何 PASS 平台收据必须有：

- `tested_build.version`：当前产物的完整版本；
- `tested_build.source_commit`：当前构建源码 SHA；
- `tested_build.build_manifest_sha256`：受验封存清单原始字节的 SHA-256；
- 实际 `frontend`、`os`、`architecture`、`runtime`；
- 非空 `evidence` 和有效 UTC `verified_at`。

前三项必须与顶层身份完全一致。正式版全部检查项均须 PASS；
FAIL、UNVERIFIED、N/A、旧 schema、缺失证据均阻塞，没有平台豁免选项。
程序能验证绑定和结构，不能替维护者证明截图、日志或人工操作真实。

### 发布时的独立证明

正式发布传入独立 `package_run`。工作流从 GitHub 获取其真实源码 SHA，
拒绝失败、其他工作流、其他仓库、非 main 或非 workflow_dispatch 的构建；
下载完整六组产物并拒绝重复文件；来源校验只使用可信源码构建的校验器，不运行附件提供的程序。
前序 RC 必须已公开，正式源码必须在其后，不能用同一 RC 提交冒充正式源码。

随后核验 schema 3、当前版本、构建清单、真实附件字节、完整平台收据，
以及[同源 CI 与全量原生证明](runtime-qualified-release.md)。全部通过才创建草稿。
tag 指向实际产品构建 SHA，附件名和内部版本均为目标版本。
RC 发布则生成当前构建的新 UNVERIFIED 收据，不复制历史 PASS。

`ACCEPTANCE.json`、正式版资格记录和证明在封存验证后附加，避免循环哈希。
原 `SHA256SUMS.txt` 不再改写；`RELEASE-SHA256SUMS.txt` 覆盖公开附件及新增元数据
（不包含校验和文件自身）。已经公开的同名 tag/Release 不覆盖。

### 验证入口

- `cargo test -p xhup-cli --lib acceptance`
- `cargo test -p xhup-cli --test acceptance_provenance`
- `python3 tests/release/test_product_version.py`
- `XHUP_CLI=<已构建CLI> python3 tests/release/test_acceptance_workflow.py`
- `python3 tests/release/check_workflow_shell.py`

合成附件和模拟 GitHub 仅验证拒绝边界，不是签名、平台或真实发布验收。
