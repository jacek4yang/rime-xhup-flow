# 显式底层运行时资格发布策略

默认 `full-platform` 模式不变：稳定版要求完整平台验收。另一个模式
`runtime-qualified-user-platform-testing-v1` 仅供仓库所有者明确授权发布，
不是“所有平台测试通过”，也不是通用跳过检查开关。

## 必须满足的门禁

- 仍从已公开、非 draft、同主版本的 RC 晋升原字节；不重编产品，内嵌版本仍为 RC。
- 独立解析 RC 标签的源 SHA，逐项核验 schema 2、完整工件列表、sealed
  BUILD-MANIFEST 与实际下载附件大小/SHA256；任何 FAIL 仍拒绝发布。
- CI、Full Regression、RC Release 三次不同成功运行，均绑定同一 RC 源 SHA，
  来自本仓库 main 的 push 或 workflow_dispatch；不接受 PR、其他仓库或其他源。
- CI 六个必需任务全部成功，拒绝缺失/分页截断/跳过/不同源的任务。
- 下载完整 native-audit-input、16 个分区的原始日志和收据；使用现有收集器重新
  `collect`，然后逐字段比较发布的 coverage.json，校验 revision/run attempt。
  不接受只有摘要或“总运行绿了”。全量静态、扩展词和开放组合覆盖仍是必需项。
- 工作流从 GitHub 独立获取仓库 owner，必须等于实际 github.actor。
  审批记录的 approved_by 不能代替工作流身份检查。
- 授权记录与验收清单中的 UNVERIFIED 平台集合必须完全一致且非空；
  未验项目原样保留，未知状态、重复平台、无理由 N/A 等旧校验继续有效。

## 授权记录（不是平台证据）

真实 RC 和成功运行出现后，提交 `release/qualification-v<VERSION>.json`：

- `schema_version: 1`，`policy: runtime-qualified-user-platform-testing-v1`；
- `version`、`accepted_rc`、`source_commit`、`build_manifest_sha256`：实际精确绑定；
- `repository`、`approved_by`、`approved_at`（UTC）：实际上下文与授权时间；
- `rationale`、非空 `limitations`：真实授权范围及已知限制；
- `user_testing_platforms`：验收清单中仍有 UNVERIFIED 的平台；
- `ci_run`、`runtime_run`、`package_run`：三个实际 GitHub Actions 运行 ID。

本实现不生成占位审批或虚假 PASS。合成 fixtures 仅用于测试。
工作流 dispatch 显式选择策略，默认仍为 full-platform；选择新模式不会改变
源 RC 内容或平台状态。稳定版附加 QUALIFICATION.json 与
QUALIFICATION-PROOFS.tar.gz，Release 正文明示范围和各平台状态。

## 限制与用户测试

Windows/macOS 人工安装与真实输入法体验由用户验证，不能使用跨平台编译或
打包成功替代。Windows/macOS 未签名，macOS 未公证；Android 只沿用既有
签名密钥链，不替换密钥。Linux 虚拟 Fcitx 输入实验尚未通过，底层 librime
通过并不证明真实桌面输入上下文正常。本次任务明确不把该图形实验作为发布门禁。

独占仅指 Rime 方案选单，不删除系统输入法。安装原配置替换、备份和卸载恢复
须按安装文档执行；手动安装需要自行保管原 default.custom.yaml。
历史完整平台验收和本策略不得混写，任何新源提交均须重新取得同源证明。
