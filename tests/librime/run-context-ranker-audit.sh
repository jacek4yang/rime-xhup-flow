#!/usr/bin/env bash
# XHUP Flow context_ranker 有界上下文调序 runtime 审计驱动(真实 librime-lua)。
#
# 用法: run-context-ranker-audit.sh <生成包目录>
#
# 与 run-lua-audit.sh 同构:真实 deploy 目录 + 真实 librime-lua 插件。
# 断言 #128 转正条件:默认关闭恒等、无证据恒等、重复词有界提升、
# 静态强固定 rank-1 不降位、OOV 可达。

set -euo pipefail

PACKAGE_DIR=${1:?"用法: run-context-ranker-audit.sh <生成包目录>"}
SHARED_DATA_DIR=${RIME_SHARED_DATA_DIR:-/usr/share/rime-data}
SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

CFLAGS="-O2 -Wall -Wextra -Werror"

compile_dict_isolated() {
  local dict=$1 dest=$2
  local d="$work/compile-$dict"
  mkdir -p "$d"
  cp "$PACKAGE_DIR/$dict.dict.yaml" "$d/"
  if [[ "$dict" == xhup_flow_learn ]]; then
    cp "$PACKAGE_DIR/xhup_flow_flow.dict.yaml" "$d/"
  fi
  cat > "$d/dc.schema.yaml" <<EOF
# Rime schema
# encoding: utf-8
---
schema:
  schema_id: dc
  name: 词典编译 wrapper
  version: "1"
engine:
  translators:
    - table_translator
translator:
  dictionary: $dict
EOF
  rime_deployer --compile "$d/dc.schema.yaml" "$d" "$SHARED_DATA_DIR" >/dev/null
  test -f "$d/build/$dict.table.bin" || {
    echo "词典编译失败: $dict" >&2
    exit 2
  }
  mkdir -p "$dest/build"
  cp "$d/build/$dict.table.bin" "$d/build/$dict.prism.bin" \
     "$d/build/$dict.reverse.bin" "$dest/build/"
}

deploy_dir=$work/deploy
mkdir -p "$deploy_dir"
cp "$PACKAGE_DIR"/*.yaml "$deploy_dir/"
cp -r "$PACKAGE_DIR/lua" "$deploy_dir/"
cat > "$deploy_dir/default.custom.yaml" <<'EOF'
patch:
  schema_list/+:
    - schema: xhup_flow
  menu/page_size: 500
EOF
rime_deployer --compile "$deploy_dir/xhup_flow.schema.yaml" "$deploy_dir" \
  "$SHARED_DATA_DIR" >/dev/null
for dict in xhup_flow_fixed_first_shortcuts xhup_flow_flow xhup_flow_learn; do
  compile_dict_isolated "$dict" "$deploy_dir"
done

cc $CFLAGS -o "$work/audit" "$SCRIPT_DIR/runtime_context_ranker_audit.c" \
  $(pkg-config --cflags --libs rime)

echo "== Lua context_ranker 合同检查 (真实 deploy 目录) =="
lua5.4 -e '
package.path = "'"$deploy_dir"'/lua/?.lua;'"$deploy_dir"'/lua/?/init.lua;" .. package.path
local xhup = require("xhup_flow")
local report = xhup.check_contract()
assert(report.ok, "Lua 合同检查未通过: " .. table.concat(report.errors, "; "))
assert(report.components.context_ranker ~= nil, "合同诊断缺 context_ranker 组件")
print("PASS  Lua check_contract() 含 context_ranker: " .. report.version)
'

echo "== Lua context_ranker runtime 审计(真实 librime-lua) =="
# 审计在 deploy 目录内执行:user_memory 组件的快照默认写到进程工作目录
# (相对路径),cd 进 deploy 即把 TSV 落在与 librime 用户数据一致的位置,
# 重启场景可真实读到。
(cd "$deploy_dir" && "$work/audit" "$SHARED_DATA_DIR" .)
