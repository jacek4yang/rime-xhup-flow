#!/usr/bin/env bash
# XHUP Flow 真实部署路径审计(rime_deployer --build 全量工作区构建)。
#
# 用法: run-deploy-audit.sh <生成包目录>
#
# 与其它审计脚本的差别:其它脚本用 rime_deployer --compile + 手工编译
# 辅助词典搭建部署目录(聚焦 runtime 行为);本脚本模拟普通用户的真实
# 部署路径 —— 把生成包放入干净目录后执行 `rime_deployer --build`,
# 断言 xhup_flow 方案的 schema/dependencies 机制让全部四个词典
# (主词典 + FIXED_FIRST / Flow / Learn 辅助词典)自然产出 .table.bin,
# 再用 runtime_smoke 对该部署跑真实输入冒烟。
#
# 这是「辅助词典必须经真实 Rime deployment graph 编译」的回归守卫:
# 绝不允许只靠测试手工编译掩盖部署缺陷(真机事故:缺失时简码/组句/
# 学习全部静默失效)。
#
# 依赖: rime_deployer、pkg-config、librime 开发头文件、C 编译器。
# 共享数据目录可用 RIME_SHARED_DATA_DIR 覆盖。

set -euo pipefail

PACKAGE_DIR=${1:?"用法: run-deploy-audit.sh <生成包目录>"}
SHARED_DATA_DIR=${RIME_SHARED_DATA_DIR:-/usr/share/rime-data}
SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

CFLAGS="-O2 -Wall -Wextra -Werror"

deploy_dir="$work/deploy"
mkdir -p "$deploy_dir"
cp "$PACKAGE_DIR"/*.yaml "$deploy_dir/"
# 方案引用 lua_filter 时必须随包携带 lua/ 模块(见 run-flow-audit.sh)。
if [[ -d "$PACKAGE_DIR/lua" ]]; then cp -r "$PACKAGE_DIR/lua" "$deploy_dir/"; fi
# 注意:部署目录必须提供一份只含 XHUP 方案的 default.yaml,而不能靠
# default.custom.yaml 叠加共享目录的 default.yaml —— CI 环境只装
# rime-prelude(共享 default.yaml 引用 luna_pinyin 等 schema 但对应文件在
# 独立数据包中、并未安装),--build 会因 missing input schema 整体退出非零。
# 真实用户的共享数据目录装有完整方案集,不受此限;这里用最小 default.yaml
# 让 --build 聚焦验证 XHUP 方案自身的 dependency graph。
cat > "$deploy_dir/default.yaml" <<'EOF'
schema_list:
  - schema: xhup_flow
  - schema: xhup_flow_static
EOF

echo "== 真实部署路径(rime_deployer --build,无手工词典编译) =="
rime_deployer --build "$deploy_dir" "$SHARED_DATA_DIR" >/dev/null

fail=0
for dict in \
  xhup_flow \
  xhup_flow_fixed_first_shortcuts \
  xhup_flow_flow \
  xhup_flow_learn; do
  if [[ -f "$deploy_dir/build/$dict.table.bin" ]]; then
    echo "PASS  --build 产出 $dict.table.bin"
  else
    echo "FAIL  --build 未产出 $dict.table.bin(schema/dependencies 未覆盖)"
    fail=1
  fi
done
[[ $fail -eq 0 ]] || exit 1

cc $CFLAGS -o "$work/runtime_smoke" "$SCRIPT_DIR/runtime_smoke.c" \
  $(pkg-config --cflags --libs rime)
echo "== 对 --build 部署的 runtime 冒烟 =="
"$work/runtime_smoke" "$SHARED_DATA_DIR" "$deploy_dir"
