#!/usr/bin/env bash
# XHUP Flow librime runtime 冒烟测试驱动脚本。
#
# 用法: tests/librime/run-runtime-smoke.sh <生成包目录>
#
# 生成包目录必须含 6 个 xhup_flow yaml(xhup-cli generate rime 的输出)。
# 脚本在临时目录搭建隔离的 Rime user 目录(绝不触碰真实用户 Rime 配置),
# 预编译词库后用 librime C API 运行 session 级冒烟测试。
#
# 依赖: rime_deployer(librime-bin)、pkg-config、librime 开发头文件
# (librime-dev)、C 编译器。共享数据目录可用 RIME_SHARED_DATA_DIR 覆盖
# (默认 /usr/share/rime-data,需含 rime-prelude)。

set -euo pipefail

PACKAGE_DIR=${1:?"用法: run-runtime-smoke.sh <生成包目录>"}
SHARED_DATA_DIR=${RIME_SHARED_DATA_DIR:-/usr/share/rime-data}
SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

# 隔离 user 目录:生成包 + schema 激活补丁。
cp "$PACKAGE_DIR"/*.yaml "$work/"
cat > "$work/default.custom.yaml" <<'EOF'
patch:
  schema_list/+:
    - schema: xhup_flow
EOF

# 预编译词库(避免 harness 依赖后台维护线程时序)。
rime_deployer --compile "$work/xhup_flow.schema.yaml" "$work" "$SHARED_DATA_DIR" >/dev/null

# 编译并运行 C harness。
cc -O2 -Wall -Wextra -Werror \
  -o "$work/runtime_smoke" "$SCRIPT_DIR/runtime_smoke.c" \
  $(pkg-config --cflags --libs rime)
"$work/runtime_smoke" "$SHARED_DATA_DIR" "$work"
