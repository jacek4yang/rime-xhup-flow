# XHUP Flow 2.0 平台验收清单(R7/R8,2026-09)

> **机器可读事实来源**:`release/acceptance-v2.0.0.json`(schema_version 1)。
> 本文档的验收表格由该清单机械校验(`xhup-cli validate-acceptance` 与
> 文档同步测试),手工编辑表格而不同步清单会被 CI 拒绝。
> 状态严格四档:PASS / FAIL / UNVERIFIED / N/A —— 语义与清单一致:
> PASS 必须附证据(清单 `evidence` + `verified_at`);UNVERIFIED 是
> 稳定版(GA)发布的阻塞项,不冒充通过;N/A 仅限平台不适用的用例
> (Android 的 Trainer 桌面控制中心生命周期)。

## 验收范围

- 版本:`2.0.0-rc.1`(workspace / trainer / miniapp / trainer-core 一致,
  由 `product_versions_are_synchronized` 与 RC 发布工作流版本门禁双保险);
- 依据工件:RC Release `xhup-flow-v2.0.0-rc.1`(run 36258521537,
  SHA256 见清单 `artifacts` 与 Release 附件 `SHA256SUMS.txt`);
- 输入层新增能力(相对 v1.0.0):Flow 连续组句 + 句子级本地学习持久化、
  上下文重排序(`context_ranker`,守护开关默认重置 0 = 严格透传)、
  joint lattice 守护诊断 filter(`joint_decoder`,默认关闭)、
  运行时诊断模块;OOV 可达性与确定性/离线/隐私承诺不变。

## 逐平台验收项

每个平台执行同一组用例(键名即清单 `checks` 的键):

| 用例 | Windows(Weasel) | Linux(fcitx5-rime) | macOS(Squirrel) | Android(Trime/前端) |
| ---- | ---- | ---- | ---- | ---- |
| 干净安装(全新用户目录) | UNVERIFIED | UNVERIFIED | UNVERIFIED | UNVERIFIED |
| 升级安装(v1.0.0 → 2.0.0-rc.1,保留 userdb) | UNVERIFIED | UNVERIFIED | UNVERIFIED | UNVERIFIED |
| 方案部署(选单启用 xhup_flow) | UNVERIFIED | UNVERIFIED | UNVERIFIED | UNVERIFIED |
| 静态层冒烟(一级简码 / 单字全码 / canonical v2 词语简码) | UNVERIFIED | UNVERIFIED | UNVERIFIED | UNVERIFIED |
| Flow 连续组句(整句 + 逐键) | UNVERIFIED | UNVERIFIED | UNVERIFIED | UNVERIFIED |
| 本地学习持久化(输入 → 重启 → 排名保持) | UNVERIFIED | UNVERIFIED | UNVERIFIED | UNVERIFIED |
| OOV 组合可达(词表外长句,如 `提示词`) | UNVERIFIED | UNVERIFIED | UNVERIFIED | UNVERIFIED |
| context_ranker 开关关闭 = 严格透传(默认态逐字节等价) | UNVERIFIED | UNVERIFIED | UNVERIFIED | UNVERIFIED |
| joint_decoder 开关关闭 = 无行为变化(默认态) | UNVERIFIED | UNVERIFIED | UNVERIFIED | UNVERIFIED |
| `xhup_flow_static` 方案(无 Lua,纯静态) | UNVERIFIED | UNVERIFIED | UNVERIFIED | UNVERIFIED |
| 控制中心:安装 / 修复 / 卸载 / 重新部署 | UNVERIFIED | UNVERIFIED | UNVERIFIED | N/A |
| 隐私核查(无遥测、无云端、userdb 仅本地) | UNVERIFIED | UNVERIFIED | UNVERIFIED | UNVERIFIED |

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
4. 稳定版 `2.0.0` 发布由工作流门禁机械强制:清单中任何 UNVERIFIED/FAIL
   都会阻断 `publish=true` 的稳定切版(#148 §3),无人工绕过。
