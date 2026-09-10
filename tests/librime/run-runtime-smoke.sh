#!/usr/bin/env bash
# XHUP Flow librime runtime 冒烟。全量 exact-code 菜单由同一 CI job 的
# run-flow-audit.sh + static-menu.manifest 逐项验证;本脚本覆盖真实 session、
# initial_quality 栅栏、canonical v2 高频别名与 prefix continuation。

set -euo pipefail

PACKAGE_DIR=${1:?"用法: run-runtime-smoke.sh <生成包目录>"}
SHARED_DATA_DIR=${RIME_SHARED_DATA_DIR:-/usr/share/rime-data}
SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

CFLAGS="-O2 -Wall -Wextra -Werror"

# priority fixture 的 secondary dictionary 仍需要临时默认命名空间 wrapper。
compile_dict_via_wrapper() {
  local dir=$1 dict=$2
  cat > "$dir/dict_compile.schema.yaml" <<EOF
# Rime schema
# encoding: utf-8
---
schema:
  schema_id: dict_compile
  name: 词典编译 wrapper
  version: "1"
engine:
  translators:
    - table_translator
translator:
  dictionary: $dict
EOF
  rime_deployer --compile "$dir/dict_compile.schema.yaml" "$dir" \
    "$SHARED_DATA_DIR" >/dev/null
  rm "$dir/dict_compile.schema.yaml"
}

# 生产 Flow/Learn 辅助词典必须分别在隔离目录编译；同目录连续复用临时
# wrapper 会让 librime 的部署状态互相干扰。真实用户路径另由
# run-deploy-audit.sh 的 `rime_deployer --build` 独立守卫。
compile_package_dict_isolated() {
  local dict=$1 dest=$2
  local dir="$work/compile-$dict"
  mkdir -p "$dir"
  cp "$PACKAGE_DIR/$dict.dict.yaml" "$dir/"
  compile_dict_via_wrapper "$dir" "$dict"
  test -f "$dir/build/$dict.table.bin" || {
    echo "词典编译失败: $dict" >&2
    exit 2
  }
  mkdir -p "$dest/build"
  cp "$dir/build/$dict.table.bin" "$dir/build/$dict.prism.bin" \
     "$dir/build/$dict.reverse.bin" "$dest/build/"
}

# ---------- 1. priority preflight ----------
preflight_control="$work/preflight-control"
preflight_production="$work/preflight-production"
for dir in "$preflight_control" "$preflight_production"; do
  mkdir -p "$dir"
  cp "$SCRIPT_DIR/priority_preflight"/*.yaml "$dir/"
done
cat > "$preflight_control/default.custom.yaml" <<'EOF'
patch:
  schema_list/+:
    - schema: preflight_control
EOF
cat > "$preflight_production/default.custom.yaml" <<'EOF'
patch:
  schema_list/+:
    - schema: preflight
EOF
rime_deployer --compile "$preflight_control/preflight_control.schema.yaml" \
  "$preflight_control" "$SHARED_DATA_DIR" >/dev/null
rime_deployer --compile "$preflight_production/preflight.schema.yaml" \
  "$preflight_production" "$SHARED_DATA_DIR" >/dev/null
compile_dict_via_wrapper "$preflight_production" preflight_secondary

cc $CFLAGS -o "$work/runtime_priority_preflight" \
  "$SCRIPT_DIR/runtime_priority_preflight.c" \
  $(pkg-config --cflags --libs rime)
echo "== priority preflight(initial_quality 栅栏机制) =="
"$work/runtime_priority_preflight" "$SHARED_DATA_DIR" "$preflight_control" \
  preflight_control "甲,乙,丙"
"$work/runtime_priority_preflight" "$SHARED_DATA_DIR" "$preflight_production" \
  preflight "甲,乙,丙,目标"

# ---------- 2. canonical v2 production 冒烟 ----------
smoke_dir="$work/smoke"
mkdir -p "$smoke_dir"
cp "$PACKAGE_DIR"/*.yaml "$smoke_dir/"
if [[ -d "$PACKAGE_DIR/lua" ]]; then cp -r "$PACKAGE_DIR/lua" "$smoke_dir/"; fi
cat > "$smoke_dir/default.custom.yaml" <<'EOF'
patch:
  schema_list/+:
    - schema: xhup_flow
  menu/page_size: 500
EOF
rime_deployer --compile "$smoke_dir/xhup_flow.schema.yaml" "$smoke_dir" \
  "$SHARED_DATA_DIR" >/dev/null
for dict in xhup_flow_flow xhup_flow_learn; do
  compile_package_dict_isolated "$dict" "$smoke_dir"
done

cc $CFLAGS -o "$work/runtime_smoke" "$SCRIPT_DIR/runtime_smoke.c" \
  $(pkg-config --cflags --libs rime)
echo "== canonical v2 production 冒烟 =="
"$work/runtime_smoke" "$SHARED_DATA_DIR" "$smoke_dir"
