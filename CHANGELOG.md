# 更新记录

## v2.1.0-rc.1

- Trainer 默认盲打不提前展示读音、编码或正确键;共享提示权限与逐题可暂停计时,
  备用路线的错误教学、退格、重试和独立完成统计使用同一状态机。
- Linux 安装包烟测不再依赖页面泄露拼音;使用同次构建的规范数据独立作答,
  并检查真实 WebKit 的隐藏 DOM、可访问性内容和盲打进度持久化。
- DEB/RPM 分别使用原生预发布版本映射,保证 RC 早于正式版;应用内部仍保留完整
  SemVer。两个格式均执行实际 WebKit、Rust 版本 IPC 和安装内容一致性检查。
- 本地完整验收将前端安全报告写入外部证据目录,避免自身生成未跟踪文件导致
  后续干净源码门禁失败;不同工作树不得共用编译器 target。
- Lua 发布改为固定 AST 解析器验证、依赖闭包构建、注释转换和 token 等价校验;
  Lua/YAML/词典/索引/声明文件来自规范生成器,独立重建拒绝陈旧或篡改产物,不发布字节码。
- 原生查询使用构建期合法前缀/码长索引排除不可能查询,保留静态菜单与原生学习身份。
- 新增版本化跨天复习证据与到期调度,区分独立主练码回忆、迟疑、答案提示、
  错键和备用全码;同日提前重复不提升复习等级。
- 桌面与小程序共用进度更新和复习校验,V1/V2 本地记录原位迁移,V2 备份可导入,
  新导出为 V3。历史 mastery 不作为独立回忆证明。
- 修复复习入口按 2/3/4 键轮换而遗漏一级简码、长词和句子的空题池问题。
- 复习页明确区分复习等级、待验证回忆与旧掌握分数;窄屏标签换行,同分顺序不依赖系统语言。
- 小程序不再裁剪演示码表,改为完整规范 Trainer JSON 的无损离线 Brotli 资源;
  构建清单绑定源、工具和产物 SHA-256,陈旧/损坏缓存不能通过打包校验。
- 完整数据读取成功前不启动练习;原生回调式文件 API 的失败、损坏数据和重试有真实页面回归,
  规范 `去=quts` 可在复习入口盲打完成,不清除旧训练记录。
  数据门禁不再移除 Taro 页面根节点;构建后执行实际 Taro/React 产物的合成宿主回归。
- 共享索引按需物化训练项与题池排序,保留全部 ID、多音路线及跨层备用码;
  保持包体积与校验/索引初始化预算,不把 Node 预算当作微信整体启动时间。
- 正式版改为同版本独立构建、准确字节验收和封存发布,不再重命名旧 RC 产物。
- schema 3 平台 PASS 绑定当前版本、源码与构建摘要;新 RC 初始化全部 UNVERIFIED,
  不复制历史验收。正式版拒绝旧平台豁免,仍重新收集完整 16 分片与同源 CI 证明。
- 打包前强制全部产品源码版本一致;公开附件另附完整发布摘要清单,
  来源篡改、重复/额外文件、错误工作流、未验平台和旧授权均阻止发布。
- 这是预发布候选,不表示全部平台真机验收通过;实际附件和发布状态以 Releases 为准。
  PR #215 的混合简码联合搜索尚未通过性能资格,不纳入本候选版本,
  连续输入与首选质量的已知限制仍阻止宣称完整产品化完成。

## v1.0.0

XHUP Flow 首个稳定版本。

### 输入方案

- 提供 `xhup_flow`(静态输入 + 连续组句 + 本地学习)与
  `xhup_flow_static`(纯静态)两套标准 librime 方案;
- 保留 26 个一级简码、生产输入字符 2/3/4 码、固定词 4/6/8 码与
  完整码别名;
- 正式切换 optimizer v2 canonical 词语简码:
  68,842 条 = 65,909 PRIMARY + 2,933 FIXED_FIRST;
- 同码 baseline/PRIMARY/FIXED_FIRST 候选以确定性整数权重显式混排,
  不依赖词典导入顺序,不受 userdb 重排;
- 保留高频传统别名,包括`就是=jqu`、`知道=vdc`、`不是=buu`、
  `你们=nim`、`还是=hdu`、`因为=yww`、`如果=rgo`;
- 提供可选 librime-lua 简码提示;插件缺失时完整降级为纯静态输入。
- 将 8,105 字降级为 `CoreStandardHanzi` 规范子集，并从固定历史小鹤数据
  与官网回归 oracle 建立 8,208 字 `InputHanzi` / attested 编码层；恢复
  「嗯」(`ogkx`/`onkx`/`enkx`)与「诶」(`eiyu`)；
- hot 100,000 词只承担冻结排名，pinned 万象其余 1,301,434 条合法词汇
  进入 secondary Flow 层；修复旧 Top-N 外「提示词」(`tiuici`)不可达；
- Flow 组句词典加入全部两键单字音码原语，支持真正的词表外组合、语气字
  与单字/词混合长句，同时保持 frozen static exact top1 不变；
- Learn 词典通过 `import_tables` 复用完整 Flow 码表并仅补充 3/4 键单字
  原语，保证共享 userdb 的 syllable-id 一致，扩词后学习、重启、导出与
  恢复行为保持可用。

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
- librime 全量验证 141,138 个静态 exact code，并覆盖全部 extended 词、
  确定性词表外组合抽样、真实口语长句、组句、学习、
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
