#!/usr/bin/env bash
# Linux-only measurement runtime. Never install or distribute this plugin/VM.
# Source checkouts are read-only object stores; dirty working files are ignored.
set -euo pipefail
output=${1:?"usage: build-profile-runtime.sh <new output directory> <Lua git checkout> <librime-lua git checkout>"}
lua_repo=${2:?"missing Lua git checkout"}
plugin_repo=${3:?"missing librime-lua git checkout"}
root=$(cd "$(dirname "$0")" && pwd)
lua_revision=6e22fedb74cf0c9b6656e9fce8b7331db847c605
plugin_revision=68f9c364a2d25a04c7d4794981d7c796b05ab627
mkdir "$output"
output=$(cd "$output" && pwd)
mkdir "$output/lua" "$output/plugin" "$output/objects"
git -C "$lua_repo" archive "$lua_revision" | tar -x -C "$output/lua"
git -C "$plugin_repo" archive "$plugin_revision" | tar -x -C "$output/plugin"
git -C "$output/lua" apply "$root/profile-lua-5.4.8.patch"
git -C "$output/plugin" apply "$root/profile-librime-lua.patch"
gcc -std=c11 -O2 -fPIC -shared -DMAKE_LIB -DLUA_USE_LINUX -D_XOPEN_SOURCE=700 \
  -include "$root/profile_runtime.h" "$output/lua/onelua.c" "$root/profile_runtime.c" \
  -lm -ldl -Wl,-soname,liblua5.4.so.0 -o "$output/liblua5.4.so.0"
gcc -std=c11 -O2 -Wall -Wextra -Werror -I"$output/lua" "$root/profile_vm_check.c" \
  "$output/liblua5.4.so.0" -Wl,-rpath,'$ORIGIN' -o "$output/profile_vm_check"
"$output/profile_vm_check"
for source in "$output"/plugin/src/*.cc "$output"/plugin/src/lib/*.cc; do
  # Match the supported plugin build flags; only the VM and probes differ.
  g++ -std=c++17 -O1 -fPIC -fvisibility=hidden -include "$root/profile_runtime.h" \
    -I"$output/lua" $(pkg-config --cflags rime opencc) \
    -c "$source" -o "$output/objects/$(basename "$source").o"
done
for source in "$output"/plugin/src/lib/*.c; do
  gcc -O1 -fPIC -fvisibility=hidden -I"$output/lua" \
    -c "$source" -o "$output/objects/$(basename "$source").o"
done
g++ -shared "$output"/objects/*.o $(pkg-config --libs rime opencc) \
  "$output/liblua5.4.so.0" -Wl,-rpath,'$ORIGIN' -o "$output/librime-lua.so"
LD_BIND_NOW=1 LD_PRELOAD="$output/librime-lua.so" /bin/true
{
  printf 'lua_commit=%s\nlibrime_lua_commit=%s\n' "$lua_revision" "$plugin_revision"
  gcc --version
  g++ --version
  pkg-config --modversion rime opencc
  sha256sum "$root/build-profile-runtime.sh" "$root"/profile* "$output/liblua5.4.so.0" "$output/librime-lua.so" "$output/profile_vm_check"
} > "$output/provenance.txt"
printf 'Measurement runtime: %s\nDo not use instrumented timings as production latency.\n' "$output"
