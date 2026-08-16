#!/usr/bin/env bash
set -euo pipefail

: "${RUNNER_TEMP:?RUNNER_TEMP is required}"
: "${GITHUB_WORKSPACE:?GITHUB_WORKSPACE is required}"
: "${GITHUB_RUN_ID:?GITHUB_RUN_ID is required}"

root=$(git -C "$GITHUB_WORKSPACE" rev-parse --show-toplevel)
packet="$root/target/j4-final"
status_repo="$RUNNER_TEMP/ripr-j4-status-repo"
status_branch=control/0.11.0-live-j4-status
mkdir -p "$packet"
if ! test -f "$packet/receipt.json" && ! test -f "$packet/failure.json"; then
  printf '%s\n' '{"schema":"ripr.source_promotion_j4_builder_failure.v1","status":"failed","phase":"workflow_or_shell_initialization","reason":"builder produced neither success nor failure receipt"}' \
    > "$packet/failure.json"
fi
printf '%s\n' "$GITHUB_RUN_ID" > "$packet/workflow-run-id.txt"
rm -rf "$status_repo"
git init "$status_repo"
git -C "$status_repo" config user.name EffortlessSteven
git -C "$status_repo" config user.email git@effortlesssteven.com
cp -a "$packet/." "$status_repo/"
git -C "$status_repo" add -A
git -C "$status_repo" commit -m "release(control): retain live J4 run $GITHUB_RUN_ID"
status_commit=$(git -C "$status_repo" rev-parse HEAD)
git -C "$root" fetch "$status_repo" "$status_commit"
git -C "$root" push --force origin "$status_commit:refs/heads/$status_branch"
