# Lua 运行时：当前支持范围

本文描述实际发布路径，取代早期路线图中的实验能力声明。

## 组件与职责

| 组件 | 当前作用 |
|---|---|
| `native_tail` | 一个原生翻译/学习提供者，有界词边规划与未完成尾码支持 |
| `quick_hint` / `annotation` | 简码提示与注释，不承担路径搜索 |
| `context_ranker` | 默认关闭；只重排全局前 3～5 个候选，其余原样透传 |
| `user_memory` | 默认关闭；有界、同意后观察的**会话**计数，无文件读写 |
| `full_span` | 前 32 项内保留同文同起点的较长跨度候选，避免短候选遮蔽完整路径 |
| `joint_decoder` | **不可用**；仅保留无缓冲透传兼容文件，方案不注册、不提供开启开关 |
| `init` | Lua 模块文件预检，不证明实际注册、执行或解码健康 |

Rust 离线 `xhup-decoder`、`native_lookup` 原型、历史 joint hook 和研究 TSV
不是已部署的上下文联合解码器。原生边界规划采用“最少词边”的结构启发式，
不是神经语言模型，也不宣称最优语言学分词；原生候选仍可选择。
详见 [原生边界与学习合同](native-tail-investigation.md)。

## 加载与降级

组件以 `lua_translator@*xhup_flow.native_tail` / `lua_filter@*...` 加载，
无需覆盖用户的 `rime.lua`。注释和调序位于 `uniquifier` 之前。

主方案 **需要实际可用的 librime-lua**。插件缺失时，静态 translator 可能仍有
候选，但这不是完整 Flow；应使用独立的 `xhup_flow_static`。后者零 Lua、
零学习，是兼容与隐私回退锚点。Weasel/Squirrel 常见发行包内置 Lua，不等于
每个用户进程已经注册并启用了模块；Windows/macOS 按 README 由用户验收。

有界学习还要求可用的原生 memorization callback、disconnect 和用户词典 tick
接口；仅有 `Component.TableTranslator` 不够。旧版/部分回移的插件可能只能
只读组句，显示 `bounded_api_unavailable`；存储与配额拒绝也会显示原因。
CI 分别检查实际旧插件降级和固定源码新版的完整持久学习，不能互相替代。

CLI doctor、Trainer 和 Lua 文件预检均不得把安装完整视为运行 PASS。
实际注册、filter 执行、存储写入需要分别观察，未观察则为 Unknown。
见 [runtime-capabilities.md](runtime-capabilities.md)。

## 隐私与持久化

- 无网络、账号、遥测或持久按键日志。
- 原生 `xhup_flow_user` 是唯一持久运行时学习存储，具备保守更新配额。
- 可选会话调序只在显式启用后观察提交；会话记忆最多 512 项/项 256 字节。
- 关闭清除，重启为空，多引擎隔离；历史 TSV 不读取、不改写、不自动迁移。
- 原生学习与会话调序是不同开关，隐私敏感场景请选择 Static。

见 [context-memory-persistence.md](context-memory-persistence.md)。

## 验证边界

Lua stub 单测证明纯逻辑合同，不替代真实 librime。
`run-lua-audit.sh` 检查注释开关与菜单不变；
`run-context-ranker-audit.sh` 检查会话同意、隔离、关闭清除和排序；
`run-flow-audit.sh` 检查完整跨度、逐键编辑、实际上屏、原生学习与管理；
静态全量审计单独覆盖完整菜单顺序/重复。仅当前构建的完成结果才是证据。

历史研究模块留在 `tests/research/`，其旧测试不代表发布功能。平台 GUI、
WebView、安装升级与最终 RC 来源/资产验收仍须独立完成；稳定发布需人工授权。
