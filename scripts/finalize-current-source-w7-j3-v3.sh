#!/usr/bin/env bash
set -euo pipefail

source_script="scripts/finalize-current-source-w7-j3.sh"
patched_script="${RUNNER_TEMP:?}/finalize-current-source-w7-j3-v3.sh"
cp "$source_script" "$patched_script"

python - "$patched_script" <<'PY'
from pathlib import Path
import sys

path = Path(sys.argv[1])
text = path.read_text(encoding="utf-8")
start_marker = "stage=verify_resolution_authority\n"
end_marker = "\nstage=candidate_focused_proof\n"
start = text.index(start_marker)
end = text.index(end_marker, start)
replacement = r'''stage=verify_resolution_authority
# The accepted J2 manifest contains reviewed integrated/source-survivor
# dispositions whose final candidate blobs intentionally differ from the raw
# source parent. Current-source carry therefore proves the actual delta:
# every path outside #1560's exact five-file set remains byte-identical to J2,
# while the three source-owned changed paths below equal current source and
# the two integrated paths retain their explicit reviewed compositions.
for path in \
  .github/workflows/source-promotion-contract.yml \
  docs/specs/RIPR-SPEC-0150-source-promotion-ci-contract.md \
  xtask/tests/source_promotion_workflow_contract.rs
do
  candidate_blob=$(git -C "$candidate_dir" rev-parse "$new_tree:$path")
  source_blob=$(git -C "$source_dir" rev-parse "$SOURCE_PARENT:$path")
  if test "$candidate_blob" != "$source_blob"; then
    printf 'current source-owned delta mismatch for %s: candidate=%s source=%s\n' \
      "$path" "$candidate_blob" "$source_blob" >&2
    exit 1
  fi
  rows=$(jq -r --arg path "$path" \
    '[.dispositions[] | select(.kind == "source_survivor" and .key == $path and .disposition == "source_blob")] | length' \
    "$resolution")
  test "$rows" = 1 || {
    printf 'expected one current source_blob disposition for %s, found %s\n' "$path" "$rows" >&2
    exit 1
  }
done

test "$(jq -r '[.dispositions[] | select(.kind == "source_survivor" and .key == "xtask/src/command.rs" and .disposition == "integrated")] | length' "$resolution")" = 1
test "$(jq -r '[.dispositions[] | select(.key == "policy/process_allowlist.txt" and .disposition == "integrated")] | length' "$resolution")" = 2
test "$(jq -r '[.dispositions[] | select(.kind == "conflict")] | length' "$resolution")" = 20
test "$(jq -r '[.dispositions[] | select(.kind == "source_survivor")] | length' "$resolution")" = 100
test "$(jq -r '[.dispositions[] | select(.kind == "swarm_authority")] | length' "$resolution")" = 57
'''
path.write_text(text[:start] + replacement + text[end:], encoding="utf-8")
PY

bash "$patched_script"
