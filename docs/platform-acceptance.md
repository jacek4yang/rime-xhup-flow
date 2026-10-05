# XHUP Flow v2.0.0 平台验收与发布范围

机器可读事实来源：`release/acceptance-v2.0.0.json`（schema 2）。
发布决定：`release/qualification-v2.0.0.json`。

## 当前范围

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

## 当前逐平台人工验收项

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

## 原字节晋升契约(Phase 1A / F02)

这是当前工作流实际执行的契约,不是对上述历史记录补造证据。

### 构建封存

签名 RC 的产品打包完成后,`xhup-cli seal-build --version <x.y.z-rc.N>
--source-commit <40位SHA> --artifacts-dir <dir>` 生成
`BUILD-MANIFEST.json`。此命令不会覆盖已存在的清单,只接受完整的
8 个发布包 + `SHA256SUMS.txt`、`CANONICAL-SHA256SUMS.txt`、
`BUILD-INFO.txt`;名称排序、SHA256 流式计算,同源同字节输出相同。

清单 schema 1 包含 `schema_version`、`version`、`source_commit`、
`artifacts: [{name, sha256}]`。源码 SHA 绑定代码、锁文件和数据版本;
已有 BUILD-INFO/规范哈希文件也作为受哈希的附件保留。这不是签名证明、
工具链可复现性证明或新增通用溯源系统。unsigned RC 演练不封存为可晋升工件。

### 新验收记录(schema 2)

保留原顶层字段及平台检查键,仅新增:
- 顶层 `build_manifest_sha256`:已受验 BUILD-MANIFEST.json **原始字节**
  的 64 位小写 SHA256;
- 平台 `runtime`:受验 librime、Lua(或明确无 Lua)、客户端及版本;
- 平台 `exemptions`:保留的历史字段；当前 schema 2 不接受豁免，应为空对象。

`version` 是目标 stable `x.y.z`,`accepted_rc` 必须是同 core 的规范
`x.y.z-rc.N`(N >= 1,不允许前导零、构建后缀或子串匹配)。
`source_commit` 是接受 RC 的完整小写 SHA,不是文档更新提交。
`artifacts` 必须与构建清单的全部 11 个文件名/摘要精确相同。

四个平台各出现一次,各包含现有 12 个检查键,不得新增未知平台/检查键。
默认完整平台门禁要求必查项 PASS；当前 schema 2 不接受 N/A，
Android 的训练器生命周期也要实际验收。历史 schema 1 的桌面控制中心豁免
只用于读取旧记录，不能作为当前稳定晋升证据。明确授权的平台待验策略
可保留 UNVERIFIED，但不得将失败或未测改写为 PASS。
每个平台的 PASS 必须附非空 `evidence` 和真实有效的 UTC
`verified_at`(`YYYY-MM-DDTHH:MM:SSZ`,校验日历日期)。
来源、平台/架构/客户端/运行时身份均不可为空。
这些字段记录审阅后的证据,程序不能替人验证日志/截图是否真实或覆盖完整。

### 工作流与 CLI 强制边界

`validate-acceptance --stable` 必须同时传
`--expect-version`、`--expect-source`、`--artifacts-dir`;
缺少外部绑定参数直接拒绝,没有默认 stable 版本。
`--expect-source` 必须来自接受 RC tag 的独立解析,不能复制 JSON 声明。

发布工作流:
1. RC 仍允许 UNVERIFIED;拒绝 FAIL、非法结构和未允许的 N/A。
   schema 1 的 Android 生命周期理由缺失仅在历史 RC 预检容忍。
   历史清单若版本/源 SHA 不匹配当前 RC,发布说明所有平台显示 UNVERIFIED。
2. stable **不重新打包**。从非 draft 的 prerelease 接受 RC 下载全部附件,
   经 GitHub commit API 解析 tag 到源提交,验证清单哈希、源、版本、全部
   附件的实际字节、集合完整性,拒绝多余文件、空文件和符号链接。
3. 只上传验证后的 payload 供发布 job 消费。任何门禁失败都不能创建 stable
   草稿;演练同样执行门禁。stable tag 指向 RC 的源 SHA。
4. 附件名和应用内嵌版本**保持 RC 原样**;stable 是这些字节的正式发布标签,
   不伪装为重编译的 stable 二进制。若必须改变内嵌版本,此路径不支持,
   必须另行设计/审核等价证明,当前不允许绕过。
5. `ACCEPTANCE.json` 单独附带此次审阅记录,不纳入构建清单以避免循环哈希。
   已下载 RC 的旧 ACCEPTANCE.json 仅在隔离附件目录中替换;构建字节不修改。

已有 rc.1/rc.2 若没有封存清单不能 retroactively 自动晋升。不得仅对旧附件
重新计算摘要并宣称它们已经受验;需要新 RC 封存及真实 schema 2 证据。
当前历史记录不能使 GA 门禁通过,未执行平台继续 UNVERIFIED。

### 可执行回归

- `cargo test -p xhup-cli --test acceptance_provenance`:合成附件正向对照、
  F02 全 N/A/假 SHA/无关 RC/缺证据重现,实际字节/清单/集合篡改与 CLI 参数。
- `cargo test -p xhup-cli --test acceptance_doc_sync`:历史记录及表格一致性。
- `tests/release/test_acceptance_workflow.py`:实际 prepare shell 块,
  使用真实 CLI 和隔离模拟 GitHub(无网络);覆盖 RC/演练/stable 分支及失败门禁。
  Unix Rust integration test 自动调用;单跑时指定 `XHUP_CLI` 为构建后的 CLI。
- `python3 tests/release/check_workflow_shell.py`:工作流 shell 语法。

这些是发布契约测试,不冒充 GitHub 真实发布、签名或四平台验收。
