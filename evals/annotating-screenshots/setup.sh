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
cp "$here/shot.png" "$eval_dir/shot.png"
cp -R "$repo/skills/annotating-screenshots" "$eval_dir/skill"

for f in common.md s1.md s2.md s3.md s4.md s5.md; do
  sed "s#\$EVAL_DIR#$eval_dir#g" "$here/$f" > "$eval_dir/$f"
done
cp "$here/s2-prev-spec.json" "$here/s2-prev-report.json" "$here/s4-prev-report.json" "$eval_dir/"

# The "previous results" the fix-a-render scenarios start from.
bin="$eval_dir/bin/markshot"
"$bin" render "$eval_dir/shot.png" --spec "$eval_dir/s2-prev-spec.json" -o "$eval_dir/s2-prev.png" > /dev/null
jq '.spec' "$eval_dir/s4-prev-report.json" \
  | "$bin" render "$eval_dir/shot.png" --spec - -o "$eval_dir/s4-prev.png" > /dev/null

echo "ready: $eval_dir"
