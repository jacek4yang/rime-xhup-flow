#!/usr/bin/env bash
# XHUP Flow Lua quick_hint runtime 审计驱动(真实 librime-lua 环境)。
#
# 用法: run-lua-audit.sh <生成包目录>
#
# 生成包目录必须含 xhup-cli generate rime 的全部产物(含 lua/ 子目录)。
# 运行环境必须已安装 librime-plugin-lua(否则 lua_filter 组件被跳过,
# 提示审计必然失败 —— 这同时反向验证插件确实在场)。
#
# 无插件降级路径由 CI 主冒烟覆盖:runtime smoke 在不安装插件的 job 变体
# 中部署同一方案,输入行为必须完整(见 ci.yml librime 作业步骤顺序)。
#
# 依赖: rime_deployer、pkg-config、librime 开发头文件、C 编译器、
# librime-lua 插件。共享数据目录可用 RIME_SHARED_DATA_DIR 覆盖。

set -euo pipefail

PACKAGE_DIR=${1:?"用法: run-lua-audit.sh <生成包目录>"}
SHARED_DATA_DIR=${RIME_SHARED_DATA_DIR:-/usr/share/rime-data}
SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

CFLAGS="-O2 -Wall -Wextra -Werror"

# 独立目录编译非默认命名空间词典(与 run-flow-audit.sh 同构)。
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

cc $CFLAGS -o "$work/audit" "$SCRIPT_DIR/runtime_lua_audit.c" \
  $(pkg-config --cflags --libs rime)

echo "== Lua quick_hint runtime 审计(真实 librime-lua) =="
"$work/audit" "$SHARED_DATA_DIR" "$deploy_dir"
