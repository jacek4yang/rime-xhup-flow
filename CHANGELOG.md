# 更新记录

## v1.0.0

XHUP Flow 首个稳定版本。

### 输入方案

- 提供 `xhup_flow`(静态输入 + 连续组句 + 本地学习)与
  `xhup_flow_static`(纯静态)两套标准 librime 方案;
- 保留 26 个一级简码、规范单字 2/3/4 码、固定词 4/6/8 码与
  完整码别名;
- 正式切换 optimizer v2 canonical 词语简码:
  68,842 条 = 65,909 PRIMARY + 2,933 FIXED_FIRST;
- 同码 baseline/PRIMARY/FIXED_FIRST 候选以确定性整数权重显式混排,
  不依赖词典导入顺序,不受 userdb 重排;
- 保留高频传统别名,包括`就是=jqu`、`知道=vdc`、`不是=buu`、
  `你们=nim`、`还是=hdu`、`因为=yww`、`如果=rgo`;
- 提供可选 librime-lua 简码提示;插件缺失时完整降级为纯静态输入。

### 工具与应用

- Rust workspace 提供 canonical 数据模型、生成器、optimizer/replay/audit
  和 `xhup-cli`;
- Trainer 桌面/Web 应用提供 12 种练习模式、错题与统计;
- 控制中心提供 Rime 方案安装、升级、修复、卸载、重新部署、
  学习导入/导出/重置与脱敏诊断;
- 提供共享 trainer-core 与微信小程序前端。

### 质量与发布

- KdConv top-2000 replay 基线:KSPC 1.8971、rank1 96.9544%、
  rank≤3 99.9120%;
- librime 全量验证 140,666 个静态 exact code,并覆盖组句、学习、
  持久化、真实 deployment graph、Lua 与日常输入控制;
- 所有 Rime/Trainer canonical 产物可重复生成,发布包附
  `SHA256SUMS.txt`、`CANONICAL-SHA256SUMS.txt` 和 `BUILD-INFO.txt`;
- 发布产物覆盖 Rime ZIP、Windows NSIS/MSI、macOS universal DMG、
  Linux deb/rpm 与 Android universal APK;
- 发布包不含个人 Rime 状态、userdb、同步数据或签名密钥。

### 迁移

- v1 selector 的 ZERO_REGRESSION/FIXED_FIRST/二码数据已移至
  `data/shortcuts/legacy/`,仅供历史重放,不再是 production truth;
- Trainer 升级会删除不再生成的
  `xhup_flow_fixed_first_shortcuts.schema.yaml` 与
  `xhup_flow_two_key_shortcuts.dict.yaml`,不触碰用户配置与学习数据;
- 早期 `xhup_fullcode` 经典方案已从 main 收口;旧版本继续可从 GitHub
  Releases 获取。
