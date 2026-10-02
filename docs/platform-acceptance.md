# XHUP Flow 2.0 平台验收清单(R7/R8,2026-09)

> **历史机器可读记录**:`release/acceptance-v2.0.0.json`(schema_version 1,rc.1)。
> 原记录不补填、不升级成伪造的当前验收;仍可解析/作为 RC 预检输入,不能晋升 stable。
> 当前稳定门禁需要新采集的 schema 2 记录,见下方「原字节晋升契约」。
> 本文档的验收表格由该清单机械校验(`xhup-cli validate-acceptance` 与
> `acceptance_doc_sync` 测试),手工编辑表格而不同步清单会被 CI 拒绝。
> 状态严格四档:PASS / FAIL / UNVERIFIED / N/A —— 语义与清单一致:
> PASS 必须附证据(清单 `evidence` + `verified_at`);UNVERIFIED 是
> 稳定版(GA)发布的阻塞项,不冒充通过;N/A 仅限平台不适用的用例
> (Android 的 Trainer 桌面控制中心生命周期)。

## 验收范围

- 历史受验版本:`2.0.0-rc.1`;这不是当前 workspace 或后来 RC 的验收声明;
- 依据工件:RC Release `xhup-flow-v2.0.0-rc.1`(run 36258521537,
  SHA256 见清单 `artifacts` 与 Release 附件 `SHA256SUMS.txt`);
- 输入层新增能力(相对 v1.0.0):Flow 连续组句 + 句子级本地学习持久化、
  上下文重排序(`context_ranker`,守护开关默认重置 0 = 严格透传)、
  joint lattice 守护诊断 filter(`joint_decoder`,默认关闭)、
  运行时诊断模块;OOV 可达性与确定性/离线/隐私承诺不变。

## Windows / Weasel 真机验收记录(2026-09-27)

- 设备:WORKSTATION,Windows 11 x64,Weasel 0.17.4(rime 1.13.1);
- 升级对象:v1.0.0 活动用户目录 `%APPDATA%\Rime`(含学习 userdb 与
  无关第三方方案配置);
- 流程:外部全量备份 → 应用 `xhup-flow-rime-v2.0.0-rc.1.zip`
  (SHA256 与清单一致)→ `xhup-cli doctor` PASS →
  `rime_deployer --build`(librime 1.13.1 msvc-x64)重建 →
  WeaselServer 重载;
- 结果:build/ 中 `xhup_flow` 与 `xhup_flow_static` 的部署 schema
  version 均为 `2.0.0-rc.1`,table.bin/prism.bin 全部新编;学习 userdb
  (LevelDB)与无关用户配置升级前后逐字节保留;
- 遗留(UNVERIFIED):真机手输的 Flow 整句输入、OOV 组合与 Trainer
  桌面安装包生命周期 —— 需要人工交互验证,见 #83。

## 逐平台验收项

每个平台执行同一组用例(键名即清单 `checks` 的键):

| 用例 | Windows(Weasel) | Linux(fcitx5-rime) | macOS(Squirrel) | Android(Trime/前端) |
| ---- | ---- | ---- | ---- | ---- |
| 干净安装(全新用户目录) | PASS | UNVERIFIED | UNVERIFIED | UNVERIFIED |
| 升级安装(v1.0.0 → 2.0.0-rc.1,保留 userdb) | PASS | UNVERIFIED | UNVERIFIED | UNVERIFIED |
| 方案部署(选单启用 xhup_flow) | PASS | UNVERIFIED | UNVERIFIED | UNVERIFIED |
| 静态层冒烟(一级简码 / 单字全码 / canonical v2 词语简码) | PASS | UNVERIFIED | UNVERIFIED | UNVERIFIED |
| Flow 连续组句(整句 + 逐键) | UNVERIFIED | UNVERIFIED | UNVERIFIED | UNVERIFIED |
| 本地学习持久化(输入 → 重启 → 排名保持) | PASS | UNVERIFIED | UNVERIFIED | UNVERIFIED |
| OOV 组合可达(词表外长句,如 `提示词`) | UNVERIFIED | UNVERIFIED | UNVERIFIED | UNVERIFIED |
| context_ranker 开关关闭 = 严格透传(默认态逐字节等价) | PASS | UNVERIFIED | UNVERIFIED | UNVERIFIED |
| joint_decoder 开关关闭 = 无行为变化(默认态) | PASS | UNVERIFIED | UNVERIFIED | UNVERIFIED |
| `xhup_flow_static` 方案(无 Lua,纯静态) | PASS | UNVERIFIED | UNVERIFIED | UNVERIFIED |
| 控制中心:安装 / 修复 / 卸载 / 重新部署 | UNVERIFIED | UNVERIFIED | UNVERIFIED | N/A |
| 隐私核查(无遥测、无云端、userdb 仅本地) | PASS | UNVERIFIED | UNVERIFIED | UNVERIFIED |

## Android 发布形态(R6 决策落地项)

- 主交付:`xhup-flow-trainer-v2.0.0-rc.1-android-arm64.apk`
  (实测 139.6 MiB,单 ABI;详见 `docs/performance-baseline.md` R6 记录);
- 兼容交付:universal APK(4 ABI)保留为附加资产;
- RC 发布工作流已切换 Android job 为双产物(arm64 主 + universal 附加),
  versionCode 派生逻辑不变;
- 兼容性假设:armeabi-v7a/x86/x86_64 设备经 universal APK 兜底;
  64 位-only 设备(2020 年后主流)走 arm64 主件。

## 验收通过标准

1. `publish=false` 演练构建全平台产物,SHA256SUMS 校验一致(已达成:
   演练 #3 run 36251799648);
2. 上表所有平台至少完成「干净安装 / 升级 / OOV / 学习持久化 / 开关默认态」
   五项核心用例并如实标记;清单状态与文档同步;
3. context_ranker / joint_decoder 生产翻转(user_memory 重置 0→1 同理)
   只在核心用例全部 PASS 后单独决策,不在 RC 内静默开启;
4. stable 演练及发布均必须通过下述 schema 2 原字节门禁;任何
   UNVERIFIED/FAIL、缺证据或绑定不一致都会失败。

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
- 平台 `exemptions`:检查键到非空理由的映射,默认空对象。

`version` 是目标 stable `x.y.z`,`accepted_rc` 必须是同 core 的规范
`x.y.z-rc.N`(N >= 1,不允许前导零、构建后缀或子串匹配)。
`source_commit` 是接受 RC 的完整小写 SHA,不是文档更新提交。
`artifacts` 必须与构建清单的全部 11 个文件名/摘要精确相同。

四个平台各出现一次,各包含现有 12 个检查键,不得新增未知平台/检查键。
必查项必须 PASS。唯一 N/A 是 `android/trainer_lifecycle`,且 schema 2
必须在 `exemptions.trainer_lifecycle` 填写理由;其他 N/A 一律失败。
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
