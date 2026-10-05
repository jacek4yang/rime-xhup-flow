#!/usr/bin/env bash
# Verify the actual compiled selector, including an old selected foreign schema.
set -euo pipefail
package="${1:?generated package required}"
shared="${RIME_SHARED_DATA_DIR:-/usr/share/rime-data}"
here="$(cd "$(dirname "$0")" && pwd)"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
mkdir "$work/user"
cp -a "$package/." "$work/user/"
# Foreign schemas remain on disk; installation must hide, not erase, them.
for schema in luna_pinyin bopomofo; do
  printf 'schema:\n  schema_id: %s\n  name: Preserved foreign schema\n  version: "1"\nengine:\n  processors: []\n  translators: []\n' "$schema" > "$work/user/$schema.schema.yaml"
done
printf 'var:\n  previously_selected_schema: luna_pinyin\n' > "$work/user/user.yaml"
# Simulate the non-XHUP system preset. Generated custom config must replace its
# entire schema_list rather than append XHUP or rely on an empty clean profile.
cp "$shared/default.yaml" "$work/user/default.yaml"
python3 - "$work/user/default.custom.yaml" <<'PY'
from pathlib import Path
import sys
source = Path(sys.argv[1]).read_text(encoding='utf-8')
assert 'schema_list:' in source and 'schema_list/+' not in source
assert source.count('schema: ') == 1 and 'schema: xhup_flow\n' in source
PY
rime_deployer --build "$work/user" "$shared" "$work/user/build" > "$work/deploy.log" 2>&1 || { head -c 8000 "$work/deploy.log"; exit 1; }
cc -std=c11 -Wall -Wextra -Werror "$here/exclusive_schema.c" $(pkg-config --cflags --libs rime) -o "$work/check"
"$work/check" "$shared" "$work/user"
test -f "$work/user/luna_pinyin.schema.yaml"
test -f "$work/user/bopomofo.schema.yaml"
echo 'PASS foreign schema files preserved while excluded from the selector'
