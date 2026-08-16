#!/usr/bin/env bash
set -euo pipefail

SOURCE_PARENT=6cc5d6135593d9fb9a745eb215c5b0f92cbd14d5
OLD_SOURCE_PARENT=a072b7efe80f1a32d7b5ba7342559a114edeb12e
SWARM_PARENT=83217e97ec6847db41d757f57279a8b1ca433fe6
SWARM_REF=refs/tags/ripr-release-0.11.0-83217e97ec6847db41d757f57279a8b1ca433fe6
J2=a2a743abd139566499f80cdca8ed999ec4ee01d4
J2_TREE=e9206995263876f087bce0da5f3aeac35895c040
OLD_CONTROL=ce6c3996d9a656180eee7b95ccb03c4327c1e813
J3_BRANCH=promote/0.11.0-swarm-w7-6cc5d6-j3
CONTROL_BRANCH=release/0.11.0-promotion-control-6cc5d6-w7-j3
VERSION=0.11.0

root=$(git rev-parse --show-toplevel)
source_repo="$RUNNER_TEMP/ripr-j3-source"
swarm_repo="$RUNNER_TEMP/ripr-j3-swarm"
j3_dir="$RUNNER_TEMP/ripr-j3-tree"
control_dir="$RUNNER_TEMP/ripr-j3-control"
preflight_out="$RUNNER_TEMP/ripr-j3-preflight"
verify_out="$RUNNER_TEMP/ripr-j3-verification"
packet="$root/target/j3-final"
rm -rf "$source_repo" "$swarm_repo" "$j3_dir" "$control_dir" "$preflight_out" "$verify_out" "$packet"
mkdir -p "$packet"

git config user.name EffortlessSteven
git config user.email git@effortlesssteven.com

git clone https://github.com/EffortlessMetrics/ripr.git "$source_repo"
git -C "$source_repo" fetch --no-tags origin "$SOURCE_PARENT" "$OLD_SOURCE_PARENT" "$J2" "$OLD_CONTROL"
test "$(git -C "$source_repo" rev-parse origin/main)" = "$SOURCE_PARENT"
test "$(git -C "$source_repo" rev-parse "$J2^{tree}")" = "$J2_TREE"

git clone https://github.com/EffortlessMetrics/ripr-swarm.git "$swarm_repo"
git -C "$swarm_repo" fetch origin "$SWARM_PARENT" "$SWARM_REF"
test "$(git -C "$swarm_repo" rev-parse "$SWARM_REF^{commit}")" = "$SWARM_PARENT"
swarm_main=$(git -C "$swarm_repo" rev-parse origin/main)
git -C "$swarm_repo" merge-base --is-ancestor "$SWARM_PARENT" "$swarm_main"
git -C "$swarm_repo" checkout --detach "$SWARM_PARENT"

cat > "$RUNNER_TEMP/j3-changed-paths.txt" <<'PATHS'
.github/workflows/source-promotion-contract.yml
docs/specs/RIPR-SPEC-0150-source-promotion-ci-contract.md
policy/process_allowlist.txt
xtask/src/command.rs
xtask/tests/source_promotion_workflow_contract.rs
PATHS
sort "$RUNNER_TEMP/j3-changed-paths.txt" -o "$RUNNER_TEMP/j3-changed-paths.txt"
git -C "$source_repo" diff --name-only "$OLD_SOURCE_PARENT..$SOURCE_PARENT" | sort > "$RUNNER_TEMP/source-parent-diff.txt"
diff -u "$RUNNER_TEMP/j3-changed-paths.txt" "$RUNNER_TEMP/source-parent-diff.txt"
python - "$RUNNER_TEMP/j3-changed-paths.txt" "$RUNNER_TEMP/j3-changed-paths.json" <<'PY'
import json
from pathlib import Path
import sys
values = [line for line in Path(sys.argv[1]).read_text(encoding="utf-8").splitlines() if line]
Path(sys.argv[2]).write_text(json.dumps(values, indent=2) + "\n", encoding="utf-8")
PY

git -C "$source_repo" worktree add --detach "$j3_dir" "$J2"
while IFS= read -r path; do
  test -n "$path" || continue
  work="$RUNNER_TEMP/merge-${path//\//_}"
  mkdir -p "$work"
  cp "$j3_dir/$path" "$work/current"
  git -C "$source_repo" show "$OLD_SOURCE_PARENT:$path" > "$work/base"
  git -C "$source_repo" show "$SOURCE_PARENT:$path" > "$work/source"
  set +e
  git merge-file -p "$work/current" "$work/base" "$work/source" > "$work/merged"
  status=$?
  set -e
  if test "$status" -ne 0; then
    echo "three-way source repair integration failed for $path with status $status" >&2
    cat "$work/merged" >&2 || true
    exit 1
  fi
  cp "$work/merged" "$j3_dir/$path"
done < "$RUNNER_TEMP/j3-changed-paths.txt"

if grep -R -n -E '^(<<<<<<<|=======|>>>>>>>)' \
  "$j3_dir/.github/workflows/source-promotion-contract.yml" \
  "$j3_dir/docs/specs/RIPR-SPEC-0150-source-promotion-ci-contract.md" \
  "$j3_dir/policy/process_allowlist.txt" \
  "$j3_dir/xtask/src/command.rs" \
  "$j3_dir/xtask/tests/source_promotion_workflow_contract.rs"; then
  echo "conflict markers remain in reviewed J3 integration" >&2
  exit 1
fi

git -C "$j3_dir" diff --check
git -C "$j3_dir" add \
  .github/workflows/source-promotion-contract.yml \
  docs/specs/RIPR-SPEC-0150-source-promotion-ci-contract.md \
  policy/process_allowlist.txt \
  xtask/src/command.rs \
  xtask/tests/source_promotion_workflow_contract.rs
j3_tree=$(git -C "$j3_dir" write-tree)
git -C "$source_repo" diff-tree --no-commit-id --name-only -r "$J2_TREE" "$j3_tree" | sort > "$RUNNER_TEMP/j3-tree-diff.txt"
diff -u "$RUNNER_TEMP/j3-changed-paths.txt" "$RUNNER_TEMP/j3-tree-diff.txt"

export GIT_AUTHOR_NAME=EffortlessSteven
export GIT_AUTHOR_EMAIL=git@effortlesssteven.com
export GIT_COMMITTER_NAME=EffortlessSteven
export GIT_COMMITTER_EMAIL=git@effortlesssteven.com
export GIT_AUTHOR_DATE=2026-08-16T02:35:00Z
export GIT_COMMITTER_DATE=2026-08-16T02:35:00Z
j3=$(printf '%s\n' \
  'promote: join repaired source with frozen W7 for 0.11.0' \
  '' \
  'Refresh the exact source parent after #1560 while preserving frozen W7' \
  'and the reviewed J2 product tree, including the catalog integration.' \
  | git -C "$source_repo" commit-tree "$j3_tree" -p "$SOURCE_PARENT" -p "$SWARM_PARENT")
test "$(git -C "$source_repo" show -s --format='%P' "$j3")" = "$SOURCE_PARENT $SWARM_PARENT"
test "$(git -C "$source_repo" rev-parse "$j3^{tree}")" = "$j3_tree"

mkdir -p "$preflight_out"
CARGO_TARGET_DIR="$RUNNER_TEMP/ripr-j3-w7-target" \
  cargo run --locked --manifest-path "$swarm_repo/Cargo.toml" -p xtask -- \
  source-promotion preflight \
  --source-parent "$SOURCE_PARENT" \
  --swarm-parent "$SWARM_PARENT" \
  --swarm-ref "$SWARM_REF" \
  --source-repo "$source_repo" \
  --swarm-repo "$swarm_repo" \
  --source-main origin/main \
  --swarm-main origin/main \
  --version "$VERSION" \
  --resolved-tree "$j3_tree" \
  --out "$preflight_out"
preflight="$preflight_out/source-promotion-preflight.json"
test -f "$preflight"
jq -e \
  --arg source "$SOURCE_PARENT" \
  --arg swarm "$SWARM_PARENT" \
  --arg tree "$j3_tree" \
  '.schema == "ripr.source_promotion_preflight.v1" and .source_parent == $source and .source_main == $source and .swarm_parent == $swarm and .dry_merge.reviewed_resolved_tree == $tree and .dry_merge.reviewed_resolved_tree_verified == true' \
  "$preflight" >/dev/null

old_manifest="$RUNNER_TEMP/j2-resolution-manifest.json"
git -C "$source_repo" show "$OLD_CONTROL:docs/release/source-promotion/resolution-manifest.json" > "$old_manifest"
resolution="$RUNNER_TEMP/j3-resolution-manifest.json"
resolution_delta="$RUNNER_TEMP/j3-resolution-refresh.json"
python "$root/tools/release-control/j3/make_manifest.py" \
  "$preflight" "$old_manifest" "$resolution" "$resolution_delta" \
  "$source_repo" "$SOURCE_PARENT" "$SWARM_PARENT" "$j3" "$j3_tree" \
  "$RUNNER_TEMP/j3-changed-paths.json"

mkdir -p "$verify_out"
CARGO_TARGET_DIR="$RUNNER_TEMP/ripr-j3-trusted-target" \
  cargo build --locked --manifest-path "$source_repo/Cargo.toml" -p xtask --bin xtask
trusted_verifier="$RUNNER_TEMP/ripr-j3-trusted-target/debug/xtask"
test -x "$trusted_verifier"
(
  cd "$source_repo"
  "$trusted_verifier" source-promotion verify \
    --preflight "$preflight" \
    --resolution-manifest "$resolution" \
    --join-head "$j3" \
    --source-main "$SOURCE_PARENT" \
    --out "$verify_out"
)
jq -e \
  --arg join "$j3" --arg tree "$j3_tree" --arg source "$SOURCE_PARENT" \
  '.schema == "ripr.source_promotion_verification.v2" and .status == "verified" and .join_head == $join and .source_main == $source and .tree == $tree and .checks.ordered_parents == true and .checks.reviewed_tree == true and .checks.release_version_identity == true' \
  "$verify_out/source-promotion-verification.json" >/dev/null

git -C "$j3_dir" reset --hard "$j3"
(
  cd "$j3_dir"
  cargo fmt --all -- --check
  cargo test --locked -p xtask --test source_promotion_workflow_contract
  cargo test --locked -p xtask source_promotion_workflow -- --nocapture
  cargo test --locked -p xtask command_catalog_ci_enforced_flags_match_repo_workflows -- --nocapture
  cargo run --locked -p xtask -- check-command-catalog
  cargo run --locked -p xtask -- check-workflows
  cargo run --locked -p xtask -- check-process-policy
  cargo run --locked -p xtask -- check-spec-format
  cargo run --locked -p xtask -- check-traceability
  cargo run --locked -p xtask -- check-doc-artifacts
)

preflight_sha=$(sha256sum "$preflight" | awk '{print $1}')
resolution_sha=$(sha256sum "$resolution" | awk '{print $1}')
contract_inputs="$RUNNER_TEMP/j3-contract-inputs.json"
jq -n \
  --arg source "$SOURCE_PARENT" \
  --arg join "$j3" \
  --arg preflight_sha "$preflight_sha" \
  --arg resolution_sha "$resolution_sha" \
  '{schema:"ripr.source_promotion_ci_inputs.v2",source_main:$source,join_head:$join,preflight:"docs/release/source-promotion/preflight.json",resolution_manifest:"docs/release/source-promotion/resolution-manifest.json",preflight_sha256:$preflight_sha,resolution_manifest_sha256:$resolution_sha}' \
  > "$contract_inputs"

git -C "$source_repo" worktree add --detach "$control_dir" "$SOURCE_PARENT"
mkdir -p "$control_dir/docs/release/source-promotion"
cp "$contract_inputs" "$control_dir/docs/release/source-promotion/contract-inputs.json"
cp "$preflight" "$control_dir/docs/release/source-promotion/preflight.json"
cp "$resolution" "$control_dir/docs/release/source-promotion/resolution-manifest.json"
git -C "$control_dir" add docs/release/source-promotion
export GIT_AUTHOR_DATE=2026-08-16T02:36:00Z
export GIT_COMMITTER_DATE=2026-08-16T02:36:00Z
control_tree=$(git -C "$control_dir" write-tree)
control_commit=$(printf '%s\n' \
  'release(control): bind exact 0.11.0 J3 promotion inputs' \
  '' \
  'Retain the fresh W7 preflight, reviewed resolution, and exact J3 sidecars.' \
  | git -C "$control_dir" commit-tree "$control_tree" -p "$SOURCE_PARENT")
test "$(git -C "$control_dir" merge-base "$control_commit" "$j3" || true)" != "$control_commit"
test "$(git -C "$control_dir" merge-base "$control_commit" "$j3" || true)" != "$j3"

live_source=$(git -C "$source_repo" ls-remote origin refs/heads/main | awk '{print $1}')
test "$live_source" = "$SOURCE_PARENT"
live_tag=$(git -C "$swarm_repo" ls-remote origin "$SWARM_REF" | awk '{print $1}')
test "$live_tag" = "$SWARM_PARENT"

git -C "$source_repo" push origin "$j3:refs/heads/$J3_BRANCH"
git -C "$control_dir" push origin "$control_commit:refs/heads/$CONTROL_BRANCH"

cp "$contract_inputs" "$packet/contract-inputs.json"
cp "$preflight" "$packet/preflight.json"
cp "$resolution" "$packet/resolution-manifest.json"
cp "$resolution_delta" "$packet/resolution-refresh.json"
cp "$verify_out/source-promotion-verification.json" "$packet/verification.json"
cp "$verify_out/source-promotion-verification.md" "$packet/verification.md"
cp "$RUNNER_TEMP/j3-changed-paths.json" "$packet/changed-paths.json"
git -C "$source_repo" diff --stat "$J2_TREE" "$j3_tree" > "$packet/j2-to-j3.stat"
git -C "$source_repo" diff --binary "$J2_TREE" "$j3_tree" > "$packet/j2-to-j3.patch"
jq -n \
  --arg source "$SOURCE_PARENT" \
  --arg old_source "$OLD_SOURCE_PARENT" \
  --arg swarm "$SWARM_PARENT" \
  --arg swarm_main "$swarm_main" \
  --arg j2 "$J2" \
  --arg j2_tree "$J2_TREE" \
  --arg j3 "$j3" \
  --arg j3_tree "$j3_tree" \
  --arg control "$control_commit" \
  --arg preflight_sha "$preflight_sha" \
  --arg resolution_sha "$resolution_sha" \
  --arg j3_branch "$J3_BRANCH" \
  --arg control_branch "$CONTROL_BRANCH" \
  '{schema:"ripr.source_promotion_j3_builder.v1",source_parent:$source,old_source_parent:$old_source,swarm_parent:$swarm,swarm_main_snapshot:$swarm_main,j2:{commit:$j2,tree:$j2_tree},j3:{commit:$j3,tree:$j3_tree,parents:[$source,$swarm],branch:$j3_branch},control:{commit:$control,branch:$control_branch,preflight_sha256:$preflight_sha,resolution_manifest_sha256:$resolution_sha},proof:["fresh W7 producer preflight","reviewed five-path three-way source repair integration","trusted source verifier status verified","focused J3 Rust and policy checks"],non_claims:["no merge","no version change","no tag","no publication","no signing","no marketplace mutation","no secret use","no back-sync"]}' \
  > "$packet/receipt.json"
printf 'J3=%s\nJ3_TREE=%s\nCONTROL_COMMIT=%s\nSOURCE_PARENT=%s\nSWARM_PARENT=%s\nPREFLIGHT_SHA256=%s\nRESOLUTION_SHA256=%s\n' \
  "$j3" "$j3_tree" "$control_commit" "$SOURCE_PARENT" "$SWARM_PARENT" "$preflight_sha" "$resolution_sha" \
  | tee "$packet/identities.env"
