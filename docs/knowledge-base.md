# 多源字符知识库

里程碑 2 为离线证据与审计层；生产字符、词汇、菜单与 Rime 文件保持兼容。
本 PR 不实现上下文评分、Lua 合同或用户学习。

## 分层

```text
固定上游 / 既有策展 TSV
    → generator::knowledge 的显式适配器
    → KnowledgeEvidence（独立读音、音码、形码、配对全码、派生码、频率、变体）
    → 来源/状态校验 → resolved production facts
    → 既有确定性生成器 → Rime / Trainer
```

`xhup-core::knowledge` 承载身份、证据类型与字符输入关系决议，不做候选排序。
`xhup-generator::knowledge` 组装现有表的离线视图；`xhup-analyzer::knowledge`
报告缺口、差异与来源。没有新增依赖，也没有第二套入库的大型规范表。

`CharacterId` 是单个 Unicode 标量的身份，不等于 core 或 production 成员。
`ReadingEvidence` 保存归一化语言学读音；`SoundCodeEvidence` 保存两键输入码，
二者不能互换。`FullCodeEvidence` 保留同一来源证明的音形配对及输入权重；
独立音码与形码投影不会自动产生笛卡尔积。`GeneratedCodeEvidence` 明确标记
现有 core 读音 × core 形码算法的结果，附读音与版本化推导来源。
`FrequencyEvidence` 区分语言学读音分数与输入权重；`VariantEvidence` 支持有向
简体/繁体/兼容/其他关系，目前没有导入任何变体记录，不从字形或编码猜测变体。

## 来源清单与许可

机器注册表：[`data/xhup/sources.tsv`](../data/xhup/sources.tsv)。

| source id | 现有证据 | 固定版本 | 许可/用途 |
| --- | --- | --- | --- |
| core-readings | pinyin-data 的 kMandarin_8105 与 kTGHZ2013 策展并集；同时支撑 data/pinyin 的音节范围 | `923b108d` | MIT，保留 data/hanzi/UPSTREAM_LICENSE |
| rime-fast-xhup | 全部历史音形配对；data/shape 是同源 core 投影 | `308d6d29`，blob `3c277333` | LGPL-3.0，既有 NOTICE 链 |
| flypy-official-ix | 已存在的四条官网回归事实 | payload SHA-256 `d33e6afa…` | oracle-facts-only，不是网站数据再分发许可 |
| wanxiang-characters | 8,544 条规范读音频率分数 | `7ec998b2`，blob `9a69cb89` | CC-BY-4.0，独立署名不变 |
| wanxiang-words | hot/extended 共用一份上游，沿用现有词汇模块 | `4618d67a`，blob `a0f66e2f` | CC-BY-4.0 |
| flow-core-composition | 已冻结的 core 编码算法及布局/音节输入 | 本仓库 `ab93235` | 项目算法 LGPL-3.0-only；不替代依赖数据自身许可 |

完整提交、路径、可用 blob/SHA-256、提取算法版本、用途、优先级与限制均在注册表。
`-` 只用于确实未记录的可选 hash，不能替代来源版本、许可或提取版本。
core 读音的原始一次性提取脚本未入库：这是已有来源链限制，本次不声称补齐了原始
上游重建器，也不虚构每个 primary/alt 行的单一上游归属。适配器版本是对现有
规范化规则的命名；修改规则必须升级版本与复核差异。

Wanxiang 的语义上游 RIME-LMDG 不是另一份独立佐证；hot 和 extended 不重复计作
两个来源。词汇行不复制进字符证据视图；范围与来源仍可由注册表及既有词汇审计追踪。
Unihan、其他许可明确的读音/XHUP 实现可通过相同来源表与类型加入，目前未新增下载。
zdic.net、zi.tools、hanyuguoxue.com 没有被抓取或导入；新增来源必须先核对许可、
署名、固定版本与可复现性，oracle 不能被重标为可再分发数据。

## 确定性格式

全部格式为 UTF-8、无 BOM、LF、末尾换行；拒绝空白行、字段不足/多余、非规范
十进制、溢出、未知枚举、多个 Unicode 标量，以及重复或乱序数据。字符不做
Unicode 归一化；增补平面按同一规则处理。

来源表 `# xhup-knowledge-sources/v1` 后每行 12 个 TAB 字段：

```text
id kind url revision path blob sha256 license extractor usage priority notes
```

source id 严格升序；路径为上游相对路径，拒绝本机绝对路径与 `..`。
来源 kind 与 usage 是枚举，许可为已审核白名单，优先级为 u16。

归一化交换格式 `# xhup-knowledge-evidence/v1` 后每行 7 个 TAB 字段：

```text
kind character value detail source status confidence
```

| kind | value | detail |
| --- | --- | --- |
| reading | 小写 ASCII 无调读音 | primary / alt |
| sound / shape | 两键码 | `-` |
| full | 四键配对码 | u32 来源输入权重 |
| generated | 四键推导码 | 真实贡献读音 |
| frequency | u64 分数 | 真实读音，或 input-weight |
| variant | 目标字符 | simplified / traditional / compatibility / other |

按全部字段的 UTF-8 字节序排序；同来源的相同语义事实不可通过更换 status、confidence、
full 权重或 frequency 分数重复插入。serializer 先排序再严格重解析，拒绝无效状态。
原有六字段 attested TSV 通过专门的兼容适配器读取，不改变其入库字节或 extractor。
该旧格式没有 confidence 列，按现有已审核事实赋 high；低/中置信度使用新交换格式。

交换导出是可复现工具产物，**不提交**。原始 full 行完整保留；sound/shape 是投影，
同来源同码去重，优先保留更高状态，同级按枚举顺序决胜。不把重复投影计成独立佐证。

## 冲突和决议

全局保留所有证据。`deprecated`、`uncertain` 或 `low` confidence 不成为生产激活关系。
来源引用必须存在；official 类状态只能来自 oracle kind；generated 状态只能来自
版本化 project kind。官网的小范围事实沿用原有用途，不导入站点数据库。

字符允许多个读音、多个形码和多个输入别名。因此冲突报告列出多个值，不自动删除
“输掉”的编码。确定性决议是按 `(character, full code)` 合并可信支持证据，并为
**同一关系**选择首选来源：

1. 状态：official（含 yield-full/outside-core）> legacy-compatible > attested > alternate > generated；
2. confidence：high > medium；
3. source priority：大者优先；
4. source id 字节序，最后 status 枚举顺序决胜。

全部可信别名仍可生产激活；没有 last-source-wins。这个顺序用于溯源解释，不是
Rime 菜单次序。既有生成器继续保护 core 静态顺序、合并来源集合、取历史最大输入
权重，并按原有投影生成 2/3/4 键关系。

`InputHanzi` 现在消费这个配对编码决议的激活关系。生产初始化不再断言 8,208 字
或 9,796 行；仍校验规范 core 全覆盖。测试保留当前计数，另用不进入真实数据的
合成标量，调用同一 production membership builder，证明增加一条可信配对证据
即可扩展集合。无需调整枚举或字符数上限。只有读音或孤立音/形证据尚不能成为
生产输入；审计会明确报告这些缺口。

## 审计与维护

```bash
cargo run --locked -p xhup-analyzer --bin knowledge-audit -- --check
cargo run --locked -p xhup-analyzer --bin knowledge-audit -- --json
cargo run --locked -p xhup-analyzer --bin knowledge-audit -- --char 嗯
cargo run --locked -p xhup-analyzer --bin knowledge-audit -- --char 嗯 --json
cargo run --locked -p xhup-analyzer --bin knowledge-audit -- --export-tsv
cargo run --release --locked -p xhup-analyzer --bin knowledge-audit -- --timings
```

`--evidence FILE --sources FILE` 审计外部离线规范 TSV，不替换生产数据。
默认 summary；JSON 同时包含全部 finding。报告可信但未支持、音缺形/形缺音、
音/形/配对全码多值、core 外字符、重复证据、孤立/非法来源和高频无输入。
高频条件为任一可信 ReadingScore ≥ `--high-frequency N`（默认 100000），不将
不同来源分数相加。该报告只覆盖已导入证据，不声称涵盖未导入上游的字符。
`--check` 对重复证据与非法 provenance 失败；多音多形等差异不是自动失败。
CI 还运行 production full 关系等值、全局计数与序列化回归。

添加来源：先审核许可和固定输入，登记 sources.tsv，再用离线适配器将来源记录映射
到独立关系类型。保留配对和来源，不从输入音码反推拼音。使用合成夹具测试格式、
冲突、优先级和导出确定性；审查 global/character 报告后才显式接入生产决议。
现有 attested/万象原始来源重生成命令沿用对应 data README；无需联网构建或测试。

## 当前基线与性能

| 项目 | 改造前 | 改造后 |
| --- | ---: | ---: |
| core / production 字符 | 8105 / 8208 | 8105 / 8208 |
| attested 行 / 去重配对 | 9796 / 9794 | 9796 / 9794 |
| 生产 2/3/4 键关系 | 28851 | 28851 |
| canonical v2 映射 | 68842 | 68842 |
| Rime 文件 / 字节 | 14 / 36845122 | 14 / 36845122 |

前后生成包逐文件 SHA-256 比较零差异。完整视图：8580 读音、17770 音码投影、
8775 形码投影、9796 attested 全码、9159 带贡献读音的 generated 关系、8544 频率，
变体 0；合并后的生产全码 9873。多值字符：sound 926、shape 564、full 1391；
可信但未支持、音形缺口、重复证据与非法来源均为 0。

新增入库来源表 2,000 字节；Windows 本机 release 初测：归一化交换导出 3,409,932 字节，适配约 12 ms，
解析约 45 ms，审计含文本输出约 75 ms。计时只在显式 `--timings` 时写 stderr，
不进入规范输出；不作跨机器硬门槛。完整视图不进入 Lua，生产启动只解析较小的
配对表和来源表；Rime 包大小和 runtime 字节未改变。

本机三次 release Rime 生成（不计编译）为 4,573 / 4,485 / 4,531 ms；与 #84
记录的 4.50–4.62 s 同量级。完整生成器确定性与 byte comparison 另由测试验证。

局限：core 原始提取脚本缺失；源差异尚需人工判定是合法多值还是错误；尚未导入
Unihan 变体或更大字符集；未来新来源的生产接入仍需显式审核。里程碑 2 已随 #85 合并。
后续 [XHUP 规则与兼容夹具](xhup-rules.md) 复用同一来源注册表，新增规则文档/实现比较
来源但不改生产证据或码表；上方性能是 #85 的历史测量，不代表扩充后的来源表大小。
