# Lua 运行时策略层架构(XHUP Flow)

状态:**quick_hint 已落地并经验证**(#59,CI librime runtime integration 全绿);
其余模块按本文规划推进。依据:docs/research-runtime-and-references.md;
本文只记录决策、权衡与不变量。

## 0. 定位

Rust 负责全局、确定性、可复现的离线计算(静态层与简码映射);Lua 只做
**轻量、有界、可降级的运行时策略**:

- 不重排静态冻结候选(FROZEN STATIC 契约红线);
- 不实现离线优化器的任何职责;
- 不产生网络请求、不读写用户隐私数据外泄;
- Lua 不可用的环境必须完整回退到纯静态行为。

## 1. 加载机制:零 rime.lua

决策:**不使用 rime.lua**。组件一律用 librime-lua 的 `*module` 语法注册:

```yaml
engine:
  filters:
    - lua_filter@*xhup_flow.quick_hint
  processors:
    - lua_processor@*xhup_flow.candidate_control*P   # 一个文件多入口
```

依据(已核实,librime-lua `modules.cc` / `lua_gears.cc`):

- `package.path` 自动包含 `{user,shared}/lua/?.lua` 与 `?/init.lua`,
  `require("xhup_flow.quick_hint")` 命中 `lua/xhup_flow/quick_hint.lua`;
- 用户目录 `rime.lua` 会**屏蔽**(非叠加)共享目录 `rime.lua` —— 依赖
  rime.lua 即与「用户已有 rime.lua」天然冲突;
- 万象/雾凇/魔然三家全部零 rime.lua,这是成熟惯例。

模块命名空间固定 `xhup_flow/*`,防止与用户既有 Lua 模块 require 撞名。

## 2. 平台可用性与降级

| 平台 | librime-lua |
|---|---|
| Weasel ≥ 0.15 | 官方安装包内置 |
| Squirrel ≥ 1.0 | 官方安装包内置 |
| fcitx5-android(plugin.rime ≥ 0.0.8) | 内置 |
| fcitx5-rime / ibus-rime | 依赖发行版 `librime-plugin-lua` 包(Debian/Ubuntu 有,红帽系常需手动) |

降级语义:schema 引用 `lua_filter@*...` 而插件缺失时,librime 记录错误并
跳过该组件,其余组件正常工作。**已验证**(CI librime job,#59):冒烟审计
先在**无插件**环境跑完整 A/B(组件被跳过、全部输入行为完整),再安装
`librime-plugin-lua` 跑 Lua 审计;`*module` 语法在 Ubuntu noble 的
librime-lua(git20230917)可用。

### 已验证的 filter 顺序不变量(#59 真机教训)

`lua_filter` 必须排在 `uniquifier` **之前**(rime-ice 等同惯例)。
librime-lua git20230917 的协程 translation 在 lua_filter 位于 uniquifier
之后时,会把每个菜单的末位候选多回吐一次(91,782 个码出现 `[X^_X]`
形态重复);librime 1.10 的 uniquifier 按 text-only 去重,无法消除位于其
上游产生的重复。模板 `rime/templates/*.yaml.in` 已固化该顺序并注释依据,
run-lua-audit.sh 断言 quick_hint 不改变候选次序作为回归守卫。

产品形态:

- `xhup_flow`(默认,现代模式):静态层 + Flow + 学习 + Lua 策略;
- `xhup_flow_static`:纯静态,无 Lua、无学习(调试/基准/隐私敏感)。

Lua 全部功能在静态方案下缺席即视为正常。

## 3. 模块规划(小而专,反巨型单文件)

```text
lua/xhup_flow/
├── init.lua                # 命名空间出口(可选,*语法不依赖它)
├── quick_hint.lua          # 简码提示(filter,只追加 comment)
├── candidate_control.lua   # 本地置顶/降频/隐藏(processor+filter 双入口)
├── context_ranker.lua      # 有界上下文调序(filter,仅前 3~5 候选)
├── sentence_policy.lua     # 简码与组句交互策略(随 Flow 重设计落地)
└── util.lua                # 平台检测、配置读取、安全 IO
```

反模式红线(来自调研):单文件 >30KB、把 lua 源文件当数据库写回、
路径片段猜平台、Lua 重造 OpenCC。

## 4. 各模块语义与不变量

### 4.1 quick_hint(简码提示)— 已落地(#59)

用户键入较长码时,候选注释显示 `⚡<简码>`(如 `时间` 候选注释 `⚡uij`)。

- 只写 candidate comment,**绝不**改变候选次序(与冻结契约兼容;
  run-lua-audit.sh 逐码断言次序不变);
- 数据源:init 时从生成器产出的简码映射文件一次性加载为哈希表
  (O(1) 查询),热路径零 IO;每候选恰好 yield 一次(重复候选问题
  由 filter 顺序保证,不在 Lua 内去重 —— 见 §2 不变量);
- 默认开启可配置(schema switch);Trainer 练习模式的答案泄露规则是
  独立约束,正常输入提示不得影响练习测试;
- 纯逻辑单测 tests/lua/test_quick_hint.lua(lua5.4,不依赖 librime)+
  模块级仿真 tests/lua/sim_quick_hint.lua(真实生成数据 + require 路径)。

**行尾不变量**:`*.lua` 受 `.gitattributes` `text eol=lf` 约束 ——
Trainer 打包用 `include_str!` 按字节嵌入 Lua 源,Windows autocrlf 转出的
CRLF 会破坏该字节不变量(#59 修复链一环)。

### 4.2 candidate_control(本地候选控制)

机制参考雾凇 cold_word_drop(置顶/降频/隐藏),但:

- 状态存储在**独立 userdb 或受控文件**(如 `xhup_flow_user_lua` /
  `lua/xhup_flow_state/`),绝不写回 `lua/` 源文件、绝不改 canonical 数据;
- 提供 restore(清空个人控制状态);
- filter 写法照抄雾凇 pin_cand_filter 骨架:init 编译配置、func 两桶
  重排、others >100 截断透传。

### 4.3 context_ranker(有界上下文调序)

- 只利用本地状态(commit history、context),低置信度 = 不动;
- 仅前 N(默认 3,可配)候选可参与交换;绝不扫描全候选流;
- 静态强固定映射(一级简码/FF/ZR 的 rank-1)永远不参与调序;
- 行为可解释:每次调序可追问证据(调试开关输出理由)。

### 4.4 sentence_policy(简码 × 组句)

语义状态机(随 Flow 重设计定稿,先文档后实现):

- 短码何时作为 token 直接上屏;
- 何时继续参与组句;
- 何时 sentence translator 接管;
- 静态优先级何时压制组句候选。

参考魔然 `moran_express_translator`:Lua 内实例化原生 translator
(`Component.Translator(env.engine, "", "table_translator@...")`)是
已验证的编排路径;`is_sentence_making` 时简码是否参与需显式决策。

## 5. 语法模型(octagram)策略

- 只挂在 **flow 组句层**,静态层永远不挂;
- 万象模型明示「原则上不支持形码」→ 不采购;路径:(a) lotem/rime-octagram-data
  essay 模型(简体效果待验证,许可待确认);(b) 自有语料经
  rime-build-grammar-word-frequency 自训;
- 挂接做成**可选补丁**(`__include` 范式,参考魔然 octagram 段),默认关,
  先 A/B(`xhup_flow_static` 为对照)再决定默认;
- penalty 参数必须对着 XHUP 词库重调,不抄万象数值。

## 6. 性能预算

- filter/translator 函数跑在 coroutine,逐候选惰性产出,可截断;
- 每次 `LuaTranslation` 析构触发 `gc_step(32)` → 热路径禁止构造大临时表;
- 一切大表在 `init(env)` 加载并挂 `env`,func 内零文件 IO、零正则编译;
- 单候选处理 O(1);整流处理 O(k),k 有界(默认 ≤100,超出原样透传);
- CI 增加延迟测量守卫(候选产生 P95,阈值随基准数据定)。

## 7. 测试策略

1. **纯 Lua 单测**:模块的确定性纯函数(打分、截断、状态迁移)用普通
   Lua 解释器跑(不依赖 librime),CI 安装 `lua5.4`;
2. **集成测试**:CI librime job 安装 `librime-plugin-lua`(Ubuntu noble
   有包),真实部署后断言 quick_hint 注释、降级行为、静态次序不变;
3. **回归不变量**(每次必过):
   - 静态候选次序与无 Lua 完全一致(全静态等值审计的 Lua 变体);
   - 无插件环境 schema 加载不报错(降级);
   - candidate_control 状态文件损坏时安全回退;
   - quick_hint 不改变任何候选次序。

## 8. 安全共存(发布阻断)

独立 Rime 包 / Trainer 安装器分发 Lua 模块时:

- 只写 `lua/xhup_flow/**`(OWNED_FILES 新增此目录);
- 不写 `rime.lua`、不改用户 `lua/` 下其它文件;
- 卸载只移除 `lua/xhup_flow/**` 与自有状态文件;
- 用户若在 `lua/xhup_flow/` 放了同名文件,视为用户补丁,安装前备份
  (与 default.custom.yaml 的备份-合并策略同构)。

## 9. 隐私红线

Lua 模块不得:发起网络请求、读取用户词库以外的个人数据、向日志输出
用户输入内容、把任何状态写到 OWNED_FILES 之外的位置。
