#!/usr/bin/env bash
set -Eeuo pipefail

OLD_SOURCE_PARENT=a072b7efe80f1a32d7b5ba7342559a114edeb12e
SWARM_PARENT=83217e97ec6847db41d757f57279a8b1ca433fe6
SWARM_REF=refs/tags/ripr-release-0.11.0-83217e97ec6847db41d757f57279a8b1ca433fe6
J2=a2a743abd139566499f80cdca8ed999ec4ee01d4
J2_TREE=e9206995263876f087bce0da5f3aeac35895c040
OLD_CONTROL=ce6c3996d9a656180eee7b95ccb03c4327c1e813
VERSION=0.11.0

: "${RUNNER_TEMP:?RUNNER_TEMP is required}"
: "${GITHUB_WORKSPACE:?GITHUB_WORKSPACE is required}"
: "${GITHUB_REPOSITORY:?GITHUB_REPOSITORY is required}"
: "${GITHUB_API_URL:?GITHUB_API_URL is required}"
: "${GITHUB_SERVER_URL:?GITHUB_SERVER_URL is required}"
: "${GH_TOKEN:?GH_TOKEN is required}"

root=$(git -C "$GITHUB_WORKSPACE" rev-parse --show-toplevel)
source_repo="$RUNNER_TEMP/ripr-j5-source"
swarm_repo="$RUNNER_TEMP/ripr-j5-swarm"
j5_dir="$RUNNER_TEMP/ripr-j5-tree"
control_dir="$RUNNER_TEMP/ripr-j5-control"
preflight_out="$RUNNER_TEMP/ripr-j5-preflight"
verify_out="$RUNNER_TEMP/ripr-j5-verification"
packet="$root/target/j5-final"
phase=initialization
status=failed
mkdir -p "$packet"

on_exit() {
  rc=$?
  trap - EXIT
  if test "$status" != success; then
    jq -n \
      --arg phase "$phase" \
      --arg exit_code "$rc" \
      --arg source_parent "${SOURCE_PARENT:-not_resolved}" \
      --arg swarm_parent "$SWARM_PARENT" \
      --arg j5 "${j5:-not_constructed}" \
      --arg j5_tree "${j5_tree:-not_constructed}" \
      --arg control_commit "${control_commit:-not_constructed}" \
      --arg candidate_branch "${J5_BRANCH:-not_published}" \
      --arg control_branch "${CONTROL_BRANCH:-not_published}" \
      --arg pr_number "${pr_number:-not_created}" \
      '{schema:"ripr.source_promotion_j5_builder_failure.v1",status:"failed",phase:$phase,exit_code:($exit_code|tonumber),source_parent:$source_parent,swarm_parent:$swarm_parent,j5:$j5,j5_tree:$j5_tree,control_commit:$control_commit,candidate_branch:$candidate_branch,control_branch:$control_branch,pr_number:$pr_number,claim_boundary:["No merge, version bump, public tag, publication, signing, marketplace mutation, release-secret use, or back-sync was performed."]}' \
      > "$packet/failure.json" || true
  fi
  exit "$rc"
}
trap on_exit EXIT

phase=clone_exact_inputs
rm -rf "$source_repo" "$swarm_repo" "$j5_dir" "$control_dir" "$preflight_out" "$verify_out"
git config user.name EffortlessSteven
git config user.email git@effortlesssteven.com

git clone "$GITHUB_SERVER_URL/$GITHUB_REPOSITORY.git" "$source_repo"
SOURCE_PARENT=$(git -C "$source_repo" rev-parse origin/main)
test "$SOURCE_PARENT" = "$(git -C "$source_repo" ls-remote origin refs/heads/main | awk '{print $1}')"
git -C "$source_repo" fetch --no-tags origin "$OLD_SOURCE_PARENT" "$J2" "$OLD_CONTROL"
test "$(git -C "$source_repo" rev-parse "$J2^{tree}")" = "$J2_TREE"

git clone "$GITHUB_SERVER_URL/EffortlessMetrics/ripr-swarm.git" "$swarm_repo"
git -C "$swarm_repo" fetch origin "$SWARM_PARENT" "$SWARM_REF"
test "$(git -C "$swarm_repo" rev-parse "$SWARM_REF^{commit}")" = "$SWARM_PARENT"
swarm_main=$(git -C "$swarm_repo" rev-parse origin/main)
git -C "$swarm_repo" merge-base --is-ancestor "$SWARM_PARENT" "$swarm_main"
git -C "$swarm_repo" checkout --detach "$SWARM_PARENT"

source_short=${SOURCE_PARENT:0:12}
J5_BRANCH="promote/0.11.0-swarm-w7-${source_short}-j5"
CONTROL_BRANCH="release/0.11.0-promotion-control-${source_short}-w7-j5"

phase=reconcile_live_source_onto_reviewed_j2
git -C "$source_repo" worktree add --detach "$j5_dir" "$J2"
python "$root/tools/release-control/j5/reconcile_tree.py" \
  "$source_repo" "$j5_dir" "$OLD_SOURCE_PARENT" "$SOURCE_PARENT" "$J2" \
  "$packet/source-delta-resolution.json" "$packet/source-changed-paths.json"

policy_work="$RUNNER_TEMP/ripr-j5-process-policy"
mkdir -p "$policy_work"
git -C "$source_repo" show "$OLD_SOURCE_PARENT:policy/process_allowlist.txt" > "$policy_work/old-source.txt"
git -C "$source_repo" show "$J2:policy/process_allowlist.txt" > "$policy_work/j2.txt"
git -C "$source_repo" show "$SOURCE_PARENT:policy/process_allowlist.txt" > "$policy_work/live-source.txt"
python "$root/tools/release-control/j5/reconcile_process_policy.py" \
  "$policy_work/old-source.txt" "$policy_work/j2.txt" "$policy_work/live-source.txt" \
  "$j5_dir" "$j5_dir/policy/process_allowlist.txt" "$packet/process-policy-resolution.json"

git -C "$j5_dir" diff --check
git -C "$j5_dir" add -A
j5_tree=$(git -C "$j5_dir" write-tree)
test "$j5_tree" != "$J2_TREE"
git -C "$source_repo" diff-tree --no-commit-id --name-status -r "$J2_TREE" "$j5_tree" > "$packet/j2-to-j5.name-status"
git -C "$source_repo" diff --stat "$J2_TREE" "$j5_tree" > "$packet/j2-to-j5.stat"
git -C "$source_repo" diff --binary "$J2_TREE" "$j5_tree" > "$packet/j2-to-j5.patch"

phase=construct_exact_direct_join
export GIT_AUTHOR_NAME=EffortlessSteven
export GIT_AUTHOR_EMAIL=git@effortlesssteven.com
export GIT_COMMITTER_NAME=EffortlessSteven
export GIT_COMMITTER_EMAIL=git@effortlesssteven.com
SOURCE_DATE=$(git -C "$source_repo" show -s --format=%cI "$SOURCE_PARENT")
export GIT_AUTHOR_DATE="$SOURCE_DATE"
export GIT_COMMITTER_DATE="$SOURCE_DATE"
j5=$(printf '%s\n' \
  'promote: join live ripr source with frozen W7 for 0.11.0' \
  '' \
  'Carry the live source-parent delta onto the reviewed J2 product tree,' \
  'preserve frozen W7, and retain a direct ordered two-parent join.' \
  | git -C "$source_repo" commit-tree "$j5_tree" -p "$SOURCE_PARENT" -p "$SWARM_PARENT")
test "$(git -C "$source_repo" show -s --format='%P' "$j5")" = "$SOURCE_PARENT $SWARM_PARENT"
test "$(git -C "$source_repo" rev-parse "$j5^{tree}")" = "$j5_tree"

phase=produce_fresh_exact_pair_preflight
mkdir -p "$preflight_out"
CARGO_TARGET_DIR="$RUNNER_TEMP/ripr-j5-w7-target" cargo run --locked \
  --manifest-path "$swarm_repo/Cargo.toml" -p xtask -- \
  source-promotion preflight \
  --source-parent "$SOURCE_PARENT" \
  --swarm-parent "$SWARM_PARENT" \
  --swarm-ref "$SWARM_REF" \
  --source-repo "$source_repo" \
  --swarm-repo "$swarm_repo" \
  --source-main origin/main \
  --swarm-main origin/main \
  --version "$VERSION" \
  --resolved-tree "$j5_tree" \
  --out "$preflight_out"
preflight="$preflight_out/source-promotion-preflight.json"
test -f "$preflight"
jq -e --arg source "$SOURCE_PARENT" --arg swarm "$SWARM_PARENT" --arg tree "$j5_tree" \
  '.schema == "ripr.source_promotion_preflight.v1" and .source_parent == $source and .source_main == $source and .swarm_parent == $swarm and .dry_merge.reviewed_resolved_tree == $tree and .dry_merge.reviewed_resolved_tree_verified == true' \
  "$preflight" >/dev/null
cp "$preflight" "$packet/preflight.json"
cp "$preflight_out/source-promotion-preflight.md" "$packet/preflight.md"

phase=regenerate_complete_resolution_manifest
old_manifest="$RUNNER_TEMP/j2-resolution-manifest.json"
git -C "$source_repo" show "$OLD_CONTROL:docs/release/source-promotion/resolution-manifest.json" > "$old_manifest"
resolution="$RUNNER_TEMP/j5-resolution-manifest.json"
resolution_delta="$RUNNER_TEMP/j5-resolution-refresh.json"
python "$root/tools/release-control/j5/make_manifest.py" \
  "$preflight" "$old_manifest" "$resolution" "$resolution_delta" \
  "$source_repo" "$SOURCE_PARENT" "$SWARM_PARENT" "$j5" "$j5_tree" \
  "$packet/source-changed-paths.json"
cp "$resolution" "$packet/resolution-manifest.json"
cp "$resolution_delta" "$packet/resolution-refresh.json"

phase=trusted_exact_join_verification
mkdir -p "$verify_out"
CARGO_TARGET_DIR="$RUNNER_TEMP/ripr-j5-trusted-target" cargo build --locked \
  --manifest-path "$source_repo/Cargo.toml" -p xtask --bin xtask
trusted_verifier="$RUNNER_TEMP/ripr-j5-trusted-target/debug/xtask"
test -x "$trusted_verifier"
(
  cd "$source_repo"
  "$trusted_verifier" source-promotion verify \
    --preflight "$preflight" \
    --resolution-manifest "$resolution" \
    --join-head "$j5" \
    --source-main "$SOURCE_PARENT" \
    --out "$verify_out"
)
jq -e --arg join "$j5" --arg tree "$j5_tree" --arg source "$SOURCE_PARENT" \
  '.schema == "ripr.source_promotion_verification.v2" and .status == "verified" and .join_head == $join and .source_main == $source and .tree == $tree and .checks.ordered_parents == true and .checks.reviewed_tree == true and .checks.release_version_identity == true' \
  "$verify_out/source-promotion-verification.json" >/dev/null
cp "$verify_out/source-promotion-verification.json" "$packet/verification.json"
cp "$verify_out/source-promotion-verification.md" "$packet/verification.md"

phase=focused_reviewed_tree_proof
git -C "$j5_dir" reset --hard "$j5"
(
  cd "$j5_dir"
  cargo fmt --all -- --check
  git diff --check
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

phase=bind_immutable_control_sidecars
preflight_sha=$(sha256sum "$preflight" | awk '{print $1}')
resolution_sha=$(sha256sum "$resolution" | awk '{print $1}')
contract_inputs="$RUNNER_TEMP/j5-contract-inputs.json"
jq -n --arg source "$SOURCE_PARENT" --arg join "$j5" \
  --arg preflight_sha "$preflight_sha" --arg resolution_sha "$resolution_sha" \
  '{schema:"ripr.source_promotion_ci_inputs.v2",source_main:$source,join_head:$join,preflight:"docs/release/source-promotion/preflight.json",resolution_manifest:"docs/release/source-promotion/resolution-manifest.json",preflight_sha256:$preflight_sha,resolution_manifest_sha256:$resolution_sha}' \
  > "$contract_inputs"
cp "$contract_inputs" "$packet/contract-inputs.json"

git -C "$source_repo" worktree add --detach "$control_dir" "$SOURCE_PARENT"
mkdir -p "$control_dir/docs/release/source-promotion"
cp "$contract_inputs" "$control_dir/docs/release/source-promotion/contract-inputs.json"
cp "$preflight" "$control_dir/docs/release/source-promotion/preflight.json"
cp "$resolution" "$control_dir/docs/release/source-promotion/resolution-manifest.json"
git -C "$control_dir" add docs/release/source-promotion
control_tree=$(git -C "$control_dir" write-tree)
control_commit=$(printf '%s\n' \
  'release(control): bind exact 0.11.0 live-source J5 inputs' \
  '' \
  'Retain the fresh W7 preflight, complete reviewed resolution, and exact J5 identity.' \
  | git -C "$control_dir" commit-tree "$control_tree" -p "$SOURCE_PARENT")
test "$(git -C "$control_dir" merge-base "$control_commit" "$j5" || true)" != "$control_commit"
test "$(git -C "$control_dir" merge-base "$control_commit" "$j5" || true)" != "$j5"

phase=recheck_live_identity_before_publication
live_source=$(git -C "$source_repo" ls-remote origin refs/heads/main | awk '{print $1}')
test "$live_source" = "$SOURCE_PARENT" || {
  echo "source main moved during J5 construction: frozen=$SOURCE_PARENT live=$live_source" >&2
  exit 1
}
live_tag=$(git -C "$swarm_repo" ls-remote origin "$SWARM_REF" | awk '{print $1}')
test "$live_tag" = "$SWARM_PARENT"

phase=publish_exact_candidate_and_control_refs
git -C "$root" fetch "$source_repo" "$j5"
git -C "$root" fetch "$control_dir" "$control_commit"
test "$(git -C "$root" rev-parse "$j5^{commit}")" = "$j5"
test "$(git -C "$root" rev-parse "$control_commit^{commit}")" = "$control_commit"
git -C "$root" push --force origin "$j5:refs/heads/$J5_BRANCH"
git -C "$root" push --force origin "$control_commit:refs/heads/$CONTROL_BRANCH"

phase=create_closed_exact_body_pr
live_source=$(git -C "$source_repo" ls-remote origin refs/heads/main | awk '{print $1}')
test "$live_source" = "$SOURCE_PARENT" || {
  echo "source main moved before PR creation: frozen=$SOURCE_PARENT live=$live_source" >&2
  exit 1
}
owner=${GITHUB_REPOSITORY%%/*}
api="$GITHUB_API_URL/repos/$GITHUB_REPOSITORY"
existing=$(curl --fail-with-body --silent --show-error --get \
  --header 'Accept: application/vnd.github+json' \
  --header "Authorization: Bearer $GH_TOKEN" \
  --header 'X-GitHub-Api-Version: 2022-11-28' \
  --data-urlencode 'state=all' \
  --data-urlencode "head=$owner:$J5_BRANCH" \
  "$api/pulls")
existing_count=$(printf '%s' "$existing" | jq 'length')
test "$existing_count" -le 1
if test "$existing_count" -eq 1; then
  pr_number=$(printf '%s' "$existing" | jq -r '.[0].number')
  test "$(printf '%s' "$existing" | jq -r '.[0].merged_at // ""')" = ""
else
  create_payload=$(jq -n \
    --arg title 'promote: join live ripr source with frozen W7 for 0.11.0' \
    --arg head "$J5_BRANCH" --arg base main \
    --arg body 'Exact J5 body is being bound to the assigned PR number.' \
    '{title:$title,head:$head,base:$base,body:$body,draft:true}')
  created=$(curl --fail-with-body --silent --show-error --request POST \
    --header 'Accept: application/vnd.github+json' \
    --header "Authorization: Bearer $GH_TOKEN" \
    --header 'X-GitHub-Api-Version: 2022-11-28' \
    "$api/pulls" --data "$create_payload")
  pr_number=$(printf '%s' "$created" | jq -r '.number')
fi

awk 'NR <= 80 { print "- `" $0 "`" }' "$packet/j2-to-j5.name-status" > "$packet/j2-to-j5.body-list.md"
cat > "$packet/pr-body.md" <<EOF
<!-- source-promotion: true -->
<!-- source-promotion-control: $control_commit -->

## Exact live-source J5

This fresh direct two-parent object replaces stale J2/#1558 after the source-owned contract repair and all later source-main movement.

\`\`\`text
J5             = $j5
SOURCE_PARENT  = $SOURCE_PARENT
SWARM_PARENT   = $SWARM_PARENT
JOIN_TREE      = $j5_tree
CONTROL        = $control_commit
PREFLIGHT_SHA  = $preflight_sha
RESOLUTION_SHA = $resolution_sha
\`\`\`

## Reviewed movement

The reviewed J2 product tree remains the baseline. J5 carries the complete live source delta from \`$OLD_SOURCE_PARENT\` to \`$SOURCE_PARENT\` through exact source copies or conflict-free three-way integration. Process policy is reconciled by literal owner and cannot widen a maximum implicitly.

The J2-to-J5 delta, source-delta receipt, process-policy receipt, fresh W7 preflight, complete \`kind:key\` manifest, trusted verification, and patch are retained in the construction packet. Frozen W7 is unchanged.

$(cat "$packet/j2-to-j5.body-list.md")

## Required merge transport

Use Create a merge commit.
Do not use Squash and merge.
Do not use Rebase and merge.

\`\`\`bash
gh pr merge $pr_number --repo EffortlessMetrics/ripr --merge --match-head-commit $j5
\`\`\`

Do not append a repair descendant to J5. Any exact-head defect or source-main movement rejects this object and requires a fresh direct join.

## Boundary

No version bump, changelog release entry, public tag, publication, signing, marketplace mutation, release-secret use, merge, or back-sync is included or authorized here.
EOF
pr_body=$(cat "$packet/pr-body.md")
update_payload=$(jq -n --arg body "$pr_body" '{body:$body,state:"closed"}')
updated=$(curl --fail-with-body --silent --show-error --request PATCH \
  --header 'Accept: application/vnd.github+json' \
  --header "Authorization: Bearer $GH_TOKEN" \
  --header 'X-GitHub-Api-Version: 2022-11-28' \
  "$api/pulls/$pr_number" --data "$update_payload")
test "$(printf '%s' "$updated" | jq -r '.state')" = closed
test "$(printf '%s' "$updated" | jq -r '.head.sha')" = "$j5"
test "$(printf '%s' "$updated" | jq -r '.base.sha')" = "$SOURCE_PARENT"
printf '%s\n' "$pr_number" > "$packet/pr-number.txt"

phase=write_success_receipt
jq -n \
  --arg source "$SOURCE_PARENT" --arg old_source "$OLD_SOURCE_PARENT" \
  --arg swarm "$SWARM_PARENT" --arg swarm_main "$swarm_main" \
  --arg j2 "$J2" --arg j2_tree "$J2_TREE" \
  --arg j5 "$j5" --arg j5_tree "$j5_tree" \
  --arg control "$control_commit" --arg preflight_sha "$preflight_sha" \
  --arg resolution_sha "$resolution_sha" --arg j5_branch "$J5_BRANCH" \
  --arg control_branch "$CONTROL_BRANCH" --arg pr_number "$pr_number" \
  '{schema:"ripr.source_promotion_j5_builder.v1",status:"verified_and_published",source_parent:$source,old_source_parent:$old_source,swarm_parent:$swarm,swarm_main_snapshot:$swarm_main,j2:{commit:$j2,tree:$j2_tree},j5:{commit:$j5,tree:$j5_tree,parents:[$source,$swarm],branch:$j5_branch},control:{commit:$control,branch:$control_branch,preflight_sha256:$preflight_sha,resolution_manifest_sha256:$resolution_sha},pull_request:{number:($pr_number|tonumber),state:"closed",reason:"connector reopen triggers hosted pull_request workflows"},proof:["live source frozen and rechecked before publication","unchanged protected W7 tag","fresh W7 producer preflight","complete refreshed kind:key manifest","trusted source-parent verifier status verified","focused J5 Rust and repository policy checks"],non_claims:["no merge","no version change","no public tag","no publication","no signing","no marketplace mutation","no release-secret use","no back-sync"]}' \
  > "$packet/receipt.json"
printf 'SOURCE_PARENT=%s\nSWARM_PARENT=%s\nJ5=%s\nJ5_TREE=%s\nCONTROL_COMMIT=%s\nJ5_BRANCH=%s\nCONTROL_BRANCH=%s\nPR_NUMBER=%s\nPREFLIGHT_SHA256=%s\nRESOLUTION_SHA256=%s\n' \
  "$SOURCE_PARENT" "$SWARM_PARENT" "$j5" "$j5_tree" "$control_commit" "$J5_BRANCH" "$CONTROL_BRANCH" "$pr_number" "$preflight_sha" "$resolution_sha" \
  > "$packet/identities.env"
status=success
phase=complete
