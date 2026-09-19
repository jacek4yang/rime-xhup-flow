# 性能基线与方法论

原则:**方法论优先于基准表演**。CI 不设机器相关的硬性性能阈值(跨
runner 噪声太大);本文件记录可复现的测量方法与一次本机基线,供后续
版本对比趋势。

## 测量方法

全部测量使用 release 构建,重复 3 次取区间,记录宿主机差异:

```bash
cargo build --release -p xhup-cli --locked
work="$(mktemp -d)"; trap 'rm -rf "$work"' EXIT

# 1. Rime 源生成(12 个 YAML + 2 个 Lua)
time target/release/xhup-cli generate rime --output "$work/rime"

# 2. Trainer 规范数据集(~7.3 MB JSON)
time target/release/xhup-cli generate trainer --output "$work/trainer"

# 3. librime 编译(部署开销,决定安装后首次部署时长)
#    见 tests/librime/run-runtime-smoke.sh 的 wrapper 编译流程
rime_deployer --compile "$work/rime/xhup_flow.schema.yaml" "$work/user" /usr/share/rime-data

# 4. runtime 行为基线(正确性优先,附带耗时)
tests/librime/run-runtime-smoke.sh ...
tests/librime/run-flow-audit.sh ...
```

## 查询性能(概念说明)

- **静态查询**:纯查表(生成期已固化排序),无运行时拼写运算;
- **Flow 组句查询**:librime table_translator + 组句 translator,
  开销集中在 librime C++ 侧,本项目不介入;
- **冷加载**:首次部署把 YAML 编译为 `.table.bin`;控制中心安装后
  的重新部署即此开销。

## 本机基线(2026-09,记录用,不作门槛)

宿主:Linux x86_64,开发机(具体硬件不影响方法论,只看趋势)。

| 操作 | 耗时(3 次区间) |
| --- | --- |
| `generate rime`(release) | 1.06 – 1.58 s |
| `generate trainer`(release) | 1.10 – 1.46 s |

librime 编译与 runtime 审计耗时以 CI 日志为准(Ubuntu runner 上
完整冒烟 + Flow 审计约数分钟),不在此重复记录。

## P0 开放输入变更对比

同一开发机、同一 librime 1.16.1；before 为 100k 词包，after 为
100k hot + 1,301,434 extended + 9,254 单字音码原语：

| 指标 | before | after |
| --- | ---: | ---: |
| Rime 源文件总大小 | 9,028,087 B | 36,845,122 B |
| GitHub CI Rime ZIP（含 `INSTALL.md`） | — | 16,282,474 B |
| release `generate rime` | 1.06–1.58 s | 5.39 s |
| `rime_deployer --build` + 冒烟 | 3.81 s | 17.07–27.08 s |
| 部署阶段峰值 RSS | 125,384 KiB | 1,105,580 KiB |
| 已部署 runtime 78 项冒烟 | — | 0.48 s / 41,216 KiB 峰值 RSS |

增长发生在首次/升级部署的词典编译阶段，已部署运行时没有同量级常驻内存
回退。该成本换取 pinned 上游全部合法词汇不再受 Top-N 可达性截断；后续若
优化物理存储，必须保持完整 runtime reachability 门禁，不得重新删词。

## 回归纪律

- 影响生成器或词典数据结构的 PR:重跑上表 1–2 项,量级变化(>2×)
  需要在 PR 说明中解释;
- 生成产物保持确定性(同输入字节级一致),性能对比因此可比;
- 不为速度引入非确定性缓存或改变冻结映射。

## 上下文解码延迟(2026-09-18,首次基线)

§22 要求测 `decoder p50/p95/p99`,此前仓库中**没有任何基线**。以下为首次测量。

**测量口径**:release 构建;对 `data/corpus/replay_fixture.txt`(2000 句)中
每个**同码歧义 token** 计时;计时范围 = 「参考路径枚举 + 排序」与
「`decode_beam_adaptive` 有界解码」两次打分之和(即生产解码路径的开销)。
菜单构造与语料分词不计入(不随按键变化)。

```bash
cargo run --release --locked -p xhup-analyzer --bin context-replay-bench --   --sentences data/corpus/replay_fixture.txt   --bigram data/corpus/kdconv_bigram.tsv --latency
```

| 指标 | 值(开发机,Windows x86_64) |
| --- | ---: |
| 样本数 | 2,701 |
| p50 | **7 µs** |
| p95 | **42 µs** |
| p99 | **95 µs** |
| max | 350 µs |

**不作为跨机器门槛**(§22 明确禁止设机器相关硬阈值);上表只记录量级与趋势。
p99 约 95 µs,比人类可感知阈值(约 10 ms)低两个数量级,余量充足。

注意:该延迟只覆盖 **Rust 侧离线评测路径**。上下文能力**尚未接入产品运行时**
(见 Issue #83 的架构说明),因此这不是端到端输入延迟。运行时接入后必须重测
并同时测 Lua memory/GC(§22 要求,当前无基线)。

## Lua 运行时内存基线(2026-09-19,首次基线)

§22 要求测量 **Lua memory / Lua GC** 与模型体积;此前仓库中**完全没有基线**。

**测量脚本**:`tests/lua/measure_memory.lua`(CI 在 librime job 中运行,
与其它 Lua 测试同一环境):

```bash
cargo run --release --locked -p xhup-cli -- generate rime --output /tmp/pkg
lua5.4 tests/lua/measure_memory.lua /tmp/pkg
```

**测量内容**:

1. 加载 `xhup_flow.data.quick_hints` 数据模块前后 `collectgarbage("count")` 之差
   —— 即**模型常驻 Lua 堆**;
2. 策略模块(`quick_hint` / `annotation`)的代码开销;
3. 100,000 次 `decorate` 调用后的**强制 GC 前后差值**(泄漏信号);
4. 单次调用耗时。

| 指标 | 值(lua5.4,开发机 Windows x86_64) |
| --- | ---: |
| 提示条目数 | 68,842 |
| **模型常驻堆(quick_hints)** | **7,845 kB(约 7.7 MB)** |
| 策略模块代码堆 | 6.8 kB |
| 单次 decorate | 约 0.14 µs |
| 100k 次调用后堆增长 | **−0.1 kB(无泄漏)** |

### 关键观察

- 源文件 `quick_hints.lua` 为 **1.7 MB**,加载后 Lua 堆为 **7.7 MB** ——
  约 **4.6 倍膨胀**(Lua 表的字符串/哈希开销)。这是**移动端最相关的数字**,
  此前从未被测过;
- 读数**完全可复现**(两次运行均为 7845.4 kB);
- 无泄漏信号:10 万次调用后强制 GC,堆增长为 −0.1 kB。

### 口径说明(避免误读)

- 这是 **lua5.4 独立解释器**的读数,**不是 librime-lua 进程内**的实测。
  插件环境下 Lua 状态与 librime 共享进程,常驻量会有差异(且 librime 自身
  词典占用远大于此);
- 因此该数字用于**趋势对比与回归检测**,不能直接当作端到端内存预算;
- 与 §22 一致:**只报告,不设跨机器硬门槛**。脚本内的断言只针对
  **泄漏**(100k 次后增长 < 512 kB)这一机器无关的性质。

### 对移动端的含义(供决策参考)

即使按最保守的独立解释器读数,提示表本身约 7.7 MB。若未来把更多证据
(如转移表)搬进 Lua,必须在此基线上增量评估 —— 这也是 Issue #83 中
「路线 A 需要约 15 MB 常驻」估算的同一量级来源。
