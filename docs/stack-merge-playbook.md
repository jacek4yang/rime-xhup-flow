# 堆叠 PR 合并手册(人工执行)

本文是 #41–#46 叠加栈的机器可核对合并手册。**自动化代理不执行合并**;
本文供人工合并时按序操作。

## 栈现状(合并前核对)

| PR | 分支 | 基于 | 增量提交 | 增量文件 | CI |
| --- | --- | --- | --- | --- | --- |
| #41 | `refactor/trainer-cross-platform-core` | `feat/hanzi-error-tutor` | 3 | 88 | ✅ 12/12 |
| #42 | `feat/wechat-mini-program` | `refactor/trainer-cross-platform-core` | 5 | 44 | ✅ 12/12 |
| #43 | `feat/rime-daily-use-hardening` | `feat/wechat-mini-program` | 5 | 15 | ✅ 12/12 |
| #44 | `feat/learning-path-engine` | `feat/rime-daily-use-hardening` | 2 | 21 | ✅ 12/12 |
| #45 | `feat/cross-platform-product-hardening` | `feat/learning-path-engine` | 4 | 10 | ✅ 12/12 |
| #46 | `chore/cross-platform-final-audit` | `feat/cross-platform-product-hardening` | 1 | 1 | ✅ |

增量文件数单调不减且各 PR 增量与主题一致(结构审计通过,无祖先回放/无关文件)。

## 合并顺序与操作(自底向上,逐个执行)

```text
merge 最低层 PR → CI 绿 → 验证下一层增量 diff → 下一层重建/改基(如需)→ 合并 → 重复
```

1. **#41**(基于 `feat/hanzi-error-tutor`):先合并其下方的
   `feat/hanzi-error-tutor` 系(若尚未合并到更早的基线),再合并 #41。
2. 合并 **#42**。**注意 squash 合并**:squash 会把 #41 的原始提交压成
   单提交,子孙分支(#42–#46)包含的祖先提交会与新 main 分叉。
3. 每合并一层后,把下一层 PR **retarget 到新 main 并 rebase**:
   ```bash
   git switch feat/wechat-mini-program
   git rebase --onto main refactor/trainer-cross-platform-core
   git push --force-with-lease
   ```
   rebase 后核对增量 diff(`git diff --stat main...HEAD`)仍与上表主题一致。
4. CI 绿后人工 approve → squash merge → 重复至 #46。

## squash 合并的关键注意

- **绝不直接整栈一次合并**:每层 PR 的 review 意义在增量 diff;
  squash 后逐层 rebase 才能保持每层可审。
- **版本守卫**:合并全程 workspace 版本不变;升版在全部合并完成后
  单独执行(见 release-readiness.md)。
- **冲突预期**:#44 与 #43 之间曾有一次分支指针整理
  (`page_no`/`ascii_composer` 修复先入 #44 再整理回 #43);
  rebase 时如遇这两处重复,以 #43 分支版本为准丢弃 #44 侧重复。

## 合并后验收

- [ ] main 上 `cargo test --workspace --all-targets --locked` 通过
- [ ] main 上 `pnpm core:test && pnpm trainer:test && pnpm miniapp:test`
- [ ] `pnpm --filter miniapp build:weapp` 产出 weapp
- [ ] product-packaging 工作流全绿(含 Rime 源包 + 真实部署路径守卫)
- [ ] 真机:重跑 `product_cli status` 应 Healthy;`rime_probe` 26/26
