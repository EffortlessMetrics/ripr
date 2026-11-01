#!/usr/bin/env bash
set -euo pipefail
# Inputs arrive through the event environment; never evaluate pull-request text.
fail() { printf '%s\n' "source-promotion composition rejected: $1" >&2; exit 1; }
marker='<!-- source-promotion: true -->'
control_prefix='<!-- source-promotion-control:'
if [[ "$HEAD_REF" != promote/0.11.0-* ]] && [[ "$PR_BODY" != *"$marker"* ]] && [[ "$PR_BODY" != *"$control_prefix"* ]]; then
  echo 'Ordinary pull request: no source-promotion composition requested.'
  exit 0
fi
[[ "$PR_BODY" == *"$marker"* ]] || fail 'promotion marker missing'
mapfile -t markers < <(printf '%s\n' "$PR_BODY" | grep -E '^<!-- source-promotion-control: [0-9a-f]{40} -->$' || true)
[[ ${#markers[@]} -eq 1 ]] || fail 'exactly one control marker required'
[[ $(printf '%s\n' "$PR_BODY" | grep -Fc "$control_prefix" || true) -eq 1 ]] || fail 'ambiguous control marker'
control=${markers[0]#'<!-- source-promotion-control: '}
control=${control%' -->'}
for sha in "$BASE_SHA" "$HEAD_SHA" "$MERGE_SHA"; do [[ "$sha" =~ ^[0-9a-f]{40}$ ]] || fail 'invalid event SHA'; done
[[ "$PR_NUMBER" =~ ^[0-9]+$ ]] || fail 'invalid pull request number'
git fetch --no-tags origin "$control" >/dev/null
git fetch --no-tags origin "refs/pull/$PR_NUMBER/merge" >/dev/null
preview=$(git --no-replace-objects rev-parse FETCH_HEAD^{commit})
[[ "$preview" == "$MERGE_SHA" ]] || fail 'event is not the live test merge commit'
control_path=docs/release/source-promotion/contract-inputs.json
preflight_path=docs/release/source-promotion/preflight.json
for path in "$control_path" "$preflight_path"; do
  entry=$(git ls-tree "$control" -- "$path")
  [[ "$entry" == $'100644 blob '*$'\t'"$path" ]] || fail 'control sidecar is not a regular tracked file'
done
inputs=$(git show "$control:$control_path")
preflight_file=$(mktemp)
trap 'rm -f "$preflight_file"' EXIT
git show "$control:$preflight_path" > "$preflight_file"
jq -e --arg p "$preflight_path" --arg base "$BASE_SHA" '.schema == "ripr.source_promotion_ci_inputs.v2" and .preflight == $p and .source_main == $base' <<< "$inputs" >/dev/null || fail 'control identities do not bind event base and merge'
expected=$(jq -er '.preflight_sha256 | select(type == "string" and test("^[0-9a-f]{64}$"))' <<< "$inputs") || fail 'invalid preflight digest'
[[ $(sha256sum "$preflight_file" | cut -d ' ' -f 1) == "$expected" ]] || fail 'preflight digest mismatch'
mode=$(jq -er '.mode' "$preflight_file")
case "$mode" in
  fast_forward)
    jq -e --arg merge "$MERGE_SHA" '.join_head == $merge' <<< "$inputs" >/dev/null || fail 'control preview mismatch'
    jq -e --arg base "$BASE_SHA" --arg head "$HEAD_SHA" '.source_parent == $base and .swarm_parent == $head' "$preflight_file" >/dev/null || fail 'staged preflight does not bind event parents'
    tree=$(jq -er '.dry_merge.preview_tree | select(type == "string" and test("^[0-9a-f]{40}$"))' "$preflight_file") || fail 'invalid qualified tree'
    git --no-replace-objects merge-base --is-ancestor "$BASE_SHA" "$HEAD_SHA" || fail 'P is not an ancestor of S'
    ;;
  two_parent_join)
    jq -e --arg head "$HEAD_SHA" '.join_head == $head' <<< "$inputs" >/dev/null || fail 'control join mismatch'
    jq -e --arg base "$BASE_SHA" '.source_parent == $base' "$preflight_file" >/dev/null || fail 'join preflight does not bind source parent'
    swarm=$(jq -er '.swarm_parent | select(type == "string" and test("^[0-9a-f]{40}$"))' "$preflight_file") || fail 'invalid swarm parent'
    tree=$(jq -er '.dry_merge.reviewed_resolved_tree | select(type == "string" and test("^[0-9a-f]{40}$"))' "$preflight_file") || fail 'invalid reviewed tree'
    [[ $(git --no-replace-objects rev-list --parents -n 1 "$HEAD_SHA") == "$HEAD_SHA $BASE_SHA $swarm" ]] || fail 'join parents differ from [P,S]'
    ;;
  *) fail 'unsupported promotion mode' ;;
esac
[[ $(git --no-replace-objects rev-list --parents -n 1 "$MERGE_SHA") == "$MERGE_SHA $BASE_SHA $HEAD_SHA" ]] || fail 'test merge parents differ from [P,head]'
[[ $(git --no-replace-objects rev-parse "$MERGE_SHA^{tree}") == "$tree" ]] || fail 'test merge tree differs from T'
[[ $(git --no-replace-objects rev-parse "$HEAD_SHA^{tree}") == "$tree" ]] || fail 'head tree differs from T'
echo 'Source-promotion composition matches the control packet.'
