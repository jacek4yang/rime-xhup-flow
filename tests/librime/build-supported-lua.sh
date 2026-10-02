#!/usr/bin/env bash
# Build a pinned, hash-verified test runtime, not a distributed product artifact.
# Ubuntu deps: g++ curl pkg-config librime-dev liblua5.4-dev libopencc-dev
#             libgoogle-glog-dev libboost-dev libmarisa-dev libyaml-cpp-dev darts
set -euo pipefail
output=${1:?"usage: build-supported-lua.sh <new output directory>"}
mkdir "$output"
output=$(cd "$output" && pwd)
revision=68f9c364a2d25a04c7d4794981d7c796b05ab627
sha256=3c4a60bacf8dd6389ca1b4b4889207b8f6c0c6a43e7b848cdac570d592a640b5
curl --fail --location --silent --show-error \
  "https://codeload.github.com/hchunhui/librime-lua/tar.gz/$revision" -o "$output/source.tar.gz"
printf '%s  %s\n' "$sha256" "$output/source.tar.gz" | sha256sum --check
mkdir "$output/source" "$output/objects"
tar -xzf "$output/source.tar.gz" --strip-components=1 -C "$output/source"
for source in "$output"/source/src/*.cc "$output"/source/src/lib/*.cc; do
  # Intentional pkg-config word splitting for compiler/linker flags.
  g++ -std=c++17 -O1 -fPIC $(pkg-config --cflags rime lua5.4 opencc) \
    -c "$source" -o "$output/objects/$(basename "$source").o"
done
for source in "$output"/source/src/lib/*.c; do
  gcc -O1 -fPIC $(pkg-config --cflags lua5.4) \
    -c "$source" -o "$output/objects/$(basename "$source").o"
done
g++ -shared "$output"/objects/*.o $(pkg-config --libs rime lua5.4 opencc) \
  -o "$output/librime-lua.so"
# Resolve all dynamic symbols before installing or running any qualification.
LD_BIND_NOW=1 LD_PRELOAD="$output/librime-lua.so" /bin/true
printf 'Pinned librime-lua %s (source SHA256 %s)\n' "$revision" "$sha256"
