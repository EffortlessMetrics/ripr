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
source_repo="$RUNNER_TEMP/ripr-j4-source"
swarm_repo="$RUNNER_TEMP/ripr-j4-swarm"
j4_dir="$RUNNER_TEMP/ripr-j4-tree"
control_dir="$RUNNER_TEMP/ripr-j4-control"
preflight_out="$RUNNER_TEMP/ripr-j4-preflight"
verify_out="$RUNNER_TEMP/ripr-j4-verification"
packet="$root/target/j4-final"
phase=initialization
status=failed
mkdir -p "$packet"

write_failure() {
  rc=$?
  if test "$status" = success; then
    return 0
  fi
  jq -n \
    --arg schema ripr.source_promotion_j4_builder_failure.v1 \
    --arg phase "$phase" \
    --arg exit_code "$rc" \
    --arg source_parent "${SOURCE_PARENT:-not_resolved}" \
    --arg swarm_parent "$SWARM_PARENT" \
    --arg j4 "${j4:-not_constructed}" \
    --arg j4_tree "${j4_tree:-not_constructed}" \
    --arg control_commit "${control_commit:-not_constructed}" \
    --arg candidate_branch "${J4_BRANCH:-not_published}" \
    --arg control_branch "${CONTROL_BRANCH:-not_published}" \
    --arg pr_number "${pr_number:-not_created}" \
    '{schema:$schema,status:"failed",phase:$phase,exit_code:($exit_code|tonumber),source_parent:$source_parent,swarm_parent:$swarm_parent,j4:$j4,j4_tree:$j4_tree,control_commit:$control_commit,candidate_branch:$candidate_branch,control_branch:$control_branch,pr_number:$pr_number,claim_boundary:["No merge, tag, version bump, publication, signing, marketplace mutation, secret release use, or back-sync was performed."]}' \
    > "$packet/failure.json" || true
}
trap write_failure EXIT

phase=clone_exact_inputs
rm -rf "$source_repo" "$swarm_repo" "$j4_dir" "$control_dir" "$preflight_out" "$verify_out"
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
J4_BRANCH="promote/0.11.0-swarm-w7-${source_short}-j4"
CONTROL_BRANCH="release/0.11.0-promotion-control-${source_short}-w7-j4"

phase=reconcile_live_source_onto_reviewed_j2
git -C "$source_repo" worktree add --detach "$j4_dir" "$J2"
python "$root/tools/release-control/j4/reconcile_tree.py" \
  "$source_repo" "$j4_dir" "$OLD_SOURCE_PARENT" "$SOURCE_PARENT" "$J2" \
  "$packet/source-delta-resolution.json" "$packet/source-changed-paths.json"

policy_work="$RUNNER_TEMP/ripr-j4-process-policy"
mkdir -p "$policy_work"
git -C "$source_repo" show "$OLD_SOURCE_PARENT:policy/process_allowlist.txt" > "$policy_work/old-source.txt"
git -C "$source_repo" show "$J2:policy/process_allowlist.txt" > "$policy_work/j2.txt"
git -C "$source_repo" show "$SOURCE_PARENT:policy/process_allowlist.txt" > "$policy_work/live-source.txt"
python "$root/tools/release-control/j4/reconcile_process_policy_v2.py" \
  "$policy_work/old-source.txt" "$policy_work/j2.txt" "$policy_work/live-source.txt" \
  "$j4_dir" "$j4_dir/policy/process_allowlist.txt" "$packet/process-policy-resolution.json"

if grep -R -n -E '^(<<<<<<<|=======|>>>>>>>)' "$j4_dir" \
  --exclude-dir=.git --exclude-dir=target; then
  echo "conflict markers remain in the reviewed J4 tree" >&2
  exit 1
fi
git -C "$j4_dir" diff --check
git -C "$j4_dir" add -A
j4_tree=$(git -C "$j4_dir" write-tree)
test "$j4_tree" != "$J2_TREE"
git -C "$source_repo" diff-tree --no-commit-id --name-status -r "$J2_TREE" "$j4_tree" \
  > "$packet/j2-to-j4.name-status"
git -C "$source_repo" diff --stat "$J2_TREE" "$j4_tree" > "$packet/j2-to-j4.stat"
git -C "$source_repo" diff --binary "$J2_TREE" "$j4_tree" > "$packet/j2-to-j4.patch"

phase=construct_exact_direct_join
export GIT_AUTHOR_NAME=EffortlessSteven
export GIT_AUTHOR_EMAIL=git@effortlesssteven.com
export GIT_COMMITTER_NAME=EffortlessSteven
export GIT_COMMITTER_EMAIL=git@effortlesssteven.com
SOURCE_DATE=$(git -C "$source_repo" show -s --format=%cI "$SOURCE_PARENT")
export GIT_AUTHOR_DATE="$SOURCE_DATE"
export GIT_COMMITTER_DATE="$SOURCE_DATE"
j4=$(printf '%s\n' \
  'promote: join live ripr source with frozen W7 for 0.11.0' \
  '' \
  'Carry the live source-parent delta onto the reviewed J2 product tree,' \
  'preserve frozen W7, and retain a direct ordered two-parent join.' \
  | git -C "$source_repo" commit-tree "$j4_tree" -p "$SOURCE_PARENT" -p "$SWARM_PARENT")
test "$(git -C "$source_repo" show -s --format='%P' "$j4")" = "$SOURCE_PARENT $SWARM_PARENT"
test "$(git -C "$source_repo" rev-parse "$j4^{tree}")" = "$j4_tree"

phase=produce_fresh_exact_pair_preflight
mkdir -p "$preflight_out"
CARGO_TARGET_DIR="$RUNNER_TEMP/ripr-j4-w7-target" \
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
  --resolved-tree "$j4_tree" \
  --out "$preflight_out"
preflight="$preflight_out/source-promotion-preflight.json"
test -f "$preflight"
jq -e \
  --arg source "$SOURCE_PARENT" \
  --arg swarm "$SWARM_PARENT" \
  --arg tree "$j4_tree" \
  '.schema == "ripr.source_promotion_preflight.v1" and .source_parent == $source and .source_main == $source and .swarm_parent == $swarm and .dry_merge.reviewed_resolved_tree == $tree and .dry_merge.reviewed_resolved_tree_verified == true' \
  "$preflight" >/dev/null
cp "$preflight" "$packet/preflight.json"
cp "$preflight_out/source-promotion-preflight.md" "$packet/preflight.md"

phase=regenerate_complete_resolution_manifest
old_manifest="$RUNNER_TEMP/j2-resolution-manifest.json"
git -C "$source_repo" show "$OLD_CONTROL:docs/release/source-promotion/resolution-manifest.json" > "$old_manifest"
resolution="$RUNNER_TEMP/j4-resolution-manifest.json"
resolution_delta="$RUNNER_TEMP/j4-resolution-refresh.json"
python "$root/tools/release-control/j4/make_manifest.py" \
  "$preflight" "$old_manifest" "$resolution" "$resolution_delta" \
  "$source_repo" "$SOURCE_PARENT" "$SWARM_PARENT" "$j4" "$j4_tree" \
  "$packet/source-changed-paths.json"
cp "$resolution" "$packet/resolution-manifest.json"
cp "$resolution_delta" "$packet/resolution-refresh.json"

phase=trusted_exact_join_verification
mkdir -p "$verify_out"
CARGO_TARGET_DIR="$RUNNER_TEMP/ripr-j4-trusted-target" \
  cargo build --locked --manifest-path "$source_repo/Cargo.toml" -p xtask --bin xtask
trusted_verifier="$RUNNER_TEMP/ripr-j4-trusted-target/debug/xtask"
test -x "$trusted_verifier"
(
  cd "$source_repo"
  "$trusted_verifier" source-promotion verify \
    --preflight "$preflight" \
    --resolution-manifest "$resolution" \
    --join-head "$j4" \
    --source-main "$SOURCE_PARENT" \
    --out "$verify_out"
)
jq -e \
  --arg join "$j4" --arg tree "$j4_tree" --arg source "$SOURCE_PARENT" \
  '.schema == "ripr.source_promotion_verification.v2" and .status == "verified" and .join_head == $join and .source_main == $source and .tree == $tree and .checks.ordered_parents == true and .checks.reviewed_tree == true and .checks.release_version_identity == true' \
  "$verify_out/source-promotion-verification.json" >/dev/null
cp "$verify_out/source-promotion-verification.json" "$packet/verification.json"
cp "$verify_out/source-promotion-verification.md" "$packet/verification.md"

phase=focused_reviewed_tree_proof
git -C "$j4_dir" reset --hard "$j4"
(
  cd "$j4_dir"
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
contract_inputs="$RUNNER_TEMP/j4-contract-inputs.json"
jq -n \
  --arg source "$SOURCE_PARENT" \
  --arg join "$j4" \
  --arg preflight_sha "$preflight_sha" \
  --arg resolution_sha "$resolution_sha" \
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
  'release(control): bind exact 0.11.0 live-source J4 inputs' \
  '' \
  'Retain the fresh W7 preflight, complete reviewed resolution, and exact J4 identity.' \
  | git -C "$control_dir" commit-tree "$control_tree" -p "$SOURCE_PARENT")
test "$(git -C "$control_dir" merge-base "$control_commit" "$j4" || true)" != "$control_commit"
test "$(git -C "$control_dir" merge-base "$control_commit" "$j4" || true)" != "$j4"

phase=recheck_live_identity_before_publication
live_source=$(git -C "$source_repo" ls-remote origin refs/heads/main | awk '{print $1}')
test "$live_source" = "$SOURCE_PARENT" || {
  echo "source main moved during J4 construction: frozen=$SOURCE_PARENT live=$live_source" >&2
  exit 1
}
live_tag=$(git -C "$swarm_repo" ls-remote origin "$SWARM_REF" | awk '{print $1}')
test "$live_tag" = "$SWARM_PARENT"

phase=publish_exact_candidate_and_control_refs
git -C "$root" fetch "$source_repo" "$j4"
git -C "$root" fetch "$control_dir" "$control_commit"
test "$(git -C "$root" rev-parse "$j4^{commit}")" = "$j4"
test "$(git -C "$root" rev-parse "$control_commit^{commit}")" = "$control_commit"
git -C "$root" push --force origin "$j4:refs/heads/$J4_BRANCH"
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
  --data-urlencode "state=all" \
  --data-urlencode "head=$owner:$J4_BRANCH" \
  "$api/pulls")
existing_count=$(printf '%s' "$existing" | jq 'length')
test "$existing_count" -le 1
if test "$existing_count" -eq 1; then
  pr_number=$(printf '%s' "$existing" | jq -r '.[0].number')
  test "$(printf '%s' "$existing" | jq -r '.[0].merged_at // ""')" = ""
else
  placeholder='Exact J4 body is being bound to the assigned PR number.'
  create_payload=$(jq -n \
    --arg title 'promote: join live ripr source with frozen W7 for 0.11.0' \
    --arg head "$J4_BRANCH" \
    --arg base main \
    --arg body "$placeholder" \
    '{title:$title,head:$head,base:$base,body:$body,draft:true}')
  created=$(curl --fail-with-body --silent --show-error \
    --request POST \
    --header 'Accept: application/vnd.github+json' \
    --header "Authorization: Bearer $GH_TOKEN" \
    --header 'X-GitHub-Api-Version: 2022-11-28' \
    "$api/pulls" \
    --data "$create_payload")
  pr_number=$(printf '%s' "$created" | jq -r '.number')
fi

tree_paths=$(sed 's/^/- `/' "$packet/j2-to-j4.name-status" | sed 's/$/`/' | head -n 80)
cat > "$packet/pr-body.md" <<EOF
<!-- source-promotion: true -->
<!-- source-promotion-control: $control_commit -->

## Exact live-source J4

This is a fresh direct two-parent promotion object. It replaces stale J2/#1558 after the source-owned contract repair and all later source-main movement.

\`\`\`text
J4             = $j4
SOURCE_PARENT  = $SOURCE_PARENT
SWARM_PARENT   = $SWARM_PARENT
JOIN_TREE      = $j4_tree
CONTROL        = $control_commit
PREFLIGHT_SHA  = $preflight_sha
RESOLUTION_SHA = $resolution_sha
\`\`\`

## Reviewed movement

The previously reviewed J2 product tree remains the baseline. J4 carries the complete live source delta from \`$OLD_SOURCE_PARENT\` to \`$SOURCE_PARENT\` through exact source copies or conflict-free three-way integration; process policy is reconciled by literal owner and cannot widen a maximum implicitly.

The J2-to-J4 tree delta is retained in the construction packet. Changed entries:

$tree_paths

The fresh W7 producer preflight, complete \`kind:key\` resolution manifest, exact trusted-source verification, source-delta receipt, process-policy receipt, and patch are bound to this object. Frozen W7 is unchanged.

## Required merge transport

Use Create a merge commit.
Do not use Squash and merge.
Do not use Rebase and merge.

\`\`\`bash
gh pr merge $pr_number --repo EffortlessMetrics/ripr --merge --match-head-commit $j4
\`\`\`

Do not append a repair descendant to J4. Any exact-head defect or source-main movement rejects this object and requires a fresh direct join.

## Boundary

No version bump, changelog release entry, public tag, publication, signing, marketplace mutation, release-secret use, merge, or back-sync is included or authorized here.
EOF
pr_body=$(cat "$packet/pr-body.md")
update_payload=$(jq -n --arg body "$pr_body" '{body:$body,state:"closed"}')
updated=$(curl --fail-with-body --silent --show-error \
  --request PATCH \
  --header 'Accept: application/vnd.github+json' \
  --header "Authorization: Bearer $GH_TOKEN" \
  --header 'X-GitHub-Api-Version: 2022-11-28' \
  "$api/pulls/$pr_number" \
  --data "$update_payload")
test "$(printf '%s' "$updated" | jq -r '.state')" = closed
test "$(printf '%s' "$updated" | jq -r '.head.sha')" = "$j4"
test "$(printf '%s' "$updated" | jq -r '.base.sha')" = "$SOURCE_PARENT"
printf '%s\n' "$pr_number" > "$packet/pr-number.txt"

phase=write_success_receipt
jq -n \
  --arg schema ripr.source_promotion_j4_builder.v2 \
  --arg source "$SOURCE_PARENT" \
  --arg old_source "$OLD_SOURCE_PARENT" \
  --arg swarm "$SWARM_PARENT" \
  --arg swarm_main "$swarm_main" \
  --arg j2 "$J2" \
  --arg j2_tree "$J2_TREE" \
  --arg j4 "$j4" \
  --arg j4_tree "$j4_tree" \
  --arg control "$control_commit" \
  --arg preflight_sha "$preflight_sha" \
  --arg resolution_sha "$resolution_sha" \
  --arg j4_branch "$J4_BRANCH" \
  --arg control_branch "$CONTROL_BRANCH" \
  --arg pr_number "$pr_number" \
  '{schema:$schema,status:"verified_and_published",source_parent:$source,old_source_parent:$old_source,swarm_parent:$swarm,swarm_main_snapshot:$swarm_main,j2:{commit:$j2,tree:$j2_tree},j4:{commit:$j4,tree:$j4_tree,parents:[$source,$swarm],branch:$j4_branch},control:{commit:$control,branch:$control_branch,preflight_sha256:$preflight_sha,resolution_manifest_sha256:$resolution_sha},pull_request:{number:($pr_number|tonumber),state:"closed",reason:"GITHUB_TOKEN creation is followed by connector reopen so hosted pull_request workflows run"},proof:["live source frozen and rechecked before publication","unchanged protected W7 tag","fresh W7 producer preflight","complete refreshed kind:key manifest","trusted source-parent verifier status verified","focused J4 Rust and repository policy checks"],non_claims:["no merge","no version change","no public tag","no publication","no signing","no marketplace mutation","no release-secret use","no back-sync"]}' \
  > "$packet/receipt.json"
printf 'SOURCE_PARENT=%s\nSWARM_PARENT=%s\nJ4=%s\nJ4_TREE=%s\nCONTROL_COMMIT=%s\nJ4_BRANCH=%s\nCONTROL_BRANCH=%s\nPR_NUMBER=%s\nPREFLIGHT_SHA256=%s\nRESOLUTION_SHA256=%s\n' \
  "$SOURCE_PARENT" "$SWARM_PARENT" "$j4" "$j4_tree" "$control_commit" "$J4_BRANCH" "$CONTROL_BRANCH" "$pr_number" "$preflight_sha" "$resolution_sha" \
  > "$packet/identities.env"
status=success
phase=complete
