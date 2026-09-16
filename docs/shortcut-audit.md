# Production Shortcut Audit(词库质量提升 / Issue #83 §3/§9/§10)

## 目的

Issue #83 §3 的硬产品合同:`advertised shortcut => useful shortcut`,
misleading-hint rate ≈ 0。本管线把「广告出去的简码」(quick-hint 提示视图)
逐条对照 canonical 真实候选菜单,量化每个简码的真实效用,并在 CI 中
以 `--threshold 0.0` 断言「misleading = 0」。

## 组成

- `crates/xhup-analyzer/src/shortcut_audit.rs`:审计引擎
  (逐条 rank/节省/判定 + 聚合指标);
- `crates/xhup-analyzer/src/bin/production-shortcut-audit.rs`:CLI 门禁
  (`--output <TSV> --threshold <rate>`,超标退出码 1);
- `crates/xhup-generator/src/lua_hints.rs`:`lua_hints_view_with_full_lens()`
  提示视图(与 Lua quick_hint 运行时同一聚合实现,§9 效用语义统一);
- CI:`.github/workflows/ci.yml` librime job 新增
  「Production Shortcut Audit」步骤。

## 指标定义(§10)

| 指标 | 定义 |
|---|---|
| useful | 输入简码 → 目标词为菜单首选(rank 1) |
| useful_with_selection | 词在菜单但非首选(需选择/翻页,仍有键节省) |
| misleading | 词不在该码菜单(广告了却命不中)—— 门槛 0 |
| misleading_rate | misleading / total(§24 DoD 量化断言) |
| expected_effort_saving | Σ P(word) × (全码长 − 简码长),P=万象归一化频率 |
| collision_mass | Σ (fanout−1)/fanout(简码位重码质量) |
| prefix_utilization | 不同提示码数 / 提示总条数 |
| top1000_shallow_coverage | 频率先验 top-1000 词获得 ≤3 键简码的比例 |

## 当前生产数据基线(2026-09-16,canonical v2 全层)

- total 68,842;useful 50,795;useful_with_selection 18,047;
- **misleading 0(rate 0.000000,门禁通过)**;
- expected_effort_saving 1.339 键/词(词频加权);
- collision_mass 19,274.9;prefix_utilization 0.754;
- top1000_shallow_coverage 0.769。

## 哨兵红线(§21)

审计对象完全由生成器提示视图派生,任何词(含 `厉害` 等回归哨兵)都不在
生产代码或审计输入中硬编码;哨兵断言只存在于测试中。

## 复现

```bash
cargo run --locked -p xhup-analyzer --bin production-shortcut-audit -- \
  --output /tmp/shortcut-audit.tsv --threshold 0.0
```

输出字节级确定(同输入两次运行 SHA-256 一致)。
