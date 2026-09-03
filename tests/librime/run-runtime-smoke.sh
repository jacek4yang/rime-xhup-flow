#!/usr/bin/env bash
# XHUP Flow librime runtime 测试驱动脚本。
#
# 用法: tests/librime/run-runtime-smoke.sh <生成包目录> <FF 审计 manifest> \
#        [二码审计 manifest]
#
# 生成包目录必须含 8 个 xhup_flow yaml(xhup-cli generate rime 的输出);
# FF manifest 由 xhup-analyzer 的 --dump-fixed-first-audit-manifest 生成
# (每条 FIXED_FIRST 简码:码、词、baseline fanout、期望名次、碰撞类型、
# baseline 菜单);二码 manifest 由 static-shortcut-audit 的
# --dump-two-key-audit-manifest 生成(全部占用 2 键码既有菜单 + 全部
# 选定二码映射),提供时执行第 4 阶段。
#
# 脚本在临时目录搭建隔离的 Rime user 目录(绝不触碰真实用户 Rime 配置),
# 依次运行:
#
#   1. priority preflight:最小双 translator fixture,验证 initial_quality
#      栅栏让 secondary 候选严格排在全部 primary 候选之后;
#   2. production 冒烟:固定层回归 + ZR 简码 + FIXED_FIRST 精确序 +
#      二码简码哨兵 + prefix continuation 哨兵;
#   3. FIXED_FIRST 全量 A/B 审计:CONTROL(派生 control schema,仅 primary
#      translator)与 PRODUCTION(真实方案)逐码对照 —— PRODUCTION 菜单
#      必须是 CONTROL 菜单末尾严格追加一个目标词;
#   4. 二码零冲突审计(提供二码 manifest 时):全部占用 2 键码菜单
#      逐项不变(P0)+ 全部选定映射 rank1 且唯一。
#
# CONTROL schema 由 production schema 在临时目录派生(去掉 fixed_first
# translator 与命名空间),不作为包产物。menu/page_size: 500 只存在于本
# 脚本生成的测试 default.custom.yaml(枚举完整候选用),不写入 production
# schema,也不代表真实前端 UI。
#
# 依赖: rime_deployer(librime-bin)、pkg-config、librime 开发头文件
# (librime-dev)、C 编译器。共享数据目录可用 RIME_SHARED_DATA_DIR 覆盖
# (默认 /usr/share/rime-data,需含 rime-prelude)。

set -euo pipefail

PACKAGE_DIR=${1:?"用法: run-runtime-smoke.sh <生成包目录> <FF 审计 manifest> [二码 manifest]"}
MANIFEST=${2:?"用法: run-runtime-smoke.sh <生成包目录> <FF 审计 manifest> [二码 manifest]"}
TWO_KEY_MANIFEST=${3:-}
SHARED_DATA_DIR=${RIME_SHARED_DATA_DIR:-/usr/share/rime-data}
SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

CFLAGS="-O2 -Wall -Wextra -Werror"

# 编译定制命名空间(table_translator@fixed_first)引用的词典。
# rime_deployer --compile 只接受 schema 文件,且只编译默认 translator
# 命名空间的词典;这里用临时 wrapper schema 把目标词典放到默认命名空间
# 编译,产出的 <词典名>.table.bin 落 build/,供真实方案的 fixed_first
# translator 加载。wrapper 仅存在于测试临时目录,不是包产物。
compile_dict_via_wrapper() {
  local dir=$1 dict=$2
  cat > "$dir/ff_dict_compile.schema.yaml" <<EOF
# Rime schema
# encoding: utf-8
---
schema:
  schema_id: ff_dict_compile
  name: 词典编译 wrapper
  version: "1"
engine:
  translators:
    - table_translator
translator:
  dictionary: $dict
EOF
  rime_deployer --compile "$dir/ff_dict_compile.schema.yaml" "$dir" \
    "$SHARED_DATA_DIR" >/dev/null
  rm "$dir/ff_dict_compile.schema.yaml"
}

# ---------- 1. priority preflight(fixture deployment) ----------
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

# ---------- 2. production 冒烟 ----------
smoke_dir="$work/smoke"
mkdir -p "$smoke_dir"
cp "$PACKAGE_DIR"/*.yaml "$smoke_dir/"
cat > "$smoke_dir/default.custom.yaml" <<'EOF'
patch:
  schema_list/+:
    - schema: xhup_flow
EOF
rime_deployer --compile "$smoke_dir/xhup_flow.schema.yaml" "$smoke_dir" \
  "$SHARED_DATA_DIR" >/dev/null
compile_dict_via_wrapper "$smoke_dir" xhup_flow_fixed_first_shortcuts

cc $CFLAGS -o "$work/runtime_smoke" "$SCRIPT_DIR/runtime_smoke.c" \
  $(pkg-config --cflags --libs rime)
echo "== production 冒烟(固定层回归 + 简码哨兵) =="
"$work/runtime_smoke" "$SHARED_DATA_DIR" "$smoke_dir"

# ---------- 3. FIXED_FIRST 全量 A/B 审计 ----------
control_dir="$work/ff-control"
production_dir="$work/ff-production"
for dir in "$control_dir" "$production_dir"; do
  mkdir -p "$dir"
  cp "$PACKAGE_DIR"/*.yaml "$dir/"
done
# menu/page_size 500:测试枚举完整候选专用,不属于 production schema,
# 也不代表真实前端 UI。
cat > "$control_dir/default.custom.yaml" <<'EOF'
patch:
  schema_list/+:
    - schema: xhup_flow_control
  menu/page_size: 500
EOF
cat > "$production_dir/default.custom.yaml" <<'EOF'
patch:
  schema_list/+:
    - schema: xhup_flow
  menu/page_size: 500
EOF

# 派生 CONTROL schema(仅存在于临时目录):schema_id 改名、去掉
# fixed_first translator 与其配置命名空间。
awk '
  /^  schema_id:/ { print "  schema_id: xhup_flow_control"; next }
  /^    - table_translator@fixed_first$/ { next }
  /^fixed_first:/ { skip = 1; next }
  skip && /^[^ ]/ { skip = 0 }
  skip { next }
  { print }
' "$PACKAGE_DIR/xhup_flow.schema.yaml" > "$control_dir/xhup_flow_control.schema.yaml"
rm "$control_dir/xhup_flow.schema.yaml"

rime_deployer --compile "$control_dir/xhup_flow_control.schema.yaml" \
  "$control_dir" "$SHARED_DATA_DIR" >/dev/null
rime_deployer --compile "$production_dir/xhup_flow.schema.yaml" \
  "$production_dir" "$SHARED_DATA_DIR" >/dev/null
compile_dict_via_wrapper "$production_dir" xhup_flow_fixed_first_shortcuts

cc $CFLAGS -o "$work/runtime_fixed_first_audit" \
  "$SCRIPT_DIR/runtime_fixed_first_audit.c" \
  $(pkg-config --cflags --libs rime)
echo "== FIXED_FIRST 全量 A/B 审计(CONTROL vs PRODUCTION) =="
# 两趟独立进程(glog 不允许同进程二次 initialize),CONTROL 菜单经
# capture 文件传给 PRODUCTION 趟。
"$work/runtime_fixed_first_audit" control "$SHARED_DATA_DIR" "$control_dir" \
  "$MANIFEST" "$work/ff-control.capture"
"$work/runtime_fixed_first_audit" production "$SHARED_DATA_DIR" \
  "$production_dir" "$MANIFEST" "$work/ff-control.capture"

# ---------- 4. 二码零冲突审计(可选,提供二码 manifest 时) ----------
if [ -n "$TWO_KEY_MANIFEST" ]; then
  cc $CFLAGS -o "$work/runtime_two_key_audit" \
    "$SCRIPT_DIR/runtime_two_key_audit.c" \
    $(pkg-config --cflags --libs rime)
  echo "== 二码零冲突全量审计(占用码菜单不变 + 选定映射 rank1) =="
  "$work/runtime_two_key_audit" "$SHARED_DATA_DIR" "$production_dir" \
    "$TWO_KEY_MANIFEST"
fi
