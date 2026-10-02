# 原生修复验证记录（不是稳定版验收）

本记录只为下列固定提交及生成包存档。原 RC.2、后续 RC.3 或其他安装物
不能继承其通过状态；Windows/macOS 的用户验收仍按 README 执行。
本次任务没有创建 tag、Release 或发布稳定版。

## 完整原生清单审计

- 工作流：[37035281631 / attempt 1](https://github.com/jacek4yang/rime-xhup-flow/actions/runs/37035281631)。
- 源码：`e352c55ffe4d9cb86028c1d331ac57afdc8c0707`（集成修复的测试提交）。
- 全部 16 个分片和汇总成功；不是采样或 focused replay。
- 完整输入：141,138 个静态码、1,301,434 条 extended 关系、1,000 条 open 组合。
- 每个分片都完成干净/学习后静态对照、extended/open、组句、真实学习与
  重启、CLI 导出/重置/导入。部署、控制、Lua、上下文及 doctor 在准备阶段执行。
- 下载原始输入和全部日志/回执后，在本地再次执行 `audit_shards.py collect`，
  独立重算分区和 SHA256，并与 CI `coverage.json` 逐字节比较，相同。

[存档目录](../data/benchmarks/native-audit-e352-v1/)保留原始计划、16 份回执
及覆盖摘要。计划 SHA256：
`c83b5fbfb5467d762fb52825d7fb5c6c9e340a4027019cd5d073948f7dd327c5`。
计划绑定源码、run/attempt、生成文件、CLI、Lua 插件及所有分区；回执绑定日志。
这是 CI 的执行证据，不是恶意 runner 防伪签名。完整输入/日志目前保留在已下载
的验证缓存与该 run 的 Actions artifacts（后者保留期 14 天）；本目录不包含
这些大文件，单凭摘要不能重演全部审计。正式发布前须对最终源/产物重新执行并
独立归档完整证据，不能把临时 artifact URL 当永久发布证明。

复核需从上述固定 run 下载 `native-audit-input`、全部 `native-audit-result-*`
和 `native-audit-complete-coverage`，合并日志/回执到不含旧输出的目录，然后：

```sh
python3 tests/librime/audit_shards.py collect --root INPUT --out RESULTS \
  --revision e352c55ffe4d9cb86028c1d331ac57afdc8c0707 --run 37035281631.1
cmp RESULTS/coverage.json CI-COVERAGE/coverage.json
```

## 已见诊断集回归：不覆盖原始负面结果

[最初的外部测试](independent-native-evaluation.md)曾发现结构规划器损害首选。
该集合用于诊断以后就是**已见回归集**，不能再称未见或严格 held-out。
保留 `native-source-external-v1.json` 原始负面结果；本轮单独存档：

- [完整配对报告](../data/benchmarks/native-source-seen-regression-v1.json)
- [实际生成包与运行环境](../data/benchmarks/native-source-seen-regression-v1-environment.json)

两模式均完成全部 1,821 串，硬行为检查 13,308 / 13,203 项均零失败。
修正策略和原生基础流首选均 **41.13%**、前3均 **44.04%**；逐例核查
原生前五内的 **814 例排名全部不变**，`--require-native-prefix` 成功。
前256可达且实际精确提交为 **46.07% / 44.92%**，新增待续键后目标可达为
**37.45% / 8.84%**。策略 SHA256 为
`4dd7e514006152d2ddc497413136388ab3a3765aec6a8be8ce504cd760a17f6d`。

这证明所观察的首选退化已修复，同时保留部分可达收益，不证明长句或专名
已达到理想准确率。源码版本中的生成来源是调用方声明，10 个运行时模块/方案
另经逐字节校验；这不等于重新独立构建全部数据。耗时有并发审计负载，不作
设备性能承诺。新的冻结确认分割另行报告，不与此已见集混为一谈。
