#!/usr/bin/env bash
# XHUP Flow 引擎 runtime 审计驱动:全静态等值 / 组句 / 学习 / 持久化 /
# 静态保护 / 学习管理端到端。
#
# 用法: run-flow-audit.sh <生成包目录> <全静态菜单 manifest> [xhup-cli 路径]
#
# 生成包目录必须含 xhup-cli generate rime 的全部产物(11 个 yaml,含
# xhup_flow_static.schema.yaml 与 Flow 组句/学习词典);manifest 由
# xhup-analyzer 的 --dump-static-menu-manifest 导出(全部 distinct 静态
# exact code 及其完整有序菜单);xhup-cli 传入时执行学习管理
# (export/reset/import)端到端验证。
#
# 脚本在临时目录搭建隔离部署(绝不触碰真实用户 Rime 配置),依次运行:
#
#   1. 全静态等值审计(干净 userdb):STATIC schema 逐码捕获完整菜单,
#      FLOW schema 逐码断言 == STATIC == manifest(证明 Flow/学习
#      translator 在无学习数据时对全部静态 exact 菜单零影响);
#   2. 组句审计:fixtures 由组句词典机械拼接(2/4/8/10 词,最长 20 字),
#      断言句子候选出现且无 auto commit;
#   3. 学习会话:提交 Flow 组句句子,训练 xhup_flow_user;
#   4. 重启持久化:全新进程断言学习状态仍在(动态候选可观察);
#   5. 学习后静态审计:全部 140k 静态 exact code 逐码断言既有候选
#      原次序、原 top1、无可见重复(动态候选只允许追加在静态组后);
#   6. 学习管理端到端(提供 xhup-cli 时):export → reset → 学习行为
#      消失 → import 到全新部署 → 学习行为恢复。
#
# 部署说明:rime_deployer --compile 只编译默认 translator 命名空间的
# 词典;FIXED_FIRST/组句/学习词典按词典在独立目录编译后拷入部署 build/
# (同目录连续 wrapper 编译会相互干扰)。menu/page_size: 500 只存在于
# 测试 default.custom.yaml,不写入 production schema。
#
# 依赖: rime_deployer、rime_dict_manager(librime-bin)、pkg-config、
# librime 开发头文件(librime-dev)、C 编译器。共享数据目录可用
# RIME_SHARED_DATA_DIR 覆盖(默认 /usr/share/rime-data,需含 rime-prelude)。

set -euo pipefail

PACKAGE_DIR=${1:?"用法: run-flow-audit.sh <生成包目录> <静态菜单 manifest> [xhup-cli 路径]"}
MANIFEST=${2:?"用法: run-flow-audit.sh <生成包目录> <静态菜单 manifest> [xhup-cli 路径]"}
XHUP_CLI=${3:-}
SHARED_DATA_DIR=${RIME_SHARED_DATA_DIR:-/usr/share/rime-data}
SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

CFLAGS="-O2 -Wall -Wextra -Werror"

# 独立目录编译非默认命名空间词典,产物拷入部署 build/(确定性,无串扰)。
compile_dict_isolated() {
  local dict=$1 dest=$2
  local d="$work/compile-$dict"
  mkdir -p "$d"
  cp "$PACKAGE_DIR/$dict.dict.yaml" "$d/"
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

# 搭建部署:包 yaml + schema_list/menu patch + 主词典编译。
prepare_deploy() {
  local dir=$1 schema_id=$2
  mkdir -p "$dir"
  cp "$PACKAGE_DIR"/*.yaml "$dir/"
  cat > "$dir/default.custom.yaml" <<EOF
patch:
  schema_list/+:
    - schema: $schema_id
  menu/page_size: 500
EOF
  rime_deployer --compile "$dir/$schema_id.schema.yaml" "$dir" \
    "$SHARED_DATA_DIR" >/dev/null
}

# ---------- 1. 全静态等值审计(干净 userdb,两趟独立进程) ----------
static_dir=$work/static
prepare_deploy "$static_dir" xhup_flow_static
compile_dict_isolated xhup_flow_fixed_first_shortcuts "$static_dir"

flow_dir=$work/flow
prepare_deploy "$flow_dir" xhup_flow
for dict in xhup_flow_fixed_first_shortcuts xhup_flow_flow xhup_flow_learn; do
  compile_dict_isolated "$dict" "$flow_dir"
done

cc $CFLAGS -o "$work/audit" "$SCRIPT_DIR/runtime_flow_audit.c" \
  $(pkg-config --cflags --libs rime)

echo "== 全静态等值审计(干净 userdb;STATIC 捕获 → FLOW 对照) =="
"$work/audit" baseline-capture "$SHARED_DATA_DIR" "$static_dir" "$MANIFEST" \
  "$work/static.capture"
"$work/audit" baseline-compare "$SHARED_DATA_DIR" "$flow_dir" "$MANIFEST" \
  "$work/static.capture"

# ---------- 2. 组句审计(fixtures 机械拼接自组句词典) ----------
flow_dict=$PACKAGE_DIR/xhup_flow_flow.dict.yaml
sent=$work/sentences.txt
: > "$sent"
gen_sentence() {
  local codes="" text="" w c
  for w in "$@"; do
    c=$(awk -F'\t' -v w="$w" '$1 == w { print $2; exit }' "$flow_dict")
    if [ -z "$c" ]; then
      echo "组句 fixture 词不在组句词典: $w" >&2
      exit 2
    fi
    codes+="$c"
    text+="$w"
  done
  printf '%s\t%s\n' "$codes" "$text" >> "$sent"
}
gen_sentence 我们 时间
gen_sentence 我们 时间 发展 工作
gen_sentence 我们 时间 发展 工作 科技 教育 社会 生活
gen_sentence 我们 时间 发展 工作 科技 教育 社会 生活 学习 世界

echo "== 组句审计(2/4/8/10 词,最长 20 字) =="
"$work/audit" sentence "$SHARED_DATA_DIR" "$flow_dir" "$sent"

# ---------- 3. 学习会话(Flow 句子提交训练 xhup_flow_user) ----------
# 单词提交经学习 translator 的编码器产生编码词条(动态候选,菜单可观察);
# 句子提交把元素词条写入 canonical 码下(用户权重)。
learn_script=$work/learn.txt
cat > "$learn_script" <<'EOF'
commit womf 1
commit uijm 1
commit womfuijm 1
commit womfuijm 1
EOF
echo "== 学习会话(句子提交 ×2) =="
"$work/audit" learning "$SHARED_DATA_DIR" "$flow_dir" "$learn_script"

# 学习后动态候选码机械发现:导出 userdb,取「文本=我们 且 码≠canonical
# womf」的码(编码器派生的学习词条码;跨会话确定,但依赖词典内容,故
# 不硬编码)。
learned_code=$(
  (cd "$flow_dir" && rime_dict_manager -e xhup_flow_user "$work/userdb.dump" \
     >/dev/null) &&
  awk -F'\t' '$1 == "我们" && $2 != "womf" { print $2; exit }' "$work/userdb.dump"
)
if [ -z "$learned_code" ]; then
  echo "学习状态不可观察:userdb 导出中未发现动态词条" >&2
  exit 1
fi

# ---------- 4. 重启持久化(全新进程断言学习状态仍在) ----------
restart_check=$work/restart-check.txt
printf '# 重启持久化:动态候选仍在;句子仍可组;canonical 码词条仍在\n' \
  > "$restart_check"
printf 'check %s 我们 contains\n' "$learned_code" >> "$restart_check"
printf 'check womfuijm 我们时间 contains\n' >> "$restart_check"
printf 'check womfuijm 我们时间 count=1\n' >> "$restart_check"
echo "== 重启持久化(全新进程;动态码 $learned_code) =="
"$work/audit" learning "$SHARED_DATA_DIR" "$flow_dir" "$restart_check"

# ---------- 5. 学习后静态审计(全部静态 exact code) ----------
echo "== 学习后静态审计(manifest 全量) =="
"$work/audit" static-baseline-learned "$SHARED_DATA_DIR" "$flow_dir" \
  "$MANIFEST"

# ---------- 6. 学习管理端到端(提供 xhup-cli 时) ----------
if [ -n "$XHUP_CLI" ]; then
  # 预留一份未学习的部署副本,作跨目录恢复目标。
  import_dir=$work/import
  cp -r "$flow_dir" "$import_dir"
  rm -rf "$import_dir/xhup_flow_user.userdb" "$import_dir/sync" \
     "$import_dir/xhup_flow_user.userdb.txt" "$import_dir/user.yaml"

  echo "== 学习管理 export → reset → 行为消失 =="
  "$XHUP_CLI" learning export --user-data-dir "$flow_dir" >/dev/null
  test -f "$flow_dir/xhup_flow_user.userdb.txt"
  "$XHUP_CLI" learning reset --user-data-dir "$flow_dir" --yes >/dev/null
  reset_check=$work/reset-check.txt
  printf '# reset 后:动态候选消失;静态候选不变\n' > "$reset_check"
  printf 'check %s 我们 absent\n' "$learned_code" >> "$reset_check"
  printf 'check uijm 时间 first\n' >> "$reset_check"
  "$work/audit" learning "$SHARED_DATA_DIR" "$flow_dir" "$reset_check"

  echo "== 学习管理 import → 跨目录恢复 → 行为恢复 =="
  "$XHUP_CLI" learning import --user-data-dir "$import_dir" \
    --snapshot "$flow_dir/xhup_flow_user.userdb.txt" >/dev/null
  import_check=$work/import-check.txt
  printf '# 跨目录恢复后:动态候选恢复;句子可组\n' > "$import_check"
  printf 'check %s 我们 contains\n' "$learned_code" >> "$import_check"
  printf 'check womfuijm 我们时间 contains\n' >> "$import_check"
  "$work/audit" learning "$SHARED_DATA_DIR" "$import_dir" "$import_check"
else
  echo "== 学习管理端到端跳过(未提供 xhup-cli 路径) =="
fi

echo "----"
echo "Flow 引擎 runtime 审计全部通过(全静态等值 / 组句 / 学习 / 持久化 / 学习后静态保护$( [ -n "$XHUP_CLI" ] && echo ' / 学习管理'))"
