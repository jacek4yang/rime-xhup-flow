#!/usr/bin/env bash
# XHUP Flow librime 日常输入控制测试驱动。
#
# 用法: tests/librime/run-ime-controls.sh <生成包目录> [shared_data_dir]
#
# 生成包目录必须含 xhup-cli generate rime 的全部 yaml 输出。
# 脚本在临时目录搭建隔离的 Rime user 目录(绝不触碰真实用户 Rime 配置):
#
#   - 部署生产包(xhup_flow),default.custom.yaml 只加 schema_list 与
#     menu/page_size: 5(真实前端每页 5 候选;翻页/数字选择测试需要)。
#   - 编译全部词典(自定义命名空间词典经 wrapper schema 编译)。
#   - 运行 runtime_ime_controls:ASCII 切换、中文标点、数字选择、
#     翻页、Escape/Enter/空格 —— 全部是日常输入法操作。
#
# 依赖: rime_deployer(librime-bin)、pkg-config、librime-dev、C 编译器。

set -euo pipefail

PACKAGE_DIR=${1:?"用法: run-ime-controls.sh <生成包目录> [shared_data_dir]"}
SHARED_DATA_DIR=${2:-${RIME_SHARED_DATA_DIR:-/usr/share/rime-data}}
SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

CFLAGS="-O2 -Wall -Wextra -Werror"

# 编译自定义命名空间(fixed_first / flow / learn)引用的词典:
# rime_deployer --compile 只编译默认 translator 命名空间,用临时 wrapper
# schema 把目标词典放到默认命名空间编译。wrapper 仅存在于测试临时目录。
compile_dict_via_wrapper() {
  local dir=$1 dict=$2
  cat > "$dir/ime_dict_compile.schema.yaml" <<EOF
# Rime schema
# encoding: utf-8
---
schema:
  schema_id: ime_dict_compile
  name: 词典编译 wrapper
  version: "1"
engine:
  translators:
    - table_translator
translator:
  dictionary: $dict
EOF
  rime_deployer --compile "$dir/ime_dict_compile.schema.yaml" "$dir" \
    "$SHARED_DATA_DIR" >/dev/null
  rm "$dir/ime_dict_compile.schema.yaml"
}

controls_dir="$work/ime-controls"
mkdir -p "$controls_dir"
cp "$PACKAGE_DIR"/*.yaml "$controls_dir/"
cat > "$controls_dir/default.custom.yaml" <<'EOF'
patch:
  schema_list/+:
    - schema: xhup_flow
  menu/page_size: 5
EOF

rime_deployer --compile "$controls_dir/xhup_flow.schema.yaml" "$controls_dir" \
  "$SHARED_DATA_DIR" >/dev/null
compile_dict_via_wrapper "$controls_dir" xhup_flow_fixed_first_shortcuts

cc $CFLAGS -o "$work/runtime_ime_controls" "$SCRIPT_DIR/runtime_ime_controls.c" \
  $(pkg-config --cflags --libs rime)
echo "== 日常输入控制(ASCII/标点/数字选择/翻页/Escape/Enter) =="
"$work/runtime_ime_controls" "$SHARED_DATA_DIR" "$controls_dir"
