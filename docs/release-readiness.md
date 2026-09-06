# XHUP Flow v1 发布就绪报告

本文件记录 v1.0.0-rc.1 发布决策前**尚未完成、需要人工执行**的事项。
CI 通过不等于发布就绪;本清单是人工控制点的权威列表。

状态截至:PR #46(chore/cross-platform-final-audit)+ RC 稳定性收口
( chore/rc-stabilization)。v1 阶段的 #25–#30 清单已由 #41–#46 跨平台
栈全面接替;本文件反映当前实测状态,不重复已自动化证据。

## 已由 CI 覆盖(不再重复人工验证)

- Rust 工作区:fmt / check / clippy(-D warnings)/ 全部测试;
- Trainer 前端:vitest 全套 + 构建(tsc 严格模式);
- librime runtime 回归:140,038 静态 exact 码审计、FIXED_FIRST 2380/2380、
  占用二码 405/405、二码 ZR 245/245、Flow 全静态等值 / 组句 / 学习持久化
  / 学习管理审计、冻结哨兵(`uij`/`uijm`/`uj`/`ujm`);
- 日常输入控制运行时验收(#43):ASCII 切换 / 中文标点 / 数字选择 /
  =- 翻页 / Escape / Enter / 空格;
- 真实部署路径守卫:临时目录 schema_list + `rime_deployer --build`,
  断言 FIXED_FIRST/Flow/Learn 三个 table.bin 由 schema/dependencies
  产出(打包 CI,防 #43 缺陷回归);
- 共享核心平台纯度守卫(node 环境,任何 DOM 依赖进核心即失败);
- 微信小程序:语义测试 + 实际 weapp 构建 + 主包/单 chunk 体积门槛;
- 跨平台产物构建与校验(product-packaging 工作流,PR 审阅产物);
- 规范数据确定性哈希(CANONICAL-SHA256SUMS.txt 跨机可比对);
- 版本同步守卫(workspace ↔ tauri.conf.json,单测强制)。

## 发布前必须人工完成

### 1. 真机冒烟(CI 无法替代)

- [x] **Windows 11 方案部署与运行时**(2026-09-06,#43 实机完成):
  Weasel 0.17.4 检出 → PM dry-run 仅含拥有文件 → install 14/14 Healthy →
  `WeaselDeployer /deploy` 成功 → `rime_probe` 26/26(一级简码 / 固定词 /
  FIXED_FIRST uij 铈→鼫→时间 / 组句「我们时间」/ 数字键穿透);
  辅助词典编译缺陷已修复并以两层回归守卫固化;
- [ ] **Windows 11 桌面应用安装流**:NSIS 安装 → 控制中心 GUI 操作 →
  学习导出/导入 → 卸载(确认 userdb 保留)—— 纯 GUI 流程仍需人工走一遍
- [ ] **macOS(arm64 与 Intel 各一)**:universal DMG 同上
- [ ] **Linux**:deb 与 rpm 各一(Fcitx5 与 IBus 各一);AppImage 未构建
- [ ] **Android**:未签名 APK 侧载(或使用 trainer-alpha 已签名产物)
  → 方案导入 fcitx5-android → 基本输入验证

### 2. 签名与公证(可选,但发布前必须显式决策)

- [ ] Windows:Authenticode 证书是否采购;不签名则发布说明必须保留
  SmartScreen 提示文字
- [ ] macOS:Developer ID 签名 + 公证,或明示「未签名,需右键打开」
- [ ] Android:发布签名走 trainer-alpha 既有密钥链路;PR 产物仅供审阅
- [ ] Linux:无需签名(deb/rpm)

### 3. 人工评审与合并顺序

当前活动栈 #41–#46(+RC 稳定性 PR):逐层增量提交数 / 文件数 / CI 状态
与 squash 合并操作顺序见 [stack-merge-playbook.md](stack-merge-playbook.md)。
早期 #25–#37 系列如尚未合并,先按 release-readiness 历史顺序自底向上处理。

### 4. v1.0.0-rc.1 发布决策(人工)

- [ ] 决定产品版本号(建议 `1.0.0-rc.1`;当前 workspace/tauri.conf 为
  0.1.0,`VERSION` 文件 1.0.0 属经典方案 `xhup_fullcode`,见
  [architecture.md](architecture.md) 版本模型一节)
- [ ] 统一升版:workspace Cargo.toml ↔ tauri.conf.json(同步修改,
  版本同步测试会强制);Rime 包版本随生成器自动内嵌
- [ ] 打 tag、创建 GitHub Release(使用 product-packaging 产出的
  SHA256SUMS / BUILD-INFO;发布路径可从 trainer-alpha 演进或独立,
  由人工执行,自动化代理不发布)
- [ ] Release 说明包含:平台矩阵、签名状态、隐私声明、已知限制
  (人读短语码未达成、学习导出依赖 rime_dict_manager、Android 手动导入、
  小程序分片为高频子集)
- [ ] 微信小程序发布物形态决策:源码仓库构建 → DevTools 上传(当前),
  或接入 CI 产出 miniprogram-ci 上传(需上传私钥,人工决策)

### 5. 文档最终核对

- [ ] README 各安装路径在真机上按文档走一遍
- [ ] `docs/legacy-fullcode-scheme.md`(冻结方案)链接可达
- [ ] NOTICE.md / LICENSE 与实际分发内容一致(尤其第三方词典授权边界)

## 明确不做(非阻塞项)

- 人读学习短语码(如 `我们时间 → wmuj`):保持 bounded research,
  不阻塞 v1;
- AppImage:外部 linuxdeploy 网络约束,deb/rpm 覆盖主流场景;
- 部署自动化按平台能力区分:Weasel/Squirrel/Fcitx5(dbus)/IBus 的
  官方机制被检测到时可自动执行,否则显示官方手动指引;
- Android 桌面端自动安装:待安全集成设计,当前仅包导出。

## 微信小程序(新增,#41–#42)

- [x] 实际 weapp 构建绿(CI 强制)+ 主包/单 chunk 体积门槛
- [x] 共享核心消费 + 数据分片(Rust 唯一来源)+ 本地进度持久化
- [ ] WeChat DevTools 视觉走查(键盘几何 / 会话滚动 / 分享文案)
- [ ] 真机预览(需测试号或个人 AppID,人工)

## 已验证 vs 待人工验证(截至 RC 稳定性)

**已自动化/实机验证**:Rust 全门禁、librime 全量运行时审计 + 日常输入
控制、Rime 源包 + 真实部署路径守卫、桌面/小程序全部测试与构建、
Windows 方案部署 + probe 26/26(#43)、桌面/小程序迁移与损坏容错。

**仍需人工/真机**:Windows 桌面应用 GUI 安装流、macOS 真机冒烟、
Linux deb/rpm + Fcitx5/IBus 真机、Android 真机复测(#45 UI 改动后)、
WeChat DevTools/真机预览、签名与公证决策、升版与发布决策。
