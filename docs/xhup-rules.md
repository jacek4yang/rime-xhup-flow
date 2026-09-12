# XHUP 规则与兼容夹具

本层用于解释和审计，不重新分配生产码位。`xhup-core::rules` 保存小型类型化规则，
`xhup-analyzer::rules` 比较独立期望与真实生产 API；没有新增运行时依赖或网络访问。

## 证据层次与来源

来源统一使用知识库的 `data/xhup/sources.tsv`，不建立第二套 provenance 框架。
规则版本和来源 revision 分开：算法语义版本为 `1`，外部文档用响应 SHA-256 固定，
开源实现用 commit、path、blob 固定。官方事实、规则推导和产品政策不能相互冒充。

| 来源 | 核验范围 | 不代表什么 |
| --- | --- | --- |
| [官方入门](https://flypy.cc/help/xh.md) | 单字全码、定长构词、少量明确简码示例 | 完整官方词库 |
| [官方双拼](https://flypy.cc/help/up.md)、其键位图 | 声韵键位和零声母；逐键图核对 | 为特殊音码补造语言学读音 |
| [形码规则](https://flypy.cc/help/gz.md)、[拆分示例](https://flypy.cc/help/ux.md) | 首末字根、限制及少量示例 | 自动从 Unicode 字形计算所有拆分 |
| [简码说明](https://flypy.cc/help/jm.md) | 显式指派、长度约定 | 所有合法缩写均已被指派 |
| 既有 `flypy-official-ix` oracle | 嗯/诶四条特殊关系及状态 | 新的全站抓取或语言学拼音表 |
| `rime-fast-xhup` 固定字符表 | 已入知识库的历史音形配对 | 当前官网完全一致 |
| `brglng-xhup-schema` | 历史四键自动上屏实现 | 官方二进制词库内容已独立核验 |
| `rime-fast-xhup-schema` | 现代双拼/辅助形码、script translator 行为 | 官方四键词码模型 |
| `rime-double-pinyin-flypy` | 独立双拼键位、零声母及拼写别名比较 | 形码、词码证据 |
| `flow-v1-policy` | v1 发布约定、历史别名、开放组合 | 官方 Xiaohe 规则 |
| `flow-v2-compiler-policy` | 固定版本编译器源码的 F/I 语法 | 官方指派或具体受保护别名 |

本次只新增规则事实、少量必要回归示例与来源元数据，不复制网页、键位图、官方码表
或开源方案代码。官网无明确数据再分发授权，维持 `oracle-facts-only`；brglng 声明
public domain，但本次也仅记录配置比较事实。其余来源许可沿用注册表；原 NOTICE
不变。未来维护者必须固定新证据的许可、revision/hash 和提取规则，不能仅写 unknown。

## 官方兼容层

- 声码：声母键加韵母键；`zh/ch/sh` 分别为 `v/i/u`。零声母单字母重复，双字母
  保留拼写，三字母取首字母加韵母键。ASCII `v` 是归一化 ü 标签，不是新读音。
- 形码：首末两个合格字根、取大优先，必要时取笔画；相交或插入导致不独立的字根
  不能随意拆取，走之/建之位置另有约定。繁体对应字根等价不等于所有异体字等价。
  `ShapePrinciple` 显式表示这些限制；`ShapeDecomposition` 必须由证据提供。
- 单字全码：两键音码加两键形码，例如官方独立示例 `小→xnld`。
- 二字词：两个字各取前两键，例如 `双拼→ulpb`。
- 三字词：前两字各取首键，末字取前两键，例如 `输入法→urfa`。
- 四字及以上：前三字首键加末字首键，例如五字词 `他乡遇故知→txyv`。
  因此官方完整词码始终四键，不是逐字两键拼接。
- 简码：未满四键的明确指派；字符简码符合全码前缀结构，二/三字词示例可取各字
  首键。结构合法不证明词库已指派该码，也不规定自动提交。
- 特殊码直接保留 attestation。`-`/`+`/`*` 分别表示出简让全、表外字、生僻字/音，
  不把标记混入 a–z 编码。生僻字单引号前缀的完整运行语义仍待独立夹具确认。

`derive_sound` 是键盘结构组合，不是普通话音系校验；例如某声韵键的组合可结构合法
但不是音节。它不能把“嗯”的语言学 `n` 自动编为 `en`。正式输入关系仍由知识库承担。

## Flow 扩展与编译器政策

`ti + ui + ci → 提示词` 的 `tiuici` 是 Flow 顺序组合，不是官方四键词码。
按官方结构可推导 `tuci`，但没有独立词库证据时只标记 `RuleCompatible`，不宣称
官网收录。Flow 组合不查询词库，不限制文本长度；词库影响更优路径，不决定可达性。

v2 单调 `F*I*` 和历史任意 F/I 简码分别记录为 **CompilerPolicy**，不是官方规则。
`就是/jqu`、`知道/vdc`、`不是/buu`、`你们/nim`、`还是/hdu`、`因为/yww`、
`如果/rgo` 由 v1 产品政策保护；未获得外部 attestation 时不称为官方别名。

下一里程碑是 Prefix-Space Compiler v3：前缀闭合 trie 上的 `(code, rank)` 候选空间，
允许一个目标出现在多个前缀，进行全局放置优化而不是一词一码。`valid prefix !=
commit boundary`：合法前缀可继续输入，短码 rank1 不自动上屏。该 PR 不实现 v3
优化器，也不实现 mandatory Lua 或生产 contextual/Beam decoder。

永久冻结的精确菜单属于 `xhup_flow_static`。智能 `xhup_flow` 保留接受码与可达性，
后续可进行有界、可解释、有基准和回归测试的动态排序；不永久禁止上下文超过旧首选。
本次只是形式化和审计，两种方案的生产输出均不变。

## 分类与差异判定

稳定分类为 `ExactOfficial`、`OfficialAlias`、`OfficialSpecial`、`HistoricalCompatible`、
`RuleCompatible`、`FlowExtension`、`IntentionalDeviation`、`Conflict`、`Unknown`。
分类描述证据，不等于测试是否通过。每例另有 `protected / observe / known-missing /
unresolved` 期望：受保护关系失去可达性、未分类官方缺失均使 `--check` 失败；额外
Flow 路径不是失败。纯声/形推导只验证规则，不冒充生产输入关系。

目前明确两项既有缺失：`知道/vd`（Flow 保护 `vdc`）和 `他乡遇故知/txyv`。
它们必须同时引用官方事实与 `flow-v1-policy`，不能把所有缺失都自动降级为政策差异。
文本仍可开放组合。本 PR 不改码表修补；规则可推导但未 attested 的关系单独报告。
多音、多形、多码本身不构成错误；知识库保留各来源。冲突分类仍与活跃码位决议分离。

## 夹具格式与维护

`data/xhup/rules/fixtures-v1.tsv` 是独立期望，不从生成器导出：

```text
# xhup-rule-fixtures/v1
id TEXT rule components expected source class expectation policy note
```

实际使用十个 TAB 字段、UTF-8/LF、末尾换行；按 ASCII id 严格排序。无值用 `-`。
拒绝重复 id/语义、未知规则/来源/分类、非法码、错误 scalar/组件数、非规范顺序。
组件按类型解析：声 `initial,final`（零声母 `-,final`）；全码 `sound,shape`；
词码 `sound,sound,...`；字简码的独立全码；F/I 政策 `sounds;FI...`；未知 `-`。
形码为 `root:key,root:key;principle+...`，principle 按 `ShapePrinciple::ALL` 顺序且唯一。
规则来源、证据来源与可选政策来源均须在同一知识库注册表内。

维护步骤：核验外部材料并固定 hash/许可 → 更新来源 → 手工记录最小独立事实 →
解析/推导/差异测试 → 审查生产差异。不要从当前生产输出生成“官方期望”。
官方网页日后变化不影响离线构建；需显式审核后升级证据。不同期望可按不同来源共存，
不得 last-source-wins。`serialize_fixtures` 提供稳定排序与校验后的规范序列化。

```bash
cargo run --locked -p xhup-analyzer --bin xhup-rule-audit -- --summary --check
cargo run --locked -p xhup-analyzer --bin xhup-rule-audit -- --char 嗯 --json
cargo run --locked -p xhup-analyzer --bin xhup-rule-audit -- --word 提示词 --json
```

支持离线 `--fixtures TSV --sources TSV`。JSON 包含期望、生产/attested/Flow 码、
推导组件、分类、规则及证据 revision 和来源表；逐字解释复用知识库，未来 Trainer
可调用同一 API。Flow 扩展码展示首选/夹具路径，不物化所有可能组合。
全局计数仅覆盖独立夹具集，不声称穷尽官方词库；未知目标会明确计数。
即使选择单个目标，`--check` 仍检查完整输入夹具集。`--timings` 仅写 stderr。

## 回归边界

v1 发布附件的逐文件 SHA-256 固定在 `data/benchmarks/v1-artifact-hashes.tsv`，不由
当前生成器创建；独立于官方规则夹具。原有 68,842 映射承诺、菜单/librime 全量审计、
v1 计数、replay、contextual foundation 和生成器确定性门禁继续运行。
今后有意更改运行时/注释合同应在独立 PR 说明允许的文件变化，不改写历史 v1 事实，
也不解除静态映射/菜单兼容。没有宣称本次完成 glyph 引擎、官方全词库一致性或新平台真机验证。

本次夹具 49 例、受保护关系 31 条；缺失受保护关系 0、已知官方偏差 2、纯规则未
attested 2、未知 2。来源注册表 17 项（字符证据来源不变），规则夹具
7,045 B，发布哈希夹具 1,722 B。Windows release 初测：夹具解析约 0.21 ms、全局
审计约 5.01 s，主要成本是既有百万词生产索引；不加载进 Lua。并行验证期间三次
Rime 生成约 4.86/4.97/4.98 s，仅记录观测，不宣称相对旧测量的性能改善。14 个
Rime 文件仍为 36,845,122 B 且与 v1 发布逐字节一致。
