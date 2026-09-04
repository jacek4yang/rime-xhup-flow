# XHUP Flow v1 发布就绪报告

本文件记录 v1.0.0-rc.1 发布决策前**尚未完成、需要人工执行**的事项。
CI 通过不等于发布就绪;本清单是人工控制点的权威列表。

状态截至:PR #30(chore/v1-release-readiness)。

## 已由 CI 覆盖(不再重复人工验证)

- Rust 工作区:fmt / check / clippy(-D warnings)/ 全部测试;
- Trainer 前端:vitest 全套 + 构建(tsc 严格模式);
- librime runtime 回归:140,038 静态 exact 码审计、FIXED_FIRST 2380/2380、
  占用二码 405/405、二码 ZR 245/245、Flow 全静态等值 / 组句 / 学习持久化
  / 学习管理审计、冻结哨兵(`uij`/`uijm`/`uj`/`ujm`);
- 跨平台产物构建与校验(product-packaging 工作流,PR 审阅产物);
- 规范数据确定性哈希(CANONICAL-SHA256SUMS.txt 跨机可比对);
- 版本同步守卫(workspace ↔ tauri.conf.json,单测强制)。

## 发布前必须人工完成

### 1. 真机冒烟(CI 无法替代)

- [ ] **Windows 11**:NSIS 安装 → 控制中心安装方案 → 重新部署 →
  Flow/Static 切换 → 组句练习 → 学习导出/导入 → 卸载(确认 userdb 保留)
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

堆叠 PR 必须自底向上评审/合并(后者的 diff 以前者为基):

1. **#25** feat: 实现动态学习与长句组句引擎(`feat/flow-engine-v1` → main)
2. **#26** feat: 完善训练器数据与多层练习引擎(`feat/trainer-engine-v2`)
3. **#27** feat: 完成训练器 v1 图形化训练体验(`feat/trainer-product-v1`)
4. **#28** feat: 增加输入法安装与管理控制中心(`feat/product-control-center`)
5. **#29** ci: 建立 XHUP Flow v1 跨平台发布流水线(`ci/product-v1-packaging`)
6. **#30** chore: 完成 XHUP Flow v1 发布前质量收口(`chore/v1-release-readiness`,本 PR)

合并后如分支仍有增量,按同序 rebase/同步,保持堆叠 diff 干净。

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
  (人读短语码未达成、学习导出依赖 rime_dict_manager、Android 手动导入)

### 5. 文档最终核对

- [ ] README 各安装路径在真机上按文档走一遍
- [ ] `docs/legacy-fullcode-scheme.md`(冻结方案)链接可达
- [ ] NOTICE.md / LICENSE 与实际分发内容一致(尤其第三方词典授权边界)

## 明确不做(非阻塞项)

- 人读学习短语码(如 `我们时间 → wmuj`):保持 bounded research,
  不阻塞 v1;
- AppImage:外部 linuxdeploy 网络约束,deb/rpm 覆盖主流场景;
- 自动重新部署:各平台机制不同,控制中心提供官方指引而非自动执行;
- Android 桌面端自动安装:待安全集成设计,当前仅包导出。
