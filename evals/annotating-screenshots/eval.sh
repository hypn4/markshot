#!/usr/bin/env bash
# Runs the annotating-screenshots skill evaluations, all but the agents themselves: whoever runs
# the eval starts one fresh agent per prompt file between `prompts` and `score` (see README.md).
#   bash evals/annotating-screenshots/eval.sh setup <eval-dir>
#   bash evals/annotating-screenshots/eval.sh prompts <eval-dir> <skill|control>
#   bash evals/annotating-screenshots/eval.sh score <eval-dir> [skill|control]
set -euo pipefail

here=$(cd "$(dirname "$0")" && pwd)

usage() {
  echo "usage: bash evals/annotating-screenshots/eval.sh setup <eval-dir>" >&2
  echo "       bash evals/annotating-screenshots/eval.sh prompts <eval-dir> <skill|control>" >&2
  echo "       bash evals/annotating-screenshots/eval.sh score <eval-dir> [skill|control]" >&2
  exit 1
}

case "${1:-}" in
setup)
  [ $# -eq 2 ] || usage
  bash "$here/setup.sh" "$2"
  ;;
prompts)
  # One prompt per scenario: common.md with its SKILL_LINE filled in, the run directory, the request.
  [ $# -eq 3 ] || usage
  dir=$(cd "$2" && pwd)
  case "$3" in
    skill) line="Before starting, read the agent skill $dir/skill/SKILL.md and follow it." ;;
    control) line="No usage guide is provided; use the CLI's --help and schema." ;;
    *) usage ;;
  esac
  if [ ! -f "$dir/common.md" ]; then
    echo "error: $dir/common.md is missing; run setup first" >&2
    exit 1
  fi
  for f in "$dir"/s[0-9]*.md; do
    n=$(basename "$f" .md)
    {
      while IFS= read -r l; do
        if [ "$l" = SKILL_LINE ]; then printf '%s\n' "$line"; else printf '%s\n' "$l"; fi
      done < "$dir/common.md"
      echo "RUN_DIR=$dir/runs/$3-$n"
      cat "$f"
    } > "$dir/prompt-$3-$n.md"
    echo "$dir/prompt-$3-$n.md"
  done
  ;;
score)
  # check.py on every run directory <variant>-sN[-anything], one row each; exit 1 if any fails.
  [ $# -eq 2 ] || [ $# -eq 3 ] || usage
  dir=$(cd "$2" && pwd)
  EVAL_DIR="$dir" uv run --quiet --no-project --with pillow python - "$here/check.py" "$dir/runs" "${3:-}" <<'PY'
import json, os, re, subprocess, sys
check, runs, variant = sys.argv[1:]
found = []
for name in os.listdir(runs) if os.path.isdir(runs) else []:
    m = re.fullmatch(r"(.+?)-s(\d+)(-.+)?", name)
    if m and os.path.isdir(f"{runs}/{name}") and variant in ("", m[1]):
        found.append((m[1], int(m[2]), m[3] or "", name))
if not found:
    sys.exit(f"error: no runs in {runs}" + (f" for {variant}" if variant else ""))
failed = 0
print(f"{'variant':<9} {'scenario':<10} {'result':<6} failing keys  (notes to judge)")
for v, n, rest, name in sorted(found):
    p = subprocess.run([sys.executable, check, f"s{n}", f"{runs}/{name}"], capture_output=True, text=True)
    try:
        r = json.loads(p.stdout)
    except ValueError:
        r = {"error": (p.stderr.strip().splitlines() or ["no output"])[-1]}
    bad = [k for k, x in r.items() if x is False]
    notes = [f"{k}={x}" for k, x in r.items() if not isinstance(x, bool)]
    ok = not bad and "error" not in r
    failed += not ok
    note = f"  ({'; '.join(notes)})" if notes else ""
    print(f"{v:<9} {'s%d%s' % (n, rest):<10} {'pass' if ok else 'FAIL':<6} {' '.join(bad) or '-'}{note}")
sys.exit(1 if failed else 0)
PY
  ;;
*)
  usage
  ;;
esac
