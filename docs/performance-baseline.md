# 性能基线与方法论

原则:**方法论优先于基准表演**。CI 不设机器相关的硬性性能阈值(跨
runner 噪声太大);本文件记录可复现的测量方法与一次本机基线,供后续
版本对比趋势。

## 测量方法

全部测量使用 release 构建,重复 3 次取区间,记录宿主机差异:

```bash
cargo build --release -p xhup-cli --locked
work="$(mktemp -d)"; trap 'rm -rf "$work"' EXIT

# 1. Rime 源生成(11 个 YAML,~6.4 MB)
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

## 回归纪律

- 影响生成器或词典数据结构的 PR:重跑上表 1–2 项,量级变化(>2×)
  需要在 PR 说明中解释;
- 生成产物保持确定性(同输入字节级一致),性能对比因此可比;
- 不为速度引入非确定性缓存或改变冻结映射。
