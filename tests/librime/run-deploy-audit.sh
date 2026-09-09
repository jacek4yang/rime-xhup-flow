#!/usr/bin/env bash
# XHUP Flow 真实部署路径审计(rime_deployer --build 全量工作区构建)。
#
# 用法: run-deploy-audit.sh <生成包目录>
#
# 与其它审计脚本的差别:其它脚本用 rime_deployer --compile + 手工编译
# 辅助词典搭建部署目录(聚焦 runtime 行为);本脚本模拟普通用户的真实
# 部署路径 —— 把生成包放入干净目录后执行 `rime_deployer --build`,
# 断言 xhup_flow 方案的 schema/dependencies 机制让三个 runtime 词典
# (包含 PRIMARY/FIXED_FIRST 的主词典 + Flow / Learn 辅助词典)自然产出
# .table.bin,
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

# 共享数据目录准备:复制真实 rime-prelude,但把其 default.yaml 的
# schema_list 换成仅 XHUP 两个方案。
#
# 为什么不用用户目录 default.custom.yaml 叠加:CI 只装 rime-prelude,
# 其 default.yaml 的 schema_list 引用 luna_pinyin 等 schema,但对应文件
# 在独立数据包中并未安装;WorkspaceUpdate 对列表内缺失 schema 记
# failure 并使 --build 整体退出非零。真实用户的共享数据目录装有完整
# 方案集,不受此限。
# 为什么不在用户目录放最小 default.yaml:ConfigFileUpdate 会把
# config_version 落后于共享版本的用户副本当过期文件移入 trash;且
# 最小 default.yaml 会屏蔽 prelude 的 punctuation/recognizer 等段,
# 破坏 import_preset: default 的运行时行为。
shared_dir="$work/shared"
cp -r "$SHARED_DATA_DIR" "$shared_dir"
awk '
  /^schema_list:/ {in_block=1; next}
  in_block && /^[^[:space:]#]/ {in_block=0}
  !in_block {print}
' "$shared_dir/default.yaml" > "$work/default.yaml"
mv "$work/default.yaml" "$shared_dir/default.yaml"
cat >> "$shared_dir/default.yaml" <<'EOF'
schema_list:
  - schema: xhup_flow
  - schema: xhup_flow_static
EOF

echo "== 真实部署路径(rime_deployer --build,无手工词典编译) =="
rime_deployer --build "$deploy_dir" "$shared_dir" >/dev/null

fail=0
for dict in \
  xhup_flow \
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
"$work/runtime_smoke" "$shared_dir" "$deploy_dir"
