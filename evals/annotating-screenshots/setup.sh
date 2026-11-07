#!/usr/bin/env bash
# Prepares a scratch directory for the annotating-screenshots skill evaluations.
# Usage: bash evals/annotating-screenshots/setup.sh <eval-dir>   (from the repo root)
set -euo pipefail

if [ $# -ne 1 ]; then
  echo "usage: bash evals/annotating-screenshots/setup.sh <eval-dir>" >&2
  exit 1
fi
eval_dir=$(mkdir -p "$1" && cd "$1" && pwd)
repo=$(cd "$(dirname "$0")/../.." && pwd)
here="$repo/evals/annotating-screenshots"

cargo build --release --locked --quiet --manifest-path "$repo/Cargo.toml"
mkdir -p "$eval_dir/bin" "$eval_dir/runs"
ln -sf "$repo/target/release/markshot" "$eval_dir/bin/markshot"
cp "$here/shot.png" "$here/console.png" "$eval_dir/"
# The same screenshot without its pHYs chunk, as some tools save Retina captures.
python3 - "$here/shot.png" "$eval_dir/shot-nodpi.png" <<'PY'
import struct, sys
data = open(sys.argv[1], "rb").read()
out, i = bytearray(data[:8]), 8
while i < len(data):
    n = struct.unpack(">I", data[i:i + 4])[0]
    if data[i + 4:i + 8] != b"pHYs":
        out += data[i:i + 12 + n]
    i += 12 + n
open(sys.argv[2], "wb").write(out)
PY
rm -rf "$eval_dir/skill"
cp -R "$repo/skills/annotating-screenshots" "$eval_dir/skill"

for f in "$here"/common.md "$here"/s[0-9]*.md; do
  sed "s#\$EVAL_DIR#$eval_dir#g" "$f" > "$eval_dir/$(basename "$f")"
done
cp "$here/s2-prev-spec.json" "$here/s2-prev-report.json" "$here/s4-prev-report.json" "$eval_dir/"

# The "previous results" the fix-a-render scenarios start from.
bin="$eval_dir/bin/markshot"
"$bin" render "$eval_dir/shot.png" --spec "$eval_dir/s2-prev-spec.json" -o "$eval_dir/s2-prev.png" > /dev/null
jq '.spec' "$eval_dir/s4-prev-report.json" \
  | "$bin" render "$eval_dir/shot.png" --spec - -o "$eval_dir/s4-prev.png" > /dev/null

echo "ready: $eval_dir"
