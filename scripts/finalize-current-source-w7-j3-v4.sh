#!/usr/bin/env bash
set -euo pipefail

source_script="scripts/finalize-current-source-w7-j3.sh"
patched_script="${RUNNER_TEMP:?}/finalize-current-source-w7-j3-v4.sh"
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
# source parent. Prove only the exact five-file #1560 carry and print every
# compared identity before enforcing it.
for path in \
  .github/workflows/source-promotion-contract.yml \
  docs/specs/RIPR-SPEC-0150-source-promotion-ci-contract.md \
  xtask/tests/source_promotion_workflow_contract.rs
do
  candidate_blob=$(git -C "$candidate_dir" rev-parse "$new_tree:$path")
  source_blob=$(git -C "$source_dir" rev-parse "$SOURCE_PARENT:$path")
  row_json=$(jq -c --arg path "$path" \
    '[.dispositions[] | select(.kind == "source_survivor" and .key == $path)]' \
    "$resolution")
  rows=$(jq -r 'length' <<<"$row_json")
  disposition=$(jq -r 'if length == 1 then .[0].disposition else "invalid-cardinality" end' <<<"$row_json")
  printf 'resolution_check path=%s candidate_blob=%s source_blob=%s rows=%s disposition=%s row=%s\n' \
    "$path" "$candidate_blob" "$source_blob" "$rows" "$disposition" "$row_json"
  if test "$candidate_blob" != "$source_blob"; then
    printf 'current source-owned delta mismatch for %s\n' "$path" >&2
    exit 1
  fi
  if test "$rows" != 1 || test "$disposition" != source_blob; then
    printf 'current source-owned manifest row mismatch for %s\n' "$path" >&2
    exit 1
  fi
done

command_count=$(jq -r '[.dispositions[] | select(.kind == "source_survivor" and .key == "xtask/src/command.rs" and .disposition == "integrated")] | length' "$resolution")
allow_count=$(jq -r '[.dispositions[] | select(.key == "policy/process_allowlist.txt" and .disposition == "integrated")] | length' "$resolution")
conflict_count=$(jq -r '[.dispositions[] | select(.kind == "conflict")] | length' "$resolution")
survivor_count=$(jq -r '[.dispositions[] | select(.kind == "source_survivor")] | length' "$resolution")
authority_count=$(jq -r '[.dispositions[] | select(.kind == "swarm_authority")] | length' "$resolution")
printf 'resolution_counts command_integrated=%s allowlist_integrated=%s conflicts=%s survivors=%s swarm_authority=%s\n' \
  "$command_count" "$allow_count" "$conflict_count" "$survivor_count" "$authority_count"
if test "$command_count" != 1 || test "$allow_count" != 2 || \
   test "$conflict_count" != 20 || test "$survivor_count" != 100 || \
   test "$authority_count" != 57; then
  printf 'reviewed resolution denominator or integrated-row count mismatch\n' >&2
  exit 1
fi
'''
path.write_text(text[:start] + replacement + text[end:], encoding="utf-8")
PY

bash "$patched_script"
