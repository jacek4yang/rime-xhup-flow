#!/usr/bin/env bash
# XHUP Flow 引擎 runtime 审计驱动:全静态等值 / 组句 / 学习 / 持久化 /
# 静态保护 / 学习管理端到端。
#
# 用法: run-flow-audit.sh <生成包目录> <全静态菜单 manifest> [xhup-cli 路径|-]
#   [extended manifest] [open-composition manifest]
#
# 学习/持久化聚焦模式: XHUP_AUDIT_ONLY_LEARNING=1 run-flow-audit.sh <生成包目录>
#   [xhup-cli 路径] —— 跳过需要全静态 manifest 的穷尽遍历(步骤 1/5),
#   只跑组句 + 学习会话 + userdb 导出可观察性 + 重启持久化 + 学习管理,
#   分钟级;PR 关键路径(ci.yml)与本地定位学习/导出回归使用。
#
# 生成包目录必须含 xhup-cli generate rime 的全部产物(12 个 yaml,含
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
#   4. 重启持久化:全新进程经 userdb 导出与菜单断言学习条目仍在且可用;
#   5. 学习后静态审计:全部 141,138 个静态 exact code
#      逐码断言既有候选
#      原次序、原 top1、无可见重复(动态候选只允许追加在静态组后);
#   6. 学习管理端到端(提供 xhup-cli 时):export → reset → 学习状态
#      消失(导出为空)→ import 到全新部署 → 学习状态恢复。
#
# 学习状态可观察性(步骤 3/4/6)一律经产品同款管理路径
# (rime_dict_manager -e / `xhup-cli learning export`)断言:静态层只存在于
# table/prism,永不进入用户词典,故「干净 userdb 导出不含该词形」与
# 「学习后导出含该词形且码唯一」合起来构成不可能由静态词条满足的动态证据。
#
# 部署说明:PRIMARY/FIXED_FIRST 是主词典 import table,已由默认
# translator 统一编译。rime_deployer --compile 只编译默认 translator
# 命名空间的词典;Flow/学习词典按词典在独立目录编译后拷入部署 build/
# (同目录连续 wrapper 编译会相互干扰)。menu/page_size: 500 只存在于
# 测试 default.custom.yaml,不写入 production schema。
# v2 统一主词典使 Flow translator 的单进程全量查询明显变慢;两遍静态
# 审计把同一 manifest 确定性拆成两个互斥 shard,各自在隔离部署副本上
# 并行执行。分片前后数据行数必须严格相等,不减少任何 exact code 覆盖。
#
# 依赖: rime_deployer、rime_dict_manager(librime-bin)、pkg-config、
# librime 开发头文件(librime-dev)、C 编译器。共享数据目录可用
# RIME_SHARED_DATA_DIR 覆盖(默认 /usr/share/rime-data,需含 rime-prelude)。

set -euo pipefail

PACKAGE_DIR=${1:?"用法: run-flow-audit.sh <生成包目录> [静态菜单 manifest] [xhup-cli 路径]"}
# 学习/持久化聚焦模式:跳过步骤 1/5(需要全静态 manifest 的穷尽遍历),
# 位置参数变为 <生成包目录> [xhup-cli 路径]。
ONLY_LEARNING=${XHUP_AUDIT_ONLY_LEARNING:-}
# Replay reuses only the isolated Flow deployment, not static traversal.
if [[ "${XHUP_AUDIT_ONLY_REPLAY:-0}" == 1 ]]; then ONLY_LEARNING=1; fi
if [ -n "$ONLY_LEARNING" ]; then
  MANIFEST=
  XHUP_CLI=${2:-}
  EXTENDED_MANIFEST=
  OPEN_MANIFEST=
else
  MANIFEST=${2:-}
  XHUP_CLI=${3:-}
  EXTENDED_MANIFEST=${4:-}
  OPEN_MANIFEST=${5:-}
fi
if [[ "$XHUP_CLI" == "-" ]]; then XHUP_CLI=; fi
if [[ -z "$ONLY_LEARNING" && -z "$MANIFEST" ]]; then
  echo "用法: run-flow-audit.sh <生成包目录> <静态菜单 manifest> [xhup-cli 路径] [extended manifest] [open manifest]" >&2
  echo "      学习聚焦模式: XHUP_AUDIT_ONLY_LEARNING=1 run-flow-audit.sh <生成包目录> [xhup-cli 路径]" >&2
  exit 2
fi
SHARED_DATA_DIR=${RIME_SHARED_DATA_DIR:-/usr/share/rime-data}
SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
python3 "$SCRIPT_DIR/../release/check_generated_runtime_sources.py" "$PACKAGE_DIR"

work=$(mktemp -d)
if [[ "${XHUP_AUDIT_KEEP_WORK:-0}" == 1 ]]; then
  trap 'echo "Retained synthetic audit directory: $work" >&2' EXIT
else
  trap 'rm -rf "$work"' EXIT
fi

CFLAGS="-O2 -Wall -Wextra -Werror"

# 独立目录编译非默认命名空间词典,产物拷入部署 build/(确定性,无串扰)。
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

# 搭建部署:包 yaml + schema_list/menu patch + 主词典编译。
prepare_deploy() {
  local dir=$1 schema_id=$2
  mkdir -p "$dir"
  cp "$PACKAGE_DIR"/*.yaml "$dir/"
  # 方案引用 lua_filter 时必须随包携带 lua/ 模块,否则在装有
  # librime-lua 的环境里组件创建失败会导致引擎无候选。
  if [[ -d "$PACKAGE_DIR/lua" ]]; then cp -r "$PACKAGE_DIR/lua" "$dir/"; fi
  cat > "$dir/default.custom.yaml" <<EOF
patch:
  schema_list/+:
    - schema: $schema_id
  menu/page_size: 500
EOF
  if [[ "${XHUP_AUDIT_ONLY_REPLAY:-0}" == 1 ]]; then
    printf 'patch:\n  flow/enable_user_dict: false\n  learn/enable_user_dict: false\n' > "$dir/xhup_flow.custom.yaml"
    printf 'patch:\n  schema_list/+ :\n    - schema: xhup_flow\n  menu/page_size: 5\n' > "$dir/default.custom.yaml"
    if [[ "${XHUP_CORPUS_MODE:-planner}" == native-only ]]; then
      [[ -n "${XHUP_REPLAY_CORPUS:-}" ]] || { echo "ablation requires corpus mode" >&2; exit 2; }
      cp "$SCRIPT_DIR/../quality/corpus_native_only.lua" "$dir/lua/"
      printf '  engine/translators/@2: lua_translator@*corpus_native_only\n' >> "$dir/xhup_flow.custom.yaml"
    fi
  fi
  rime_deployer --compile "$dir/$schema_id.schema.yaml" "$dir" \
    "$SHARED_DATA_DIR" >/dev/null
}

# 经产品同款管理路径导出 Flow 用户词典(rime_dict_manager -e 与
# `xhup-cli learning export` 同一底层工具)。数据库不存在/打开失败 =
# 无学习数据:导出文件保持为空,不视为错误。
export_userdb() {
  local dir=$1 out=$2
  rm -f "$out"
  (cd "$dir" && rime_dict_manager -e xhup_flow_user "$out" >/dev/null 2>&1) \
    || true
  [ -f "$out" ] || : > "$out"
}

# 导出文件中指定词形的全部码(每行一个;空输出 = 该词形不在用户词典)。
exported_codes_for() {
  awk -F'\t' -v text="$2" '$1 == text { print $2 }' "$1"
}

# 导出文件中的全部「词形<TAB>码」行(跳过 #@ 元数据行)。
exported_entries() {
  awk -F'\t' '$1 != "" && $2 != "" && $1 !~ /^#/ { print $1 "\t" $2 }' "$1"
}

# ---------- 1. 全静态等值审计(干净 userdb,两趟独立进程) ----------
# STATIC 部署只在全量模式需要(步骤 1 的 STATIC 捕获基线)。
if [ -z "$ONLY_LEARNING" ]; then
static_dir=$work/static
prepare_deploy "$static_dir" xhup_flow_static
fi

flow_dir=$work/flow
prepare_deploy "$flow_dir" xhup_flow
for dict in xhup_flow_learn; do
  compile_dict_isolated "$dict" "$flow_dir"
done

if [[ "${XHUP_AUDIT_ONLY_REPLAY:-0}" == 1 ]]; then
  if [[ -n "${XHUP_REPLAY_CORPUS:-}" ]]; then
    cc $CFLAGS -std=c11 "$SCRIPT_DIR/runtime_corpus.c" $(pkg-config --cflags --libs rime) -o "$work/corpus"
    "$work/corpus" "$flow_dir" "$SHARED_DATA_DIR" "$XHUP_REPLAY_CORPUS" "${XHUP_CORPUS_MODE:-planner}"
    exit $?
  fi
  cc $CFLAGS -std=c11 "$SCRIPT_DIR/runtime_replay.c" $(pkg-config --cflags --libs rime) -o "$work/replay"
  "$work/replay" "$flow_dir" "$SHARED_DATA_DIR" "${XHUP_REPLAY_MODE:---qualify}"
  cc $CFLAGS -std=c11 "$SCRIPT_DIR/runtime_extended.c" $(pkg-config --cflags --libs rime) -o "$work/extended"
  "$work/extended" "$flow_dir" "$SHARED_DATA_DIR" --stress
  if [[ "${XHUP_REPLAY_VERIFY_READONLY:-0}" == 1 ]]; then
    # Intentionally request learning on the OLD binding. Partial API backports
    # may initialize empty metadata, but must disconnect before any user update.
    printf 'patch:\n  flow/enable_user_dict: true\n  learn/enable_user_dict: true\n' > "$flow_dir/xhup_flow.custom.yaml"
    rime_deployer --compile "$flow_dir/xhup_flow.schema.yaml" "$flow_dir" "$SHARED_DATA_DIR" >/dev/null
    for restart in 1 2; do
      "$work/extended" "$flow_dir" "$SHARED_DATA_DIR" --learn-unavailable
      if [[ -e "$flow_dir/xhup_flow_user.userdb" ]]; then
        (cd "$flow_dir" && rime_dict_manager -e xhup_flow_user "$work/readonly-$restart.tsv")
      else
        : > "$work/readonly-$restart.tsv"
      fi
      [[ -z "$(exported_entries "$work/readonly-$restart.tsv")" ]] || {
        echo "FAIL old binding learned records" >&2; exit 1;
      }
    done
    cmp "$work/readonly-1.tsv" "$work/readonly-2.tsv"
    echo "PASS old binding: no learned entries, unchanged native export across restart"
  fi
  if [[ "${XHUP_REPLAY_VERIFY_LEARNING:-0}" == 1 ]]; then
    if [[ -d "$flow_dir/xhup_flow_user.userdb" ]]; then
      (cd "$flow_dir" && rime_dict_manager -e xhup_flow_user "$work/off.tsv")
      [[ -z "$(exported_entries "$work/off.tsv")" ]] || { echo "FAIL learning-off wrote records" >&2; exit 1; }
    fi
    printf 'patch:\n  flow/enable_user_dict: true\n  learn/enable_user_dict: true\n' > "$flow_dir/xhup_flow.custom.yaml"
    rime_deployer --compile "$flow_dir/xhup_flow.schema.yaml" "$flow_dir" "$SHARED_DATA_DIR" >/dev/null
    for count in 1 2; do
      "$work/extended" "$flow_dir" "$SHARED_DATA_DIR" --learn-once
      (cd "$flow_dir" && rime_dict_manager -e xhup_flow_user "$work/once.tsv")
      python3 - "$work/once.tsv" "$count" <<'PY'
import re
import sys
rows = {}
for line in open(sys.argv[1], encoding="utf-8"):
    if line.startswith("#"):
        continue
    fields = line.rstrip("\n").split("\t")
    if len(fields) >= 3:
        match = re.search(r"(?:^|\s)c=(\d+)", fields[2])
        count = int(match.group(1)) if match else int(fields[2].strip())
        rows[(fields[0], fields[1].strip())] = count
for key in [("你", "ni"), ("好", "hcnz"), ("去", "qu")]:
    assert rows.get(key) == int(sys.argv[2]), (key, rows.get(key), sys.argv[2])
print("PASS one native update per component per commit across process restart")
PY
    done
    "$work/replay" "$flow_dir" "$SHARED_DATA_DIR" --learn
    (cd "$flow_dir" && rime_dict_manager -e xhup_flow_user "$work/learn.tsv")
    # Exact boundary-provider learning identity; no delimiter or foreign code.
    grep -E '^你[[:space:]]+ni[[:space:]]' "$work/learn.tsv"
    grep -E '^好[[:space:]]+hcnz[[:space:]]' "$work/learn.tsv"
    "$work/replay" "$flow_dir" "$SHARED_DATA_DIR" --learn
    echo 'PASS native genuine identity, variant code persistence and process restart' >&2
    # Refuse updates using the native persisted tick, without a second Lua ledger.
    printf 'patch:\n  flow/enable_user_dict: true\n  flow/learning_max_updates: 0\n  learn/enable_user_dict: true\n' > "$flow_dir/xhup_flow.custom.yaml"
    rime_deployer --compile "$flow_dir/xhup_flow.schema.yaml" "$flow_dir" "$SHARED_DATA_DIR" >/dev/null
    (cd "$flow_dir" && rime_dict_manager -e xhup_flow_user "$work/quota-before.tsv")
    "$work/extended" "$flow_dir" "$SHARED_DATA_DIR" --learn-blocked
    (cd "$flow_dir" && rime_dict_manager -e xhup_flow_user "$work/quota-after.tsv")
    cmp "$work/quota-before.tsv" "$work/quota-after.tsv"
    echo 'PASS native quota refuses mutation across restart while typing still works' >&2
  fi
  exit 0
fi

cc $CFLAGS -o "$work/audit" "$SCRIPT_DIR/runtime_flow_audit.c" \
  $(pkg-config --cflags --libs rime)

# 审计分片数。默认 2 与历史 CI 行为一致;可用环境变量覆盖以便在**实验分支**
# 上验证「更多分片能否缩短墙钟而不触发 OOM」——本 job 是 CI 最长的门禁
# (3h16m–5h00m,曾在 #113 上撞满 300 分钟超时)。
#
# 注意:每分片都会 `cp -a` 一份完整数据目录并加载完整 librime 词典,因此
# 提高分片数会增加峰值内存;默认值保持 2 正是出于该内存余量的保守选择。
# 门禁语义与分片数无关(分片只是把同一份 manifest 切分并行处理)。
AUDIT_SHARDS=${XHUP_AUDIT_SHARDS:-2}
if ! [[ "$AUDIT_SHARDS" =~ ^[1-9][0-9]*$ ]]; then
  echo "XHUP_AUDIT_SHARDS 必须是正整数,实际: $AUDIT_SHARDS" >&2
  exit 2
fi

# 步骤 1 只在全量模式运行(需要全静态 manifest)。
if [ -z "$ONLY_LEARNING" ]; then
manifest_rows=$(grep -vc '^#' "$MANIFEST")
manifest_shards=()
for ((i = 0; i < AUDIT_SHARDS; ++i)); do
  shard="$work/static-manifest-$i"
  : > "$shard"
  manifest_shards+=("$shard")
done
awk -v n="$AUDIT_SHARDS" -v prefix="$work/static-manifest-" '
  /^#/ { next }
  { print > (prefix ((rows++) % n)) }
' "$MANIFEST"
shard_rows=0
for shard in "${manifest_shards[@]}"; do
  rows=$(wc -l < "$shard")
  shard_rows=$((shard_rows + rows))
done
[[ "$shard_rows" -eq "$manifest_rows" ]] || {
  echo "静态 manifest 分片丢行:原始 $manifest_rows / 分片 $shard_rows" >&2
  exit 2
}

echo "== 全静态等值审计(干净 userdb;STATIC 捕获 → FLOW 对照) =="
audit_pids=()
audit_logs=()
for ((i = 0; i < AUDIT_SHARDS; ++i)); do
  static_clone="$work/static-clean-$i"
  flow_clone="$work/flow-clean-$i"
  capture="$work/static-$i.capture"
  log="$work/static-clean-$i.log"
  cp -a "$static_dir" "$static_clone"
  cp -a "$flow_dir" "$flow_clone"
  (
    "$work/audit" baseline-capture "$SHARED_DATA_DIR" "$static_clone" \
      "${manifest_shards[$i]}" "$capture"
    "$work/audit" baseline-compare "$SHARED_DATA_DIR" "$flow_clone" \
      "${manifest_shards[$i]}" "$capture"
  ) > "$log" 2>&1 &
  audit_pids+=("$!")
  audit_logs+=("$log")
done
audit_status=0
audit_rss_peaks=()
for i in "${!audit_pids[@]}"; do
  pid="${audit_pids[$i]}"
  # 采样该分片进程的峰值 RSS(§22 要求测量 deploy/runtime 内存;CI 无基线)。
  # 后台采样,避免 wait 期间拿不到数据。
  (
    peak=0
    while kill -0 "$pid" 2>/dev/null; do
      # 优先 VmHWM(历史峰值 RSS,单调不减,不依赖采样频率);
      # 该字段在部分环境(如 Git-Bash 的 /proc 仿真)不存在,此时退回
      # VmRSS(当前值)并对子进程一起采样,取观察到的最大值。
      # 若两者都取不到,保持 0 —— **不得**伪造读数。
      for p in "$pid" $(pgrep -P "$pid" 2>/dev/null); do
        status_file="/proc/$p/status"
        [ -r "$status_file" ] || continue
        hwm=$(awk '/^VmHWM:/ { print $2 }' "$status_file" 2>/dev/null || true)
        [ -z "${hwm:-}" ] && hwm=$(awk '/^VmRSS:/ { print $2 }' "$status_file" 2>/dev/null || true)
        if [ -n "${hwm:-}" ] && [ "$hwm" -gt "$peak" ] 2>/dev/null; then peak=$hwm; fi
      done
      sleep 2
    done
    if [ "$peak" -eq 0 ]; then
      echo "警告: 无法采样分片 $i 的 RSS(VmHWM/VmRSS 均不可读)" >&2
    fi
    echo "$peak" > "$work/audit-rss-$i"
  ) &
  audit_rss_peaks+=("$!")
done
for pid in "${audit_pids[@]}"; do
  if ! wait "$pid"; then audit_status=1; fi
done
for sampler in "${audit_rss_peaks[@]}"; do wait "$sampler" 2>/dev/null || true; done
for i in "${!audit_pids[@]}"; do
  rss=$(cat "$work/audit-rss-$i" 2>/dev/null || echo 0)
  echo "librime 审计分片 $i 峰值 RSS: ${rss} KiB"
done
for log in "${audit_logs[@]}"; do cat "$log"; done
[[ "$audit_status" -eq 0 ]] || exit 1
fi

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

# P0 开放输入回归：前四条验证 attested 字符与旧 Top-N 外来源词，
# 「提嗯诶」明确不存在于 hot/extended 词表，只能由逐字原语组句；其余为
# 包含单字/语气词/结构助词的真实句子，不从词典机械挑词。
cat >> "$sent" <<'EOF'
eiyu	诶
ogkx	嗯
enkx	嗯
tiuici	提示词
tiogei	提嗯诶
enwojtdeveyhjqkeyile	嗯我觉得这样就可以了
einizfmeyezdveli	诶你怎么也在这里
wojtdevegeuurufaxmzdhcdolene	我觉得这个输入法现在好多了呢
vegetiuiciykgdmwyzufmewfti	这个提示词应该没有什么问题
wojbtmvybwjixuwjujvegexlmu	我今天准备继续完善这个项目
EOF

echo "== 组句审计(已知词 + attested 字符 + 词表外组合 + 真实口语长句) =="
"$work/audit" sentence "$SHARED_DATA_DIR" "$flow_dir" "$sent"

# ---------- 2b. 全扩展词 + 确定性开放组合 runtime 可达性 ----------
run_sharded_reachability() {
  local manifest=$1 label=$2 shard_count=$3
  local rows shard_rows=0 status=0
  rows=$(grep -vc '^#' "$manifest")
  local pids=() logs=()
  for ((i = 0; i < shard_count; ++i)); do
    : > "$work/reach-$label-$i"
  done
  awk -v n="$shard_count" -v prefix="$work/reach-$label-" '
    /^#/ { next }
    { print > (prefix ((rows++) % n)) }
  ' "$manifest"
  for ((i = 0; i < shard_count; ++i)); do
    local shard="$work/reach-$label-$i"
    local clone="$work/reach-deploy-$label-$i"
    local log="$work/reach-$label-$i.log"
    shard_rows=$((shard_rows + $(wc -l < "$shard")))
    cp -a "$flow_dir" "$clone"
    ("$work/audit" contains-manifest "$SHARED_DATA_DIR" "$clone" "$shard") \
      > "$log" 2>&1 &
    pids+=("$!")
    logs+=("$log")
  done
  [[ "$shard_rows" -eq "$rows" ]] || {
    echo "$label manifest 分片丢行:原始 $rows / 分片 $shard_rows" >&2
    exit 2
  }
  for pid in "${pids[@]}"; do if ! wait "$pid"; then status=1; fi; done
  for log in "${logs[@]}"; do cat "$log"; done
  [[ "$status" -eq 0 ]] || exit 1
}

if [[ -n "$EXTENDED_MANIFEST" ]]; then
  echo "== 全扩展词 exact runtime 可达性审计 =="
  run_sharded_reachability "$EXTENDED_MANIFEST" extended 4
fi
if [[ -n "$OPEN_MANIFEST" ]]; then
  echo "== 1000 条确定性词表外开放组合 runtime 审计 =="
  run_sharded_reachability "$OPEN_MANIFEST" open 2
fi

# ---------- 3. 学习会话(Flow 句子提交训练 xhup_flow_user) ----------
# 单词提交把元素词条写入用户词典(用户权重);句子提交把组句元素词条写入
# 各自 canonical 码下。学习状态**必须**能经产品同款管理路径
# (rime_dict_manager -e / `xhup-cli learning export`)观察。
#
# 学习状态契约(替代旧的「导出码 ≠ canonical 输入码」启发式:#152 统一
# Flow/Learn 音节 id 空间后,同一元素不再被写成两个码,该启发式已无对象):
#   1) 静态层只存在于 table/prism,永不进入 userdb —— 「干净 userdb 导出不含
#      该词形」+「学习后导出含该词形」合起来即可证明动态学习状态,不可能由
#      既有静态词条满足;
#   2) 同一词形在 userdb 中只能对应一个码:Flow 与 Learn translator 共享
#      xhup_flow_user,两者音节 id 空间必须一致,否则同一元素会被写成两个码
#      (历史缺陷:learn 表把 flow 表的音节 id 译成别的词条码);
#   3) 该码在全新进程中必须仍能定位到该词形(重启持久化)。
learn_script=$work/learn.txt
cat > "$learn_script" <<'EOF'
commit-text womf 我们
commit-text uijm 时间
commit-text womfuijm 我们时间
commit-text womfuijm 我们时间
EOF

# 干净对照:学习前导出不得含将被学习的词形。
clean_dump=$work/userdb-clean.dump
export_userdb "$flow_dir" "$clean_dump"
if [ -n "$(exported_codes_for "$clean_dump" 我们)" ]; then
  echo "干净 userdb 导出已含「我们」:导出未反映学习状态" >&2
  exit 1
fi

echo "== 学习会话(句子提交 ×2) =="
"$work/audit" learning "$SHARED_DATA_DIR" "$flow_dir" "$learn_script"

learned_dump=$work/userdb-learned.dump
export_userdb "$flow_dir" "$learned_dump"
learned_codes=$(exported_codes_for "$learned_dump" 我们 | sort -u)
learned_code_count=$(printf '%s' "$learned_codes" | grep -c . || true)
if [ "$learned_code_count" -eq 0 ]; then
  echo "学习状态不可观察:userdb 导出中未发现动态词条" >&2
  exit 1
fi
if [ "$learned_code_count" -ne 1 ]; then
  echo "学习词条码不唯一(Flow/Learn 音节 id 空间可能已分叉):" >&2
  printf '  %s\n' $learned_codes >&2
  exit 1
fi
learned_code=$learned_codes
learned_commits=$(awk -F'\t' -v text=我们 '$1 == text { print $3; exit }' "$learned_dump")
learned_tick=$(awk -F'\t' '$1 == "#@/tick" { print $2; exit }' "$learned_dump")
[ "${learned_commits:-0}" -ge 1 ] || {
  echo "学习词条 commits 未推进: ${learned_commits:-0}" >&2
  exit 1
}
[ "${learned_tick:-0}" -ge 1 ] || {
  echo "userdb /tick 未推进: ${learned_tick:-0}" >&2
  exit 1
}
echo "学习状态可观察: 我们@$learned_code commits=$learned_commits tick=$learned_tick"

# ---------- 4. 重启持久化(全新进程断言学习状态仍在且可用) ----------
restart_check=$work/restart-check.txt
printf '# 重启持久化:用户词典词条仍在且可在其码下定位;句子仍可组\n' \
  > "$restart_check"
# 每条 userdb 词条(词形 T,码 C)在全新进程中都必须能在 C 下定位到 T:
# 覆盖「学习词条落到别的词条码」的串码回归,且不硬编码任何词形/码。
while IFS=$'\t' read -r text code; do
  [ -n "$text" ] && [ -n "$code" ] || continue
  printf 'check %s %s contains\n' "$code" "$text" >> "$restart_check"
done < <(exported_entries "$learned_dump")
printf 'check womfuijm 我们时间 contains\n' >> "$restart_check"
printf 'check womfuijm 我们时间 count=1\n' >> "$restart_check"
echo "== 重启持久化(全新进程;学习码 $(printf '%s' "$learned_codes" | tr '\n' ' ')) =="
"$work/audit" learning "$SHARED_DATA_DIR" "$flow_dir" "$restart_check"

# ---------- 5. 学习后静态审计(全部静态 exact code) ----------
# 只在全量模式运行(需要全静态 manifest)。
if [ -z "$ONLY_LEARNING" ]; then
echo "== 学习后静态审计(manifest 全量) =="
audit_pids=()
audit_logs=()
for ((i = 0; i < AUDIT_SHARDS; ++i)); do
  learned_clone="$work/flow-learned-$i"
  log="$work/static-learned-$i.log"
  cp -a "$flow_dir" "$learned_clone"
  (
    "$work/audit" static-baseline-learned "$SHARED_DATA_DIR" \
      "$learned_clone" "${manifest_shards[$i]}"
  ) > "$log" 2>&1 &
  audit_pids+=("$!")
  audit_logs+=("$log")
done
audit_status=0
audit_rss_peaks=()
for i in "${!audit_pids[@]}"; do
  pid="${audit_pids[$i]}"
  # 采样该分片进程的峰值 RSS(§22 要求测量 deploy/runtime 内存;CI 无基线)。
  # 后台采样,避免 wait 期间拿不到数据。
  (
    peak=0
    while kill -0 "$pid" 2>/dev/null; do
      # 优先 VmHWM(历史峰值 RSS,单调不减,不依赖采样频率);
      # 该字段在部分环境(如 Git-Bash 的 /proc 仿真)不存在,此时退回
      # VmRSS(当前值)并对子进程一起采样,取观察到的最大值。
      # 若两者都取不到,保持 0 —— **不得**伪造读数。
      for p in "$pid" $(pgrep -P "$pid" 2>/dev/null); do
        status_file="/proc/$p/status"
        [ -r "$status_file" ] || continue
        hwm=$(awk '/^VmHWM:/ { print $2 }' "$status_file" 2>/dev/null || true)
        [ -z "${hwm:-}" ] && hwm=$(awk '/^VmRSS:/ { print $2 }' "$status_file" 2>/dev/null || true)
        if [ -n "${hwm:-}" ] && [ "$hwm" -gt "$peak" ] 2>/dev/null; then peak=$hwm; fi
      done
      sleep 2
    done
    if [ "$peak" -eq 0 ]; then
      echo "警告: 无法采样分片 $i 的 RSS(VmHWM/VmRSS 均不可读)" >&2
    fi
    echo "$peak" > "$work/audit-rss-$i"
  ) &
  audit_rss_peaks+=("$!")
done
for pid in "${audit_pids[@]}"; do
  if ! wait "$pid"; then audit_status=1; fi
done
for sampler in "${audit_rss_peaks[@]}"; do wait "$sampler" 2>/dev/null || true; done
for i in "${!audit_pids[@]}"; do
  rss=$(cat "$work/audit-rss-$i" 2>/dev/null || echo 0)
  echo "librime 审计分片 $i 峰值 RSS: ${rss} KiB"
done
for log in "${audit_logs[@]}"; do cat "$log"; done
[[ "$audit_status" -eq 0 ]] || exit 1
fi

# ---------- 6. 学习管理端到端(提供 xhup-cli 时) ----------
if [ -n "$XHUP_CLI" ]; then
  # 预留一份未学习的部署副本,作跨目录恢复目标。
  import_dir=$work/import
  cp -r "$flow_dir" "$import_dir"
  rm -rf "$import_dir/xhup_flow_user.userdb" "$import_dir/sync" \
     "$import_dir/xhup_flow_user.userdb.txt" "$import_dir/user.yaml"

  echo "== 学习管理 export → reset → 学习状态消失 =="
  "$XHUP_CLI" learning export --user-data-dir "$flow_dir" >/dev/null
  test -f "$flow_dir/xhup_flow_user.userdb.txt"
  "$XHUP_CLI" learning reset --user-data-dir "$flow_dir" --yes >/dev/null
  # reset 后导出必须为空:静态层不参与 userdb,故「无该词形」= 学习状态已清空
  # (旧断言用「动态码下菜单不含它」,依赖已不存在的串码词条)。
  reset_dump=$work/userdb-reset.dump
  export_userdb "$flow_dir" "$reset_dump"
  if [ -n "$(exported_codes_for "$reset_dump" 我们)" ]; then
    echo "learning reset 后 userdb 导出仍含学习词条" >&2
    exit 1
  fi
  reset_check=$work/reset-check.txt
  printf '# reset 后:静态候选不变\n' > "$reset_check"
  printf 'check uijm 时间 first\n' >> "$reset_check"
  "$work/audit" learning "$SHARED_DATA_DIR" "$flow_dir" "$reset_check"

  echo "== 学习管理 import → 跨目录恢复 → 学习状态恢复 =="
  "$XHUP_CLI" learning import --user-data-dir "$import_dir" \
    --snapshot "$flow_dir/xhup_flow_user.userdb.txt" >/dev/null
  import_dump=$work/userdb-import.dump
  export_userdb "$import_dir" "$import_dump"
  imported_codes=$(exported_codes_for "$import_dump" 我们 | sort -u)
  if [ "$imported_codes" != "$learned_codes" ]; then
    echo "import 后 userdb 未恢复学习词条码: ${imported_codes:-<空>}" >&2
    exit 1
  fi
  import_check=$work/import-check.txt
  printf '# 跨目录恢复后:词条在其码下可定位;句子可组\n' > "$import_check"
  while IFS=$'\t' read -r text code; do
    [ -n "$text" ] && [ -n "$code" ] || continue
    printf 'check %s %s contains\n' "$code" "$text" >> "$import_check"
  done < <(exported_entries "$import_dump")
  printf 'check womfuijm 我们时间 contains\n' >> "$import_check"
  "$work/audit" learning "$SHARED_DATA_DIR" "$import_dir" "$import_check"
else
  echo "== 学习管理端到端跳过(未提供 xhup-cli 路径) =="
fi

echo "----"
echo "Flow 引擎 runtime 审计全部通过(全静态等值 / 组句 / 学习 / 持久化 / 学习后静态保护$( [ -n "$XHUP_CLI" ] && echo ' / 学习管理'))"
