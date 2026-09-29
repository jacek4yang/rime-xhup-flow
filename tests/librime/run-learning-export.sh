#!/usr/bin/env bash
# 学习/持久化聚焦回归(分钟级)。
#
# 只跑与学习状态相关的阶段:组句 → 学习会话 → userdb 导出可观察性 →
# 重启持久化 → 学习管理端到端;跳过需要全静态 manifest 的 1.3M 条穷尽遍历
# (由 full-regression.yml 的 run-flow-audit.sh 全量模式覆盖)。
#
# 用法: run-learning-export.sh <生成包目录> [xhup-cli 路径|-]
#
# 回归目标(#152):Flow translator 与 learn translator 共享 xhup_flow_user,
# 两侧音节 id 空间必须一致 —— 否则同一元素会被写成两个码(learn 表把 flow
# 表的音节 id 译成别的词条码),导出的学习条目就会落在不属于它的码下。
# 本脚本断言:干净 userdb 导出不含该词形 → 学习后含该词形且**码唯一** →
# 全新进程仍可在该码下定位该词形;不硬编码任何词形/码(码取自 userdb 导出)。
set -euo pipefail

exec env XHUP_AUDIT_ONLY_LEARNING=1 \
  "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/run-flow-audit.sh" "$@"
