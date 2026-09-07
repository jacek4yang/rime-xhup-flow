# Input Model v2 前期调研：Lua 运行时层、参考方案与语法模型

> 调研日期：2026-09（以各仓库当时主干为准）。标注约定：**【已核实】**= 直接读过对应源码/官方文档；**【推断】**= 由间接证据得出；**【待验证】**= 需要在真机/真包上确认。
>
> 本仓库现状基线：`rime/templates/xhup_flow.schema.yaml.in` 是纯 table_translator 四层结构（primary / fixed_first / flow / learn）+ `uniquifier`，**无 Lua、无 grammar 配置**；候选次序由 FROZEN STATIC 契约冻结（`docs/architecture.md`）。

## 1. Lua 运行时层的工程形态

### 1.1 librime-lua 在各平台的真实可用性

| 平台 | librime-lua / octagram 可用性 | 证据 |
| --- | --- | --- |
| Weasel（小狼毫） | **官方安装包内置** librime-lua + librime-octagram（自 0.15.0 起） | **【已核实】** [rime.im 小狼毫更新日志](https://rime.im/release/weasel/)：“將 Rime 插件納入自動化構建流程。本次發行包含兩款插件：librime-lua · librime-octagram” |
| Squirrel（鼠须管） | **官方安装包内置** lua、octagram、predict 三款插件（自 1.0，2025-01；插件改为单独构建） | **【已核实】** [rime/squirrel releases](https://github.com/rime/squirrel/releases)：“librime 插件現單獨構建…本安裝包含 lua、octagram、predict 三[款]” |
| fcitx5-rime / ibus-rime（Linux 桌面） | **不内置**，依赖发行版把 librime 插件打为独立包，由 librime 在系统插件目录动态加载。Debian/deepin 系有 `librime-plugin-lua`、`librime-plugin-octagram`；Ubuntu noble 有 `librime-plugin-octagram`；红帽系常需手动安装 | **【已核实】** [Ubuntu packages: librime-plugin-octagram](https://packages.ubuntu.com/eu/noble/arm64/libs/librime-plugin-octagram)；[deepin 社区 apt 依赖清单](https://bbs.deepin.org/post/241786)（含 librime-plugin-lua/-octagram）；雾凇 README：“部分发行版——特别是红帽系——需要手动安装 [librime-lua]”；万象 RIME-LMDG README：“fcitx5 需要配合安装 librime-plugin-octagram，不同的 Linux 发行版包名可能不同” |
| fcitx5-android | **【已核实（间接）】** rime 插件（plugin.rime ≥ 0.0.8）可运行含 librime-lua 的雾凇全部功能，即 lua 已打包进其 rime 支持 | 雾凇安装文档平台表（[rime-ice 镜像说明](https://gitee.com/Lamentations/rime-ice)）：“Android: fcitx5-android + plugin.rime ≥ 0.0.8” |
| ibus-rime 特别提示 | librime 版本常滞后；雾凇官方建议用 [hchunhui/ibus-rime.AppImage](https://github.com/hchunhui/ibus-rime.AppImage) 跟进 librime | **【已核实】** 雾凇 README FAQ |

版本基线参考（魔然安装文档，**【已核实】**，[moran.rimeinn.org](https://moran.rimeinn.org/zh-Hans/book/maintenance/install.html)）：核心功能要求 librime ≥ 1.8.5，全部功能 ≥ 1.12；librime-lua ≥ 2023-08；兼容 Lua 5.2–5.5 与 LuaJIT。

### 1.2 官方加载机制：rime.lua 与 lua/ 目录

源码：`hchunhui/librime-lua` `src/modules.cc` 的 `lua_init()`（**【已核实】**，[modules.cc](https://github.com/hchunhui/librime-lua/blob/master/src/modules.cc)）：

1. `package.path` 追加（**注意先后顺序 = 用户目录优先于共享目录**）：
   `<user>/lua/?.lua`、`<user>/lua/?/init.lua`、`<shared>/lua/?.lua`、`<shared>/lua/?/init.lua`。
   → 这就是 `require("wanxiang.wanxiang")` 能命中 `lua/wanxiang/wanxiang.lua` 的机制，无需任何额外配置。
2. rime.lua 加载逻辑：`file_exists(user/rime.lua)` 则执行之；**否则**才执行 `shared/rime.lua`；都没有仅 `LOG(INFO)`。
   → 关键推论：**用户目录的 rime.lua 会完全屏蔽共享目录的 rime.lua（不是叠加）**。任何依赖 rime.lua 的方案都与“用户已有 rime.lua”天然冲突。

### 1.3 不触碰 rime.lua 的官方机制：`*module` 语法

源码：`src/lua_gears.cc` 的 `raw_init()`（**【已核实】**，[lua_gears.cc](https://github.com/hchunhui/librime-lua/blob/master/src/lua_gears.cc)）：

- 组件声明 `klass` 以 `*` 开头（如 `lua_filter@*pin_cand_filter`）→ 直接 `require("pin_cand_filter")`（按 §1.2 的 package.path 解析到 `lua/pin_cand_filter.lua`），与 rime.lua 无关。
- 多段 `*`（如 `lua_processor@*moran_pin*pin_processor`）→ `require("moran_pin")` 后逐级取子表成员（`sub_module_init()`），即**一个模块文件可导出多个组件入口**。
- 组件模块约定返回含 `init` / `func` / `fini` / `tags_match`（filter）的 table，或直接是函数。

这就是“与用户 rime.lua 安全共存”的正解：**完全不使用 rime.lua**。三大参考方案均如此（见 §2，三家仓库根目录均无 rime.lua）。雾凇 README FAQ 也把它作为官方推荐写法（“使用新版 librime-lua 引入模块的方式…不用修改 rime.lua”，指向 [librime-lua wiki Scripting](https://github.com/hchunhui/librime-lua/wiki/Scripting#%E6%96%B0%E7%89%88-librime-lua)）。

### 1.4 与用户已有 rime.lua 的共存结论

- require 冲突：模块名是全局命名空间，`require("wanxiang")` 若用户已有同名模块会被 `package.loaded` 缓存命中旧模块 → **模块名必须带产品前缀**（如 `xhup_flow/*`）。
- 覆盖问题：用户目录 `lua/foo.lua` 会优先于共享目录同名文件（§1.2 的路径顺序）——这是特性（用户可打补丁），也是风险（用户遗留文件可静默改变行为）。
- 若确实需要 rime.lua：没有官方合并机制；用户 rime.lua 会屏蔽共享 rime.lua。结论：**不要用 rime.lua**。

### 1.5 注册机制与性能注意事项（**【已核实】**，lua_gears.cc）

- 注册入口：`rime_lua_initialize()` 向 librime Registry 注册 `lua_processor` / `lua_segmentor` / `lua_translator` / `lua_filter` 四种组件；组件在 schema 的 engine 段落按序生效。
- Filter/Translator 的 Lua 函数跑在 **coroutine**（`newthread` + `resume`）里，天然惰性（`yield` 逐候选产出），可以中途截断。
- GC：`LuaTranslation::~LuaTranslation()` 每次析构 `gc_step(32)`、每 256 次全量 `gc()`——每次按键产生的大量短命翻译对象会持续走 GC 步进，**Lua 侧应尽量避免在 func 内构造大临时表**。
- 每次按键的调用开销：processor 是同步 `lua_->call(...)`；filter 是每候选一次 resume。大候选表扫描的代价 = O(候选数) × 每候选 Lua 工作量；惯用截断模式见雾凇 `pin_cand_filter.lua`（找齐置顶词或 others 累积 >100 即 break，再原样透传剩余迭代器，见 §2.2）。
- 一次性数据（大词表、正则）应在 `init(env)` 加载并挂到 `env` 上，不要放 func 热路径。万象 `lua/wanxiang/wanxiang.lua` 的 `compile_regex` 是极端例子：为绕开某些 Linux 机型上直接调正则接口导致 fcitx5 崩溃，改用 librime 的 `Projection` 对象做“erase 完整匹配”来实现布尔正则匹配，并在注释中说明该缓存化做法性能反而更优（**【已核实】**）。

## 2. 三个参考方案

### 2.1 万象 rime-wanxiang（amzxyz/rime-wanxiang，默认分支 wanxiang）

目录结构（**【已核实】**，GitHub API contents）：

- 根目录：`wanxiang.schema.yaml`（主方案）、`wanxiang.dict.yaml`、`wanxiang_algebra.yaml`（155KB 演算式库，内含全拼/各双拼方案标注块）、`wanxiang_symbols.yaml`、`dicts/`、`lua/`、`custom/`；**无 rime.lua**。
- `lua/` 只有两个子目录：`lua/data/`（txt 词库：emoji、english_chinese、STCharacters/STPhrases、HKVariants、TWVariants、abbrev、chengyu、tips_show 等）和 `lua/wanxiang/`（全部模块）。

`lua/wanxiang/` 关键模块与职责（按 `wanxiang.schema.yaml` engine 段落注释+文件尺寸，**【已核实】**）：

| 文件 | 职责 |
| --- | --- |
| `wanxiang.lua` (13KB) | 公共工具库：平台检测（`is_mobile_device` 靠 `rime_api.get_distribution_code_name()` + user_data_dir 路径片段 + `jit.os`）、文件回退加载（用户目录>共享目录）、Projection 正则 hack、输入法类型识别 |
| `super_processor.lua` (30KB) | processor：小键盘、字母选词、符号快打、超强分词、重复限制、退格限制、声调回退、以词定字 |
| `super_filter.lua` (26KB) | 字符集过滤+简繁权重继承 |
| `super_sequence.lua` (29KB, 导出 `*P` 和 `*F` 两个成员) | 手动排序：Ctrl+j/k/l/p 移动/重置/置顶，持久化到 `lua/sequence` userdb |
| `super_replacer.lua` (58KB) | 用 Lua+txt 词库取代 OpenCC：append/replace/comment/abbrev 四模式、流水线（s2t→t2hk）、句子级 FMM 替换 |
| `super_lookup.lua` (57KB) | 输入中反查辅助筛选（` 引导辅码查词） |
| `super_comment_preedit.lua` (21KB) | 超级注释：错词提示、辅码显示、剩余编码提示 |
| `user_predict.lua`（导出 P/T/F 三成员） | 自养预测：独立 `lua/predict` userdb、上屏后预测、上下文调频、衰减（decay_rate 0.85、expiry_days 90） |
| `context_reorder.lua` (30KB) | 上下文调频（随 user_predict） |
| `auto_phrase.lua`、`partial_commit.lua`、`key_binder.lua`、`charset_filter.lua`、`shijian.lua` (125KB 时间日期)、`super_calculator.lua` (128KB)、`super_english.lua`、`super_symbols.lua`、`input_statistics.lua`、`number_conversion.lua`、`unicode_conversion.lua`、`super_tips.lua`、`version_display.lua`、`set_schema.lua`（/zrm、/flypy 切换方案）、`userdb.lua`、`librime.lua`、`bit.lua`、`random_tools.lua` | 见名知义；注册全部用 `lua_processor@*wanxiang.xxx` 语法 |

组件挂载方式（**【已核实】**，`wanxiang.schema.yaml` engine 段）：一例即明——`lua_processor@*wanxiang.super_processor`、`lua_translator@*wanxiang.user_predict*T`、`lua_filter@*wanxiang.super_sequence*F`，同一文件多入口用第二段 `*` 成员名区分。

词库分层（**【已核实】**，`wanxiang.dict.yaml`）：`sort: by_weight`，`import_tables` 依次为 `dicts/zi`（字表）、`dicts/jichu`（基础 2-4 字词）、`dicts/lianxiang`（联想，≥5 字）、`dicts/cuoyin`（错音错字）、`dicts/duoyin`（多音兼容）、`dicts/shici`、`diming`、`yixue`、`huaxue`、`yaopin`、`mingren`、`yiren`、`wuzhong`、`renming`、`taifeng`、`fangyan`。

octagram 接入（**【已核实】**，`wanxiang.schema.yaml`）：

```yaml
grammar:
  language: wanxiang-lts-zh-hans
  collocation_max_length: 8
  collocation_min_length: 2
  collocation_penalty: -16
  non_collocation_penalty: -8
  weak_collocation_penalty: -100
  rear_penalty: -20
```

模型文件（`.gram`）不入 git 主仓，由 [amzxyz/RIME-LMDG](https://github.com/amzxyz/RIME-LMDG) releases 分发（简体 `wanxiang-lts-zh-hans` / 繁体 `wanxiang-lts-zh-hant`）。

### 2.2 雾凇 rime-ice（iDvel/rime-ice，main）

结构（**【已核实】**）：根目录 `rime_ice.schema.yaml` + 8 个双拼变体 schema、`cn_dicts/`（base/ext/tencent）、`en_dicts/`、`opencc/`、`lua/`；**无 rime.lua**。`rime_ice.schema.yaml` 全部组件用 `lua_filter@*corrector`、`lua_filter@*search@radical_pinyin` 等 `*` 语法（`*search@radical_pinyin` 说明 `*` 语法与 `@name_space` 可叠加）。

候选控制三件套（**【已核实】**，源码均读过）：

1. **置顶**：`lua/pin_cand_filter.lua`。`init` 时把 schema 配置（`编码<Tab>词1 词2…`）编译成 `env.pin_cands[preedit去空格] = {词…}`，并自动生成简拼键（`ni hao` 额外生成 `nih`；末音节 zh/ch/sh 再生成双字母简拼键；显式定义的简码键优先）。`func` 用 `pined`/`others` 两桶重排，找齐或 others>100 即截断。**性能模式值得照抄**。
2. **降频/隐藏/删词**：`lua/cold_word_drop/` 子模块目录（这同时验证了 `require("cold_word_drop/string")` 式的子目录 require）。`processor.lua` 截获 `Ctrl+d`（强制删词）/`Ctrl+x`（按码隐藏）/`Ctrl+j`（降频到第四位），用 `table.serialize` 把内存表**序列化写回** `lua/cold_word_drop/{drop,hide,reduce_freq}_words.lua` 数据文件，`filter.lua` 在候选流中执行剔除。缺点：`get_record_filername()` 里对 ibus 路径做了 `os.getenv("HOME").."/.config/ibus"` 的硬编码 hack，Weasel 下转换路径分隔符。
3. **数据 curate 流程**：`cn_dicts` 分 base（两字词+调频）/ext（多音字注音）/tencent（大词库，无注音靠 Rime 自动注音）；字表分 8105 常用字表与 41448 Unihan 大字表；维护内容是异形词/错别字校对、注音修正、词频调整，通过置顶 issue #666 收词（README“长期维护的中英词库”一节）。语法模型默认**不开**，走 `others/recipes/grammar` plum 配方打补丁接入 RIME-LMDG 模型（README FAQ，**【已核实】**）。

其他 lua：`corrector.lua`（错音错字提示，20KB）、`search.lua`（辅码查词）、`reduce_english_filter.lua`、`long_word_filter.lua`、`autocap_filter.lua`、`v_filter.lua`、`select_character.lua`（以词定字）、`lunar.lua`（附 722KB `lunar.db`）、`force_gc.lua`（强制 GC 的 translator，反向印证 Lua 侧内存是已知问题）。

### 2.3 魔然 rime-moran（rimeinn/rime-moran，main）

结构（**【已核实】**）：`moran.schema.yaml` 主方案 + `moran_fixed.schema.yaml`（固顶快码）、`moran_sentence.schema.yaml`、`moran_aux.schema.yaml`、`moran.yaml`（共享补丁库：algebra/key_bindings/octagram 段）、`moran.*.dict.yaml`（base 37MB / tencent 25MB / moe 5MB / words / charset 等）、`lua/`；**无 rime.lua**。构建用 Makefile+uv/python（`tools/gen_*.py` 生成单字码表、zrmdb、chaifen），测试用 `mira` 跑 `tests/*.test.yaml`。

快码（简快码/固顶码）与组句的交互语义（**【已核实】**，`moran.schema.yaml` + `moran.yaml` 注释 + `lua/moran_express_translator.lua` v0.12.1）：

- 架构：双 translator 由 Lua 统一包装。`moran_express_translator`（schema 中以 `lua_translator@*moran_express_translator@with_reorder` 挂载）在 init 时动态创建 `Component.Translator(env.engine, "", "table_translator@fixed")` 和 `"script_translator@smart"` 两个内部翻译器——**即 Lua 里实例化原生组件**，先查 fixed 码表、后查 smart 组句，合并输出。
- 快码何时直接作为 token 上屏：输入 <4 码时 fixed 码表输出固顶字词（带 ⚡️ 提示符，`quick_code_indicator`）；**输入 =4 码时默认“动词模式”禁止码表输出**（避免低频多字词盖住组句），切到“固词模式”（switch `inflexible`）才输出码表二字词。
- 何时继续参与组句：未选过字（`env.engine.context.input == input`）时 fixed 输出；已进入造词/造句流程（`is_sentence_making`）时默认**仍允许单字简码参与**（`quick_code_in_sentence_making: true`，0.11.0 起，配合 reorder_filter 实现）。
- 造词修复机制（该文件 0.1.0 设计动机）：用户选过字后**临时禁用 table 翻译器**，让 script_translator 看到全部输入，解决原生流程中 table/script 双翻译器互相干扰导致造词失败的问题。
- 出简让全（ijrq）：单字有简码时打全码会被推迟到 `defer` 位之后，并可提示简码打法（`moran/ijrq/*` 配置）；4 码时可用 `inject_fixed_words/inject_fixed_chars` 把快码词条注入第二候选。
- hint 相关：`moran_hint_filter.lua`（辅码/简码提示）、`enable_quick_code_hint`（如 `yy te er 英特尔` 提示 `⚡yte`）、`moran_reorder_filter.lua`（与 `@with_reorder` 配套，把 `` `F `` 标记的码表候选重排）。

octagram 接入（**【已核实】**，`moran.yaml` 的 `octagram:` 段 + `Makefile`）：

```yaml
octagram:
  enable_for_sentence:
    __patch:
      grammar: { language: zh-hant-t-essay-bgw, collocation_max_length: 4, collocation_min_length: 3 }
  enable_for_fixed:
    __patch:
      grammar: { language: zh-hant-t-essay-bgc, collocation_max_length: 4, collocation_min_length: 2 }
      translator/+: { contextual_suggestions: true, max_homographs: 7 }
```

主方案末尾 `__include: moran:/octagram/enable_for_sentence`（默认开）；模型文件 `zh-hant-t-essay-bg{c,w}.gram` 由 Makefile `wget` 自 [lotem/rime-octagram-data](https://github.com/lotem/rime-octagram-data)。**这是“模型可选挂接”的范本：配置与模型文件分离、一条 __include 开关。**

### 2.4 许可证汇总（均**【已核实】**）

| 项目 | 许可证 | 对再分发的含义 |
| --- | --- | --- |
| hchunhui/librime-lua | BSD-3-Clause | 插件本体由客户端携带，方案侧无义务；源码可自由参考 |
| lotem/librime-octagram | BSD-3-Clause | 同上；`.gram` 模型另看数据仓 |
| lotem/rime-octagram-data（essay 模型） | 仓库级需注意，**【待验证】**各 .gram 的具体许可 | 用前先确认 |
| amzxyz/rime-wanxiang | CC-BY-4.0（整仓） | 可参考；直接复制 Lua 代码需署名；词库/模型同属 CC-BY（RIME-LMDG **【待验证】**单独条款） |
| iDvel/rime-ice | **GPL-3.0-only**（README“许可证：GPL-3.0 (only) License.”） | **不能复制其 Lua 代码进非 GPL 项目**；只能参考机制与思路。词库数据亦在 GPL 覆盖范围 |
| rimeinn/rime-moran | GPLv3（LICENSE 头注明“Packaged distribution of rime-moran”；README：“完整方案发行依 GPL v3，若某文件中另有说明则依对应许可”） | 同上：只可参考；`moran.tencent.dict.yaml` 源自腾讯词库，**再分发风险更高**；GitHub API 对该仓 license 标记为 NOASSERTION，分部文件可能有例外条款，用前逐文件查头注 |

## 3. Rime 语法/组句模型（librime-octagram）

### 3.1 工作原理（**【已核实】**，[lotem/librime-octagram](https://github.com/lotem/librime-octagram) `src/octagram.cc`、`src/gram_db.*`、`src/gram_encoding.*`）

- 资源加载：schema 的 `grammar/language: <name>` → 经 librime `ResourceResolver`（资源类型 `gram_db`，后缀 `.gram`）在用户/共享目录解析 `<name>.gram`；`GramDb::Load()` 为内存映射式读取，按 language 缓存（`db_by_language_`）。模型不进内存常驻解码，只做映射查询。
- 打分（`Octagram::Query(context, word, is_rear)`）：本质是**二元文法 + 上下文退避**——把 context 末尾最多 `collocation_max_length-1` 字与 word 前部编码（`grammar::encode`），逐步缩短 context 前缀做 `GramDb::Lookup`，命中得分 = `scale(value)` + 惩罚项：搭配长度 ≥ `collocation_min_length` 用 `collocation_penalty`，否则 `weak_collocation_penalty`；句尾位置（is_rear）再查 `word + "$"` 加 `rear_penalty`；无模型或空 context 返回 `non_collocation_penalty`。
- 默认参数（源码 `GrammarConfig`）：max 4 / min 3，penalties -12 / -12 / -24 / -18。
- 接入点：语法分通过 librime 组句翻译器（script_translator 的 `contextual_suggestions` 等开关，魔然 `enable_for_fixed` 补丁即打开它）参与候选打分/调序。**模型只认“字/词文本”层面，不直接感知输入编码**——编码映射由方案的 algebra/speller 层完成，理论上拼音、双拼、形码都能挂。

### 3.2 万象的模型格式与工具链（**【已核实】**，RIME-LMDG README）

- 格式即 librime-octagram `.gram`；万象基于 32GB 多领域语料训练，配同仓带调拼音词库使用；构建教程在 [amzxyz/rime-build-grammar-word-frequency](https://github.com/amzxyz/rime-build-grammar-word-frequency)（wiki）；效果评测用 gaboolic/rime-schema-compare（雾凇挂模型后句对率 61.7%→75.4%，万象 62.3%→76.9%）。
- **关键声明（原文）**：“基于拼音权重排序序列优化，对形码无特殊处理，因此只推荐运用在以拼音序列为基础的词库中……原则上不支持形码。” → 万象模型不能直接拿来给 XHUP 用。

### 3.3 形码/exact-code 方案接入语法模型的典型方式与已知坑

- 典型接入（**【已核实】**）：只把 grammar 挂在**组句层 translator**（魔然挂在 smart；万象挂在主 script_translator），静态/固顶码表层不挂；模型文件与方案分离分发，用 `__include` 或补丁开关。XHUP 现状天然匹配这一形态：`xhup_flow` 的 `flow` translator（`enable_sentence: true` + `user_dict: xhup_flow_user`）就是挂点，primary/fixed_first 静态层不动。
- 已知坑：
  1. 权重平衡是词库-模型联合调参结果（万象明示只有万象词库能发挥最佳），换词库必须重调 penalty（万象自己就把默认 -12/-12/-24/-18 调成了 -16/-8/-100/-20）。
  2. `contextual_suggestions` 与连续组句/用户词典互相干扰：万象在 `user_dict_set` 注释里明确关掉它（“又要预测又要连续句子，放一起很难优雅”）（**【已核实】**，schema 注释）。
  3. 模型热词可能压制 exact code 的确定性——对 XHUP 的 FROZEN STATIC 契约是红线：语法分只能作用于 flow 组句候选层，绝不允许上浮到静态层之前（架构上已由 translator 分层 + initial_quality 栅栏保证，但要守住不引入“跨层重排”的 Lua）。
  4. `.gram` 体积大（百 MB 级）且与 librime-octagram 插件版本相关；Linux 发行版插件缺失时 schema 含 grammar 段的影响（报错/静默降级）**【待验证】**——建议像雾凇/魔然一样做成可选补丁而非默认开启。

## 4. 对 XHUP Flow 的落地建议

### 直接可用（照抄机制，不抄代码）

1. **零 rime.lua 的模块机制**：schema 里 `lua_filter@*xhup_flow.xxx` / `lua_processor@*xhup_flow.yyy*member`，模块放 `lua/xhup_flow/`（自动进 package.path，§1.2/1.3）。这同时解决“用户已有 rime.lua”的共存问题，也符合控制中心 OWNED_FILES 只增不碰的约束。注意模块名必须带 `xhup_flow` 前缀防 require 撞名。
2. **雾凇 pin_cand_filter 的 filter 写法**：init 编译配置为哈希表、func 两桶重排+截断（>100 透传）。任何 Lua 候选干预都用这个骨架。
3. **魔然的可选挂接范式**：octagram/功能开关做成独立 yaml 段 + `__include`/补丁，模型文件独立分发、默认关。
4. **平台检测**：`rime_api.get_distribution_code_name()` + `get_user_data_dir()`（万象 wanxiang.lua 已验证的 API 集），比路径猜测可靠的部分直接用。
5. **组件内实例化原生 translator**（魔然 `Component.Translator(env.engine, "", "table_translator@fixed")`）——如果 v2 需要“Lua 编排多个原生翻译器”，这是已验证路径。
6. 发布门禁可仿魔然：用真实 librime 跑部署级测试（魔然用 mira + tests/*.test.yaml；本仓库 CI 已有 librime 实机编译门禁，可扩展行为断言）。

### 需要适配

1. **octagram 模型**：万象模型声明不支持形码，不能直接采购。路径有二：(a) 用 lotem/rime-octagram-data 的 essay-bgw（繁体训练，简体场景效果**【待验证】**）；(b) 用 rime-build-grammar-word-frequency 工具链基于自有语料自训简体模型。无论哪条，penalty 参数要对着 XHUP 词库重调，并先在 `xhup_flow_static` 做 A/B。
2. **语法分只进 flow 层**：grammar 配置加在 flow translator 所在 schema，但需实测确认 octagram 的调分不改变 primary/fixed_first 候选次序（契约红线；若语法分会跨层渗透则放弃挂接或改走“组句候选追加”模式）。
3. **降频/隐藏类功能**：机制参考雾凇 cold_word_drop，但存储不要写回 lua 数据文件——写独立 userdb 或受控目录文件，避免污染 OWNED_FILES 里的 Lua 源。
4. **用户词典学习**：万象 user_predict 的独立 userdb + 衰减/过期设计与 `xhup_flow_user` 身份冻结契约需对齐；若做，作为新 translator/新 db 追加，不改 `xhup_flow_user` 语义。

### 不要学

1. 万象的巨型单文件模块（super_calculator 128KB、shijian 125KB、super_lookup 57KB；moran_shijian 86KB）——部署体积与可维护性都差；XHUP 的 Lua 应小而专。
2. 雾凇 cold_word_drop 的平台路径硬编码 hack 与“把 lua 源文件当数据库写回”的做法。
3. 万象 `is_mobile_device` 靠路径片段（`/sdcard/` 等）猜测平台的脆弱启发式。
4. 万象 super_replacer 用 Lua+txt 全面取代 OpenCC——重造轮子且文本库无索引，XHUP 如有简繁/表情需求直接用原生 simplifier/OpenCC。
5. 雾凇“清空用户目录再整体复制”的分发方式——与 XHUP 控制中心“只写 OWNED_FILES、绝不触碰用户其它配置”的安全不变量根本冲突。
6. 雾凇（GPL-3.0-only）与魔然（GPLv3）的代码不可复制进本仓库（本仓 LICENSE 非 GPL）；万象（CC-BY-4.0）可复制但须署名并记录来源——优先全部自研、仅借鉴机制。
