#!/usr/bin/env bash
set -euo pipefail
# Inputs arrive through the event environment; never evaluate pull-request text.
fail() { printf '%s\n' "source-promotion composition rejected: $1" >&2; exit 1; }
marker='<!-- source-promotion: true -->'
control_prefix='<!-- source-promotion-control:'
# Routing identity is mandatory, even for an ordinary non-promotion PR.
[[ ${PR_BODY+x} && ${HEAD_REF+x} ]] || fail 'event routing inputs missing'
if [[ "${REQUIRE_TRUSTED_EVENT:-0}" == "1" ]]; then
  [[ "${EVENT_NAME:-}" == "pull_request_target" ]] || fail 'trusted pull_request_target event required'
  [[ "${WORKFLOW_SOURCE_SHA:-}" =~ ^[0-9a-f]{40}$ ]] || fail 'trusted workflow source SHA missing or invalid'
fi
[[ -n "$HEAD_REF" && "$HEAD_REF" != -* ]] && git check-ref-format "refs/heads/$HEAD_REF" || fail 'invalid head ref'
# GitHub web-edited bodies may use CRLF; marker cardinality stays exact.
PR_BODY=${PR_BODY//$'\r'/}
PREV_PR_BODY=${PREV_PR_BODY:-}
PREV_PR_BODY=${PREV_PR_BODY//$'\r'/}
if [[ "$HEAD_REF" != promote/0.11.0-* ]] && [[ "$PR_BODY" != *"$marker"* ]] && [[ "$PR_BODY" != *"$control_prefix"* ]]; then
  if [[ "$PREV_PR_BODY" == *"$marker"* ]] || [[ "$PREV_PR_BODY" == *"$control_prefix"* ]]; then
    fail 'promotion marker removed by body edit'
  fi
  if [[ -n "${COMPOSITION_RECEIPT_OUT:-}" ]]; then
    jq -n \
      --arg schema 'ripr.source_promotion_composition_gate.v1' \
      --arg status 'ordinary_pr_not_promotion' \
      --arg event_name "${EVENT_NAME:-unknown}" \
      --arg workflow_source_sha "${WORKFLOW_SOURCE_SHA:-unknown}" \
      --arg workflow_run_id "${WORKFLOW_RUN_ID:-unknown}" \
      --arg workflow_ref "${WORKFLOW_REF:-unknown}" \
      --arg token_permissions "${TOKEN_PERMISSIONS:-contents:read}" \
      --arg head_ref "$HEAD_REF" \
      --arg protected_head_sha "${HEAD_SHA:-}" \
      --arg protected_preview_sha "${MERGE_SHA:-}" \
      '{
        schema: $schema,
        status: $status,
        event_name: $event_name,
        workflow_source_sha: $workflow_source_sha,
        workflow_run_id: $workflow_run_id,
        workflow_ref: $workflow_ref,
        token_permissions: $token_permissions,
        secrets_used: false,
        head_ref: $head_ref,
        protected_head_sha: $protected_head_sha,
        protected_preview_sha: $protected_preview_sha,
        check_run_source_identity: "declared_default_branch_workflow_check_run_unobserved",
        ruleset_readback: "external_owner_gated_unknown"
      }' > "$COMPOSITION_RECEIPT_OUT"
  fi
  echo 'Ordinary pull request: no source-promotion composition requested.'
  exit 0
fi
command -v python3 >/dev/null 2>&1 || fail 'python3 is required for promotion sidecar validation'
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
control_file=$(mktemp)
preflight_file=$(mktemp)
trap 'rm -f "$control_file" "$preflight_file"' EXIT
git show "$control:$control_path" > "$control_file"
reject_duplicate_keys() {
  python3 -c '
import json, sys
def unique(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError("duplicate JSON key")
        result[key] = value
    return result
try:
    json.load(sys.stdin, object_pairs_hook=unique)
except (ValueError, UnicodeError, RecursionError):
    sys.exit(1)
' < "$1"
}
# jq is stream-oriented: a valid object followed by an incomplete object can
# otherwise satisfy separate field checks using values from different objects.
jq -e -s 'length == 1 and (.[0] | type == "object")' "$control_file" >/dev/null || fail 'control must contain exactly one JSON object'
reject_duplicate_keys "$control_file" || fail 'control must contain exactly one JSON object without duplicate keys'
inputs=$(cat "$control_file")
git show "$control:$preflight_path" > "$preflight_file"
jq -e -s 'length == 1 and (.[0] | type == "object")' "$preflight_file" >/dev/null || fail 'preflight must contain exactly one JSON object'
reject_duplicate_keys "$preflight_file" || fail 'preflight must contain exactly one JSON object without duplicate keys'
jq -e --arg p "$preflight_path" --arg base "$BASE_SHA" '.schema == "ripr.source_promotion_ci_inputs.v2" and .preflight == $p and .source_main == $base' <<< "$inputs" >/dev/null || fail 'control identities do not bind event base and merge'
expected=$(jq -er '.preflight_sha256 | select(type == "string" and test("^[0-9a-f]{64}$"))' <<< "$inputs") || fail 'invalid preflight digest'
[[ $(sha256sum "$preflight_file" | cut -d ' ' -f 1) == "$expected" ]] || fail 'preflight digest mismatch'
mode=$(jq -er '.mode | select(type == "string")' "$preflight_file") || fail 'invalid or missing preflight mode'
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
staged_packet_verdict='not_configured'
if [[ "${REQUIRE_STAGED_PACKET:-0}" == "1" ]] || [[ -n "${STAGED_VERIFIER_BIN:-}" ]]; then
  [[ -n "${STAGED_VERIFIER_BIN:-}" && -x "${STAGED_VERIFIER_BIN:-}" ]] || fail 'staged packet verifier binary missing or not executable'
  staged_packet_path=docs/release/source-promotion/staged-packet.json
  qual_receipt_path=docs/release/source-promotion/qualification-receipt.json
  for spath in "$staged_packet_path" "$qual_receipt_path"; do
    sentry=$(git ls-tree "$control" -- "$spath")
    [[ "$sentry" == $'100644 blob '*$'\t'"$spath" ]] || fail 'staged admission sidecar is not a regular tracked file'
  done
  staged_packet_file=$(mktemp)
  qual_receipt_file=$(mktemp)
  trap 'rm -f "$control_file" "$preflight_file" "$staged_packet_file" "$qual_receipt_file"' EXIT
  git show "$control:$staged_packet_path" > "$staged_packet_file"
  git show "$control:$qual_receipt_path" > "$qual_receipt_file"
  expected_packet_sha=$(jq -er '.staged_packet_sha256 | select(type == "string" and test("^[0-9a-f]{64}$"))' <<< "$inputs") || fail 'invalid staged_packet_sha256 in control inputs'
  expected_receipt_sha=$(jq -er '.qualification_receipt_sha256 | select(type == "string" and test("^[0-9a-f]{64}$"))' <<< "$inputs") || fail 'invalid qualification_receipt_sha256 in control inputs'
  [[ $(sha256sum "$staged_packet_file" | cut -d ' ' -f 1) == "$expected_packet_sha" ]] || fail 'staged packet digest mismatch'
  [[ $(sha256sum "$qual_receipt_file" | cut -d ' ' -f 1) == "$expected_receipt_sha" ]] || fail 'qualification receipt digest mismatch'
  swarm_s="$HEAD_SHA"
  if [[ "$mode" == "two_parent_join" ]]; then
    swarm_s="$swarm"
  fi
  "$STAGED_VERIFIER_BIN" source-promotion verify-staged-packet \
    --packet "$staged_packet_file" \
    --qualification-receipt "$qual_receipt_file" \
    --repo . \
    --expected-p "$BASE_SHA" \
    --expected-s "$swarm_s" \
    --expected-k "$MERGE_SHA" \
    --expected-t "$tree" >/dev/null || fail 'staged packet independent verification rejected'
  staged_packet_verdict='admitted'
fi
if [[ -n "${COMPOSITION_RECEIPT_OUT:-}" ]]; then
  inputs_sha256=$(sha256sum "$control_file" | cut -d ' ' -f 1)
  swarm_s="$HEAD_SHA"
  if [[ "$mode" == "two_parent_join" ]]; then
    swarm_s="$swarm"
  fi
  jq -n \
    --arg schema 'ripr.source_promotion_composition_gate.v1' \
    --arg status 'verified' \
    --arg mode "$mode" \
    --arg staged_packet_verdict "$staged_packet_verdict" \
    --arg event_name "${EVENT_NAME:-unknown}" \
    --arg workflow_source_sha "${WORKFLOW_SOURCE_SHA:-unknown}" \
    --arg workflow_run_id "${WORKFLOW_RUN_ID:-unknown}" \
    --arg workflow_ref "${WORKFLOW_REF:-unknown}" \
    --arg token_permissions "${TOKEN_PERMISSIONS:-contents:read}" \
    --arg p "$BASE_SHA" \
    --arg s "$swarm_s" \
    --arg k "$MERGE_SHA" \
    --arg t "$tree" \
    --arg protected_head_sha "$HEAD_SHA" \
    --arg protected_preview_sha "$MERGE_SHA" \
    --arg control_commit "$control" \
    --arg contract_inputs_sha256 "$inputs_sha256" \
    --arg preflight_sha256 "$expected" \
    '{
      schema: $schema,
      status: $status,
      mode: $mode,
      staged_packet_verdict: $staged_packet_verdict,
      event_name: $event_name,
      workflow_source_sha: $workflow_source_sha,
      workflow_run_id: $workflow_run_id,
      workflow_ref: $workflow_ref,
      token_permissions: $token_permissions,
      secrets_used: false,
      protected_head_sha: $protected_head_sha,
      protected_preview_sha: $protected_preview_sha,
      identities: {
        P: $p,
        S: $s,
        K: $k,
        T: $t,
        control_commit: $control_commit,
        contract_inputs_sha256: $contract_inputs_sha256,
        preflight_sha256: $preflight_sha256
      },
      check_run_source_identity: "declared_default_branch_workflow_check_run_unobserved",
      ruleset_readback: "external_owner_gated_unknown"
    }' > "$COMPOSITION_RECEIPT_OUT"
fi
echo 'Source-promotion composition matches the control packet.'
