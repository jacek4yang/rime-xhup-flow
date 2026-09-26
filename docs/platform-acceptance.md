# XHUP Flow 2.0.0-rc.1 平台验收清单(R7,2026-09)

状态:**RC 候选验收清单**。2.0.0-rc.1 的平台验收按本清单逐项标记,
全部证据以 RC Release 附件、CI 工作流结果与本清单的人工复核记录为准。
本清单是 `docs/release-readiness.md`(v1.0.0 基线)在 2.0 周期的延续。

## 验收范围

- 版本:`2.0.0-rc.1`(workspace / trainer / miniapp / trainer-core 一致,
  由 `product_versions_are_synchronized` 与 RC 发布工作流版本门禁双保险);
- 产物:`xhup-flow-rime-v2.0.0-rc.1.zip` + 五平台 trainer 安装包
  (Android 按 #143 R6 决策,arm64-only 为主要交付形态);
- 输入层新增能力(相对 v1.0.0):Flow 连续组句 + 句子级本地学习持久化、
  上下文重排序(`context_ranker`,守护开关默认重置 0 = 严格透传)、
  joint lattice 守护诊断 filter(`joint_decoder`,默认关闭)、
  运行时诊断模块;OOV 可达性与确定性/离线/隐私承诺不变。

## 逐平台验收项

每个平台执行同一组用例,状态三档:PASS / FAIL / UNVERIFIED。

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
| 控制中心:安装 / 修复 / 卸载 / 重新部署 | UNVERIFIED | UNVERIFIED | UNVERIFIED | N/A(移动端为学习管理) |
| 隐私核查(无遥测、无云端、userdb 仅本地) | UNVERIFIED | UNVERIFIED | UNVERIFIED | UNVERIFIED |

## Android 发布形态(R6 决策落地项)

- 主交付:`xhup-flow-trainer-v2.0.0-rc.1-android-arm64.apk`
  (实测 139.6 MiB,单 ABI;详见 `docs/performance-baseline.md` R6 记录);
- 兼容交付:universal APK(4 ABI)保留为附加资产;
- RC 发布工作流需切换 Android job 为双产物(arm64 主 + universal 附加),
  versionCode 派生逻辑不变;
- 兼容性假设:armeabi-v7a/x86/x86_64 设备经 universal APK 兜底;
  64 位-only 设备(2020 年后主流)走 arm64 主件。

## 验收通过标准

1. `publish=false` 演练构建全平台产物,SHA256SUMS 校验一致;
2. 上表所有平台至少完成「干净安装 / 升级 / OOV / 学习持久化 / 开关默认态」
   五项核心用例并如实标记;
3. context_ranker / joint_decoder 生产翻转(user_memory 重置 0→1 同理)
   只在核心用例全部 PASS 后单独决策,不在 RC 内静默开启。
