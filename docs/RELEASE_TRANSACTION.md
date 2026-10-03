# Live-history release transaction runbook

Status: active operator runbook for the `0.11.0` live-head transaction and
future releases using the same two-repository history model.

This document is the canonical lifecycle from the release boundary through
reopened swarm development. It sequences existing verifier contracts; it does
not replace their semantics or make publication implicit.

## Authority and command labels

`EffortlessMetrics/ripr-swarm` is development authority and preserves every
commit reachable from the selected swarm main, including the reviewable history
of ordinary squash-merged feature/fix PRs.
`EffortlessMetrics/ripr` is source, release, and distribution authority.
Public tags, GitHub Releases, crates.io, VS Code Marketplace, and Open VSX are
source-side surfaces. Never publish or create a public-release tag from swarm.

Every command is marked:

- **[READ-ONLY]** — reads a checkout, receipt, GitHub state, or public channel.
- **[LOCAL-MUTATING]** — changes only a named local checkout, worktree, index,
  branch, or receipt file.
- **[EXTERNAL-PUBLISHING]** — changes a remote ref, policy, tag, release,
  registry, marketplace, or other external state. Public artifact, tag,
  release, and store operations require explicit channel authorization in
  source issue [#1470](https://github.com/EffortlessMetrics/ripr/issues/1470);
  control-plane ref and policy operations require owner approval and a receipt.

Examples use POSIX shell syntax. On Windows, translate syntax but preserve
exact SHAs and expected-head checks; never substitute floating `main`.
Every executable Bash block below begins in the same fail-fast transaction
shell: run `set -euo pipefail` before the block (and again when copying a block
into a new shell). A block copied without that session preamble is not valid
proof; an unset required variable or failed pipeline must stop the transaction.

## Packet, authority reset, and stop points

Create `target/ripr/release-transaction/<VERSION>/` outside tracked source.
The packet must bind `SOURCE_PARENT`, `SWARM_PARENT`, protected
`refs/tags/ripr-release-<VERSION>-<SWARM_PARENT>`, local verifier
`refs/ripr/release-<VERSION>-<SWARM_PARENT>`, `MERGE_BASE`, reviewed `JOIN_TREE`,
`J`, `SWARM_BEFORE`, `SOURCE_RELEASE_HEAD`, reviewed `BACK_SYNC_TREE`, and `K`.
Retain all-reachable and first-parent counts/digests, open-PR dispositions,
toolchain/version, routed-CI URL and `headSha`, conflict/resolution manifest,
artifact receipts, authorization, channel results, policy-before/exception/
after snapshots, and cleanup. JSON and Markdown projections must share inputs.
Run the command blocks in one operator shell, or export the variables before
continuing. The paths below are packet-local and are not repository authority.

```bash
set -euo pipefail
# [LOCAL-MUTATING] repo=operator checkout; initialize transaction packet paths
VERSION=0.11.0
PACKET_ROOT="$(pwd)/target/ripr/release-transaction/${VERSION}"
mkdir -p "$PACKET_ROOT"
RELEASE_NOTES="$PACKET_ROOT/release-notes.md"
PUBLICATION_RECEIPT="$PACKET_ROOT/publication-receipt.json"
POLICY_BEFORE="$PACKET_ROOT/policy-before.json"
POLICY_EXCEPTION="$PACKET_ROOT/policy-temporary-exception.json"
POLICY_EXCEPTION_REQUEST="$PACKET_ROOT/policy-temporary-exception-request.json"
POLICY_RESTORE_REQUEST="$PACKET_ROOT/policy-restore-request.json"
POLICY_AFTER="$PACKET_ROOT/policy-after.json"
POLICY_APPROVAL="$PACKET_ROOT/policy-owner-approval.json"

# [READ-ONLY] repo=operator shell; require caller-selected fresh roots
SOURCE_ROOT="${SOURCE_ROOT:?set to the absolute path of a fresh ripr checkout}"
SWARM_ROOT="${SWARM_ROOT:?set to the absolute path of a fresh ripr-swarm checkout}"
test "$(git -C "$SOURCE_ROOT" rev-parse --is-inside-work-tree)" = true
test "$(git -C "$SWARM_ROOT" rev-parse --is-inside-work-tree)" = true
RUN_WAIT_SECONDS=1800
wait_for_run_success() {
  run_id="$1"
  deadline=$(( $(date +%s) + RUN_WAIT_SECONDS ))
  while true; do
    run_state="$(gh run view "$run_id" --repo EffortlessMetrics/ripr --json status,conclusion --jq '[.status, (.conclusion // "")] | @tsv')"
    status="${run_state%%$'\t'*}"
    conclusion="${run_state#*$'\t'}"
    if test "$status" = completed; then
      test "$conclusion" = success
      return 0
    fi
    test "$(date +%s)" -lt "$deadline" || { echo "workflow $run_id did not complete within $RUN_WAIT_SECONDS seconds" >&2; return 1; }
    sleep 15
  done
}
bind_new_dispatch_run() {
  before_file="$1"
  after_file="$2"
  sha="$3"
  ref="$4"
  since="$5"
  endpoint="$6"
  deadline=$(( $(date +%s) + 300 ))
  while test "$(date +%s)" -lt "$deadline"; do
    gh api --paginate --slurp "$endpoint" | jq '[.[] | .workflow_runs[]]' > "$after_file"
    jq -n --slurpfile before "$before_file" --slurpfile after "$after_file" \
      --arg sha "$sha" --arg ref "$ref" --arg since "$since" '
    ($before[0] | map(.id)) as $old_ids |
    [$after[0][] | .id as $run_id | select(.head_sha == $sha and .head_branch == $ref and
      .event == "workflow_dispatch" and .created_at >= $since and
      (($old_ids | index($run_id)) | not))] | unique_by(.id)' > "${after_file%.json}-new.json"
    count="$(jq 'length' "${after_file%.json}-new.json")"
    test "$count" -le 1 || return 1
    test "$count" = 1 && { jq -r '.[0].id' "${after_file%.json}-new.json"; return 0; }
    sleep 5
  done
  return 1
}
```

Historical candidate-only `C -> T`, hard-cut, replacement-freeze, and
candidate-ref receipts remain audit evidence but are superseded. They are not
the active `0.11.0` authority, denominator, pin, source parent, or permission.

Retained artifacts under `docs/release-candidates/` retain their #3842
lifecycle metadata and `cargo xtask check-release-targets` policy validation.
Changing a template's schema updates its own raw digest and human projection;
it does not activate a pin or rewrite historical receipt bytes. The broad
#3924 consumer-integration work remains deferred for 0.11.x. Direct #1609
schema-1.1 admission below binds the independently reviewed manifest and exact
owner packets without treating milestone policy as live-head selection.
Historical registry mode stays separately available and never becomes a
fallback from refused direct inputs.

Stop at once when a repository identity, owner, version, exact input, receipt,
expected head, policy state, or publication authorization is ambiguous. Green
CI, a ship packet, J, or K is evidence only; none authorizes publication.

## 1. Reset truth and pre-cut queue observations

Use fresh checkouts. Ordinary source-repository development PRs are drift:
redirect them to swarm or classify them as release/security work before the
source hold. Record each remaining PR as included, deferred, superseded, or
not release-relevant; do not close another owner's work to make the queue look
empty.

```bash
set -euo pipefail
# [LOCAL-MUTATING] repo=both fresh operator checkouts; fetch and reconcile live state
git -C "$SWARM_ROOT" fetch origin --prune --tags
git -C "$SOURCE_ROOT" fetch origin --prune --tags
git -C "$SWARM_ROOT" status --short --branch
git -C "$SOURCE_ROOT" status --short --branch
gh pr list --repo EffortlessMetrics/ripr-swarm --state open --limit 100
gh pr list --repo EffortlessMetrics/ripr --state open --limit 100
SOURCE_ORIGIN="$(git -C "$SOURCE_ROOT" remote get-url origin)"
SWARM_ORIGIN="$(git -C "$SWARM_ROOT" remote get-url origin)"
case "$SOURCE_ORIGIN" in
  git@github.com:EffortlessMetrics/ripr.git|https://github.com/EffortlessMetrics/ripr|https://github.com/EffortlessMetrics/ripr.git) ;;
  *) echo "unexpected source origin: $SOURCE_ORIGIN" >&2; exit 1 ;;
esac
case "$SWARM_ORIGIN" in
  git@github.com:EffortlessMetrics/ripr-swarm.git|https://github.com/EffortlessMetrics/ripr-swarm|https://github.com/EffortlessMetrics/ripr-swarm.git) ;;
  *) echo "unexpected swarm origin: $SWARM_ORIGIN" >&2; exit 1 ;;
esac
```

Admit only bounded final swarm cleanup: a release-blocking candidate failure,
source-preflight survivor, or required policy/evidence contract. After pinning,
later swarm-main merges are outside this release; branches may continue, but
their merges belong to the next release.

```bash
set -euo pipefail
# [LOCAL-MUTATING] repo=packet; capture exact heads and pin files
test -z "$(git -C "$SOURCE_ROOT" status --short)"
test -z "$(git -C "$SWARM_ROOT" status --short)"
SOURCE_OBSERVED_HEAD="$(git -C "$SOURCE_ROOT" rev-parse refs/remotes/origin/main)"
SWARM_PARENT="$(git -C "$SWARM_ROOT" rev-parse refs/remotes/origin/main)"
test "$(git -C "$SOURCE_ROOT" rev-parse "$SOURCE_OBSERVED_HEAD^{commit}")" = "$SOURCE_OBSERVED_HEAD"
test "$(git -C "$SWARM_ROOT" rev-parse "$SWARM_PARENT^{commit}")" = "$SWARM_PARENT"
printf '%s\n' "$SOURCE_OBSERVED_HEAD" > "$PACKET_ROOT/source-observed-head"
printf '%s\n' "$SWARM_PARENT" > "$PACKET_ROOT/SWARM_PARENT"
```

```bash
set -euo pipefail
# [READ-ONLY] repo=packet+source+swarm; load pins and reject drift
SOURCE_OBSERVED_HEAD="$(tr -d '\r\n' < "$PACKET_ROOT/source-observed-head")"
SWARM_PARENT="$(tr -d '\r\n' < "$PACKET_ROOT/SWARM_PARENT")"
# SOURCE_OBSERVED_HEAD is observation only; source #1769 binds SOURCE_PARENT later.
test "$(git -C "$SWARM_ROOT" rev-parse refs/remotes/origin/main)" = "$SWARM_PARENT"
```

The queue snapshot is produced by the two `gh pr list` calls above and bound
to the exact observed heads. Source observation is not a held SOURCE_PARENT.
Keep the producer output, not a hand-edited table:

```bash
set -euo pipefail
# [LOCAL-MUTATING] repo=packet+swarm; copy and validate the checked-in selection template before W
LIVE_HEAD_TEMPLATE="docs/release-candidates/0.11.0-live-head-selection.json"
cp "$LIVE_HEAD_TEMPLATE" "$PACKET_ROOT/live-head-selection-template.json"
jq -e '.schema_version == "1.1" and .status == "active_selection_template" and .candidate == null and .source_parent == null' "$LIVE_HEAD_TEMPLATE" >/dev/null
TEMPLATE_SHA256="$(sha256sum "$LIVE_HEAD_TEMPLATE" | awk '{print $1}')"
cargo xtask check-release-targets
jq -e --arg path "$LIVE_HEAD_TEMPLATE" --arg sha "$TEMPLATE_SHA256" '[.artifacts[] | select(.path == $path and .sha256 == $sha and .state == "active_selection_template" and .projection_of == null)] | length == 1' docs/release-candidates/index.json >/dev/null
```

```bash
set -euo pipefail
# [LOCAL-MUTATING] repo=packet; retain observations, not candidate admission
gh pr list --repo EffortlessMetrics/ripr-swarm --state open --limit 100 --json number,title,headRefName,headRefOid,baseRefName,isDraft,updatedAt > "$PACKET_ROOT/swarm-open-prs.json"
gh pr list --repo EffortlessMetrics/ripr --state open --limit 100 --json number,title,headRefName,headRefOid,baseRefName,isDraft,updatedAt > "$PACKET_ROOT/source-open-prs.json"
jq -n --arg observed_at "$(date -u +%Y-%m-%dT%H:%M:%SZ)" --arg source_observed_head "$SOURCE_OBSERVED_HEAD" --arg swarm_parent "$SWARM_PARENT" --slurpfile swarm "$PACKET_ROOT/swarm-open-prs.json" --slurpfile source "$PACKET_ROOT/source-open-prs.json" '{schema_version: 1, kind: "precut_queue_observation", observed_at_utc: $observed_at, source_observed_head: $source_observed_head, swarm_parent: $swarm_parent, queues: {swarm: $swarm[0], source: $source[0]}, claim: "observation only; not a candidate manifest or source hold"}' > "$PACKET_ROOT/precut-queue-observation.json"
```

Before W, swarm movement extends the denominator and requires a new current
#2766/#2768/#3807 handoff. Source observations may change during this stage;
source #1769 does not select or hold SOURCE_PARENT until swarm qualification.
After that later source hold, movement invalidates the corresponding source
transaction. Never update only a SHA in an existing immutable receipt.

## 2. Guarded swarm pin

`W` is the first successful push of the protected candidate tag below. The
checked-in schema-1.1 template is copied, not activated in place. Before W,
#2766 selected claims, #2768 denominator and #3807 audit must have current
accepted owner packets. The final manifest binds their raw bytes and actual
candidate/package identities. The template and a producer-emitted sidecar do
not establish those approvals. After W, unrelated main movement is outside the
candidate, never a silent membership update.

```bash
set -euo pipefail
# [LOCAL-MUTATING] repo=swarm root; create only the local transaction ref
VERSION=0.11.0
PIN_TAG="ripr-release-${VERSION}-${SWARM_PARENT}"
SWARM_REF="refs/tags/$PIN_TAG"
VERIFIER_REF="refs/ripr/release-${VERSION}-${SWARM_PARENT}"
git -C "$SWARM_ROOT" update-ref "$VERIFIER_REF" "$SWARM_PARENT"
test "$(git -C "$SWARM_ROOT" rev-parse "$VERIFIER_REF^{commit}")" = "$SWARM_PARENT"
```

```bash
set -euo pipefail
# [READ-ONLY] repo=swarm remote; require protected candidate-tag guarantee
PIN_RULESET_ID="$(gh api repos/EffortlessMetrics/ripr-swarm/rulesets --paginate --jq '.[] | select(.name == "release-transaction-pins" and .target == "tag" and .enforcement == "active") | .id')"
test "$(printf '%s\n' "$PIN_RULESET_ID" | awk 'NF {count++} END {print count + 0}')" -eq 1
PIN_RULESET="$PACKET_ROOT/pin-ruleset.json"
gh api "repos/EffortlessMetrics/ripr-swarm/rulesets/${PIN_RULESET_ID}" > "$PIN_RULESET"
# GitHub's REST ruleset examples use fully qualified refs (for example,
# `refs/heads/main`), and its rule-suite API documents `refs/tags/` as the
# tag-ref prefix. See https://docs.github.com/en/rest/orgs/rules and
# https://docs.github.com/en/rest/repos/rule-suites. Keep the candidate tag
# name below unqualified (`ripr-release-...`), but require this repository's
# fail-closed policy contract to cover only the tag namespace. The checked-in
# fixture/test proves that local contract; it does not prove GitHub server
# acceptance or rejection. Verify live settings through the read-only receipt.
jq -e --arg tag "refs/tags/ripr-release-*" '(.target == "tag" and .enforcement == "active") and (.conditions.ref_name.include == [$tag]) and (any(.rules[]?; .type == "update")) and (any(.rules[]?; .type == "deletion")) and (.conditions.ref_name.exclude == []) and (.bypass_actors == [])' "$PIN_RULESET" >/dev/null
```

```bash
set -euo pipefail
assert_live_pin_guard() {
  test "$(git -C "$SWARM_ROOT" ls-remote origin "refs/tags/$PIN_TAG" | awk '{print $1}')" = "$SWARM_PARENT"
  gh api "repos/EffortlessMetrics/ripr-swarm/rulesets/${PIN_RULESET_ID}" > "$PACKET_ROOT/pin-ruleset-live.json"
  cmp "$PIN_RULESET" "$PACKET_ROOT/pin-ruleset-live.json"
  test "$(sha256sum "$PACKET_ROOT/pin-ruleset-live.json" | awk '{print $1}')" = "$(sha256sum "$PIN_RULESET" | awk '{print $1}')"
  test "$(sha256sum "$PACKET_ROOT/pin-remote.sha" | awk '{print $1}')" = "$PIN_DIGEST"
}
```

The exact ruleset receipt is a prerequisite, not a suggestion. If the named
active tag ruleset, its exact `refs/tags/ripr-release-*` pattern, or both
update/deletion rules are absent, stop before W; do not substitute an
unprotected custom ref.

```bash
set -euo pipefail
# [EXTERNAL-PUBLISHING] repo=swarm remote; publish protected candidate tag only
PIN_BEFORE="$(git -C "$SWARM_ROOT" ls-remote origin "refs/tags/$PIN_TAG" | awk '{print $1}')"
test -z "$PIN_BEFORE"
test -z "$(git -C "$SWARM_ROOT" show-ref --tags --verify "refs/tags/$PIN_TAG" 2>/dev/null || true)"
git -C "$SWARM_ROOT" tag "$PIN_TAG" "$SWARM_PARENT"
git -C "$SWARM_ROOT" push origin "refs/tags/$PIN_TAG:refs/tags/$PIN_TAG"
PIN_AFTER="$(git -C "$SWARM_ROOT" ls-remote origin "refs/tags/$PIN_TAG" | awk '{print $1}')"
test "$PIN_AFTER" = "$SWARM_PARENT"
printf '%s\n' "$PIN_AFTER" > "$PACKET_ROOT/pin-remote.sha"
PIN_DIGEST="$(sha256sum "$PACKET_ROOT/pin-remote.sha" | awk '{print $1}')"
```

```bash
set -euo pipefail
# [LOCAL-MUTATING] repo=packet; assemble the one schema-1.1 manifest after W
# These are existing accepted owner packets, not files synthesized to make this step pass.
SELECTED_CLAIMS_PACKET="${SELECTED_CLAIMS_PACKET:?set controller-relative accepted #2766 packet}"
DENOMINATOR_PACKET="${DENOMINATOR_PACKET:?set controller-relative accepted #2768 packet}"
AUDIT_PACKET="${AUDIT_PACKET:?set controller-relative accepted #3807 packet}"
# #2768 supplies the reviewed schema-1.1 projection: topo-order/reverse and
# first-parent/reverse full-SHA+LF digests from SPEC-0144, not historical JSON hashes.
DENOMINATOR_RANGE_JSON="${DENOMINATOR_RANGE_JSON:?set accepted #2768 range projection}"
# References to the selected candidate's exact proof-input bytes, using SPEC-0144 Evidence fields.
PROOF_INPUTS_JSON="${PROOF_INPUTS_JSON:?set accepted proof-input reference list}"
# Supplied by the trusted controller from exact native owner decisions, not generated here.
OWNER_ACCEPTANCE_JSON="${OWNER_ACCEPTANCE_JSON:?set reviewed selection/denominator/audit acceptance envelopes}"
CONTROLLER_ROOT="${CONTROLLER_ROOT:?set absolute independent controller root}"
CANDIDATE_TREE="$(git -C "$SWARM_ROOT" rev-parse "$SWARM_PARENT^{tree}")"
test "$(git -C "$SWARM_ROOT" rev-parse HEAD)" = "$SWARM_PARENT"
test -z "$(git -C "$SWARM_ROOT" status --porcelain=v1 --untracked-files=all)"
# Copy readbacks into this controller's packet; paths remain relative and bytes are retained.
PIN_READBACK_PATH="${PIN_READBACK_PATH:?set controller-relative pin-remote.sha}"
PIN_RULESET_PATH="${PIN_RULESET_PATH:?set controller-relative pin-ruleset.json}"
jq --arg sha "$SWARM_PARENT" --arg tree "$CANDIDATE_TREE" --arg ref "$SWARM_REF" \
  --arg workspace_sha "$(git -C "$SWARM_ROOT" show "$SWARM_PARENT:Cargo.toml" | sha256sum | awk '{print $1}')" \
  --arg package_sha "$(git -C "$SWARM_ROOT" show "$SWARM_PARENT:crates/ripr/Cargo.toml" | sha256sum | awk '{print $1}')" \
  --arg lock_sha "$(git -C "$SWARM_ROOT" show "$SWARM_PARENT:Cargo.lock" | sha256sum | awk '{print $1}')" \
  --arg selection "$SELECTED_CLAIMS_PACKET" --arg selection_sha "$(sha256sum "$CONTROLLER_ROOT/$SELECTED_CLAIMS_PACKET" | awk '{print $1}')" \
  --arg denominator "$DENOMINATOR_PACKET" --arg denominator_sha "$(sha256sum "$CONTROLLER_ROOT/$DENOMINATOR_PACKET" | awk '{print $1}')" \
  --arg audit "$AUDIT_PACKET" --arg audit_sha "$(sha256sum "$CONTROLLER_ROOT/$AUDIT_PACKET" | awk '{print $1}')" \
  --arg readback "$PIN_READBACK_PATH" --arg readback_sha "$(sha256sum "$CONTROLLER_ROOT/$PIN_READBACK_PATH" | awk '{print $1}')" \
  --arg ruleset "$PIN_RULESET_PATH" --arg ruleset_sha "$(sha256sum "$CONTROLLER_ROOT/$PIN_RULESET_PATH" | awk '{print $1}')" \
  --slurpfile range "$DENOMINATOR_RANGE_JSON" --slurpfile proof "$PROOF_INPUTS_JSON" --slurpfile accepted "$OWNER_ACCEPTANCE_JSON" '
  .status = "pinned_exact_head" |
  .candidate = {repository: "EffortlessMetrics/ripr-swarm", sha: $sha, tree: $tree, ref: $ref,
    package: {name: "ripr", version: .release_line, workspace_manifest_sha256: $workspace_sha,
      package_manifest_sha256: $package_sha, lock_sha256: $lock_sha}} |
  .range = $range[0] |
  .prerequisites = {selected_claims: {packet: {owner_issue: 2766, path: $selection, sha256: $selection_sha}, acceptance: $accepted[0].selected_claims},
    denominator: {packet: {owner_issue: 2768, path: $denominator, sha256: $denominator_sha}, acceptance: $accepted[0].denominator},
    audit: {packet: {owner_issue: 3807, path: $audit, sha256: $audit_sha}, acceptance: $accepted[0].audit}} |
  .pin = {remote_ref_readback: {owner_issue: 1609, path: $readback, sha256: $readback_sha},
    ruleset: {owner_issue: 1609, path: $ruleset, sha256: $ruleset_sha}} |
  .qualification.proof_inputs = $proof[0] |
  .source_parent = null' "$PACKET_ROOT/live-head-selection-template.json" > "$PACKET_ROOT/live-head-selection.json"
# Generate the handoff from this same DTO; no independent hand-authored identity projection.
jq -r '["# Swarm candidate manifest", "", ("Candidate: " + .candidate.repository + "@" + .candidate.sha),
  ("Tree: " + .candidate.tree), ("Ref: " + .candidate.ref),
  ("Qualification: " + .qualification.state), "Source parent: unbound until source #1769", "", "Non-claims:"]
  + (.non_claims | map("- " + .)) | .[]' "$PACKET_ROOT/live-head-selection.json" > "$PACKET_ROOT/live-head-selection.md"
# STOP: retain this proposed output for #1609 acceptance. Hashing it is not approval.
# Only after independent #1609 review, the trusted release operator supplies the accepted digest.
ACCEPTED_MANIFEST_SHA256="${ACCEPTED_MANIFEST_SHA256:?obtain from the reviewed #1609 handoff, not this producer}"
test "$(sha256sum "$PACKET_ROOT/live-head-selection.json" | awk '{print $1}')" = "$ACCEPTED_MANIFEST_SHA256"
```

At every later receipt, read `refs/tags/$PIN_TAG` again and require the
recorded SHA and the exact ruleset receipt to remain unchanged. The protected
candidate tag, not the local verifier ref, is the W membership pin.

The manifest records the accepted #2768 boundary, separately named
all-reachable and first-parent counts/digests, record-set identity, exact
candidate tree/ref/package, accepted owner packets and claims/non-claims.
It does not predict SOURCE_PARENT or the later source-specific merge base.
The schema-1.1 range uses the shared source-promotion recipe in SPEC-0144,
with the #2768 boundary rather than the later source-specific merge base.
Admission recomputes both Git ranges/counts/digests; record-set adjudication
remains with #2768. The retained input budget is 64 proof references, 16 MiB
per file and 64 MiB aggregate. Supported origin strings are the exact three
spellings checked above. Files are observed unlocked snapshots, not atomic
or authenticated custody. Repin only for a release-invalidating exact-candidate
semantic/policy failure or source-preflight survivor failure. Main movement
alone never repins. Changed inputs supersede the packet and require a new one.

## 3. Exact swarm qualification and source preflight

Qualification is a detached worktree at `SWARM_PARENT`; a hosted result must
report that exact SHA as `headSha`.

```bash
set -euo pipefail
# [LOCAL-MUTATING] repo=swarm root; create disposable exact-head worktree
QUAL_ROOT="${SWARM_ROOT}-release-${VERSION}-qual"
git -C "$SWARM_ROOT" worktree add --detach "$QUAL_ROOT" "$SWARM_PARENT"
test "$(git -C "$QUAL_ROOT" rev-parse HEAD)" = "$SWARM_PARENT"
```

```bash
set -euo pipefail
# [LOCAL-MUTATING] repo=swarm qualification worktree; checks write local reports
assert_live_pin_guard
git -C "$QUAL_ROOT" status --short --branch
git -C "$QUAL_ROOT" rev-parse HEAD
(cd "$QUAL_ROOT" && cargo xtask check-pr)
(cd "$QUAL_ROOT" && cargo xtask check-generated-clean)
(cd "$QUAL_ROOT" && cargo xtask check-doc-index)
# These are preparation checks only. Do not manufacture an acceptance receipt
# from an arbitrary successful hosted run or its head SHA.
# #2769 must independently review the complete exact-candidate bundle first.
SELECTION_DECISION="${SELECTION_DECISION:?native #1609 acceptance-comment URL}"
QUALIFICATION_DECISION="${QUALIFICATION_DECISION:?native #2769 complete-bundle acceptance-comment URL}"
QUALIFICATION_BUNDLE="${QUALIFICATION_BUNDLE:?controller-relative accepted complete qualification bundle}"
```

A missing, timed-out, or differently headed hosted result is unavailable, not
a pass. A generic successful CI run is never the #2769 acceptance record.
The native decision and bundle field contract is in
[SOURCE_PROMOTION_PREFLIGHT.md](SOURCE_PROMOTION_PREFLIGHT.md#native-selection-and-complete-qualification-admission). After #2769 accepts the complete exact-candidate bundle, source #1769
rereads and dispositions the source queue, then binds the source parent:

```bash
set -euo pipefail
# [READ-ONLY] repo=source; performed only after complete swarm qualification
SOURCE_PARENT="$(git -C "$SOURCE_ROOT" rev-parse refs/remotes/origin/main)"
test "$(git -C "$SOURCE_ROOT" rev-parse HEAD)" = "$SOURCE_PARENT"
test -z "$(git -C "$SOURCE_ROOT" status --porcelain=v1 --untracked-files=all)"
# [LOCAL-MUTATING] repo=packet; distinct later source transaction identity
printf '%s\n' "$SOURCE_PARENT" > "$PACKET_ROOT/SOURCE_PARENT"
MERGE_BASE="$(git -C "$SWARM_ROOT" merge-base "$SOURCE_PARENT" "$SWARM_PARENT")"
```

Source movement after this hold requires a new source generation. It does not
retroactively mutate the swarm manifest. Run the existing exact-pair preflight with exact SHA declarations; its
contract is [`SOURCE_PROMOTION_PREFLIGHT.md`](SOURCE_PROMOTION_PREFLIGHT.md)
and [`RIPR-SPEC-0148`](specs/RIPR-SPEC-0148-source-promotion-preflight.md).
The resolution reviewer produces `JOIN_TREE` as a full tree SHA in the
resolution manifest; the automatic `preview_tree` is never substituted.

```bash
set -euo pipefail
# [LOCAL-MUTATING] repo=swarm operator checkout; preflight writes local receipts, no J
JOIN_TREE="${JOIN_TREE:?set to the separately reviewed full resolved-tree SHA from the resolution manifest}"
assert_live_pin_guard
(cd "$SWARM_ROOT" && cargo xtask source-promotion preflight \
  --source-parent "$SOURCE_PARENT" --swarm-parent "$SWARM_PARENT" \
  --swarm-ref "$SWARM_REF" --source-repo "$SOURCE_ROOT" --swarm-repo "$SWARM_ROOT" \
  --source-main "$SOURCE_PARENT" --swarm-main "$SWARM_PARENT" \
  --version "$VERSION" --resolved-tree "$JOIN_TREE" \
  --controller-root "$PACKET_ROOT" --candidate-manifest live-head-selection.json \
  --selection-decision "$SELECTION_DECISION" \
  --qualification-bundle "$QUALIFICATION_BUNDLE" \
  --qualification-decision "$QUALIFICATION_DECISION" \
  --out "$PACKET_ROOT/source-promotion")
PREFLIGHT_JSON="$PACKET_ROOT/source-promotion/source-promotion-preflight.json"
test "$(jq -r '.dry_merge.reviewed_resolved_tree // empty' "$PREFLIGHT_JSON")" = "$JOIN_TREE"
```

The v2 receipt cannot be supplied to source main
`82b2d7c262d229d5244263d458d10cd0189cb966`'s v1-only verifier. The transaction
remains held until [ripr#1769](https://github.com/EffortlessMetrics/ripr/issues/1769)
delivers coordinated v2 acceptance consumption; no v1 fallback waives it.

Review every conflict path, survivor, swarm exclusion, authority candidate,
and separately reviewed `JOIN_TREE`; record a resolution manifest. A clean
textual merge is not semantic approval. Parent, ref, identity, ancestry,
digest, conflict, or tree changes invalidate the receipt.

## 4. Construct and prove J in source

J is the source-promotion join:

```text
J.parent[0] = SOURCE_PARENT
J.parent[1] = SWARM_PARENT
```

The exact-J verifier is the separate source contract from
[#1493](https://github.com/EffortlessMetrics/ripr/issues/1493), merged at
[`9c53b12b`](https://github.com/EffortlessMetrics/ripr/commit/9c53b12b73aa5a11c198f8c5265902127a6d7dff).
Use its verifier and receipt; this runbook does not duplicate its semantics.

> **Do not run the construction and publication blocks below for 0.11.0.**
> They state the required graph only. ripr#1772 rejects a `J` built with raw
> `git merge` and pushed as a branch. For 0.11.0, ripr#1772 builds and
> publishes `J` with the source repository's guarded constructor
> (`source-promotion admit-resolved-tree`, `construct-exact-join`,
> `publish-candidate-ref`; see ripr `docs/SOURCE_PROMOTION.md`), and ripr#1773
> merges it with a guarded expected-head and expected-base merge commit.
> Conflict dispositions, the trial join, and tree qualification are described
> in [`swarm-development.md`](swarm-development.md#resolving-the-join).

```bash
set -euo pipefail
# [LOCAL-MUTATING] repo=source promotion checkout; construct reviewed J
assert_live_pin_guard
git -C "$SOURCE_ROOT" fetch "$SWARM_ORIGIN" "$SWARM_PARENT"
test "$(git -C "$SOURCE_ROOT" rev-parse "$SWARM_PARENT^{commit}")" = "$SWARM_PARENT"
git -C "$SOURCE_ROOT" switch -c "promote/${VERSION}-swarm" "$SOURCE_PARENT"
git -C "$SOURCE_ROOT" merge --no-ff --no-commit "$SWARM_PARENT"
# Resolve only paths named by the reviewed preflight manifest.
git -C "$SOURCE_ROOT" commit -m "promote: join frozen ripr-swarm candidate for ${VERSION}"
J="$(git -C "$SOURCE_ROOT" rev-parse HEAD)"
```

```bash
set -euo pipefail
# [EXTERNAL-PUBLISHING] repo=source; publish the reviewed promotion branch
git -C "$SOURCE_ROOT" push origin "promote/${VERSION}-swarm:refs/heads/promote/${VERSION}-swarm"
REMOTE_PROMOTION_HEAD="$(git -C "$SOURCE_ROOT" ls-remote origin "refs/heads/promote/${VERSION}-swarm" | awk '{print $1}')"
test "$REMOTE_PROMOTION_HEAD" = "$J"
SOURCE_PROMOTION_PR_URL="$(gh pr create --repo EffortlessMetrics/ripr --head "promote/${VERSION}-swarm" --base main --title "promote: join frozen swarm ${VERSION}" --body "Review exact J=${J}; parents are SOURCE_PARENT=${SOURCE_PARENT} and SWARM_PARENT=${SWARM_PARENT}. Ref #1493.")"
SOURCE_PROMOTION_PR="${SOURCE_PROMOTION_PR_URL##*/}"
test -n "$SOURCE_PROMOTION_PR"
```

```bash
set -euo pipefail
# [READ-ONLY] repo=source; bind exactly one reviewed promotion PR
SOURCE_PROMOTION_PR="$(gh pr list --repo EffortlessMetrics/ripr --head "$(git -C "$SOURCE_ROOT" branch --show-current)" --state open --json number --jq 'if length == 1 then .[0].number else empty end')"
test -n "$SOURCE_PROMOTION_PR"
```

```bash
set -euo pipefail
# [READ-ONLY] repo=source promotion checkout; exact J shape/tree/ancestry
assert_live_pin_guard
test "$(git -C "$SOURCE_ROOT" show -s --format='%P' "$J" | awk '{print NF}')" -eq 2
test "$(git -C "$SOURCE_ROOT" show -s --format='%P' "$J" | awk '{print $1}')" = "$SOURCE_PARENT"
test "$(git -C "$SOURCE_ROOT" show -s --format='%P' "$J" | awk '{print $2}')" = "$SWARM_PARENT"
git -C "$SOURCE_ROOT" merge-base --is-ancestor "$SOURCE_PARENT" "$J"
git -C "$SOURCE_ROOT" merge-base --is-ancestor "$SWARM_PARENT" "$J"
test "$(git -C "$SOURCE_ROOT" rev-parse "$J^{tree}")" = "$JOIN_TREE"
```

Never append a repair commit; squash, rebase, cherry-pick, and tree-equivalent
reconstruction fail the contract. Rebuild J from the held exact pair after a
changed resolution or receipt.

```bash
set -euo pipefail
# [LOCAL-MUTATING] repo=source remote; fetch and guard expected head before J transport
git -C "$SOURCE_ROOT" fetch origin main
test "$(git -C "$SOURCE_ROOT" rev-parse refs/remotes/origin/main)" = "$SOURCE_PARENT"
```

```bash
set -euo pipefail
# [EXTERNAL-PUBLISHING] repo=source PR; merge only reviewed J with expected head
assert_live_pin_guard
gh pr merge "$SOURCE_PROMOTION_PR" --repo EffortlessMetrics/ripr --merge --match-head-commit "$J"
```

```bash
set -euo pipefail
# [LOCAL-MUTATING] repo=source remote; fetch and bind source main to the new merge result
assert_live_pin_guard
git -C "$SOURCE_ROOT" fetch origin main
SOURCE_JOIN_HEAD="$(git -C "$SOURCE_ROOT" rev-parse refs/remotes/origin/main)"
test "$SOURCE_JOIN_HEAD" != "$J"
test "$(git -C "$SOURCE_ROOT" show -s --format='%P' "$SOURCE_JOIN_HEAD" | awk '{print NF}')" -eq 2
test "$(git -C "$SOURCE_ROOT" show -s --format='%P' "$SOURCE_JOIN_HEAD" | awk '{print $1}')" = "$SOURCE_PARENT"
test "$(git -C "$SOURCE_ROOT" show -s --format='%P' "$SOURCE_JOIN_HEAD" | awk '{print $2}')" = "$J"
test "$(git -C "$SOURCE_ROOT" show -s --format='%P' "$J" | awk '{print NF}')" -eq 2
test "$(git -C "$SOURCE_ROOT" show -s --format='%P' "$J" | awk '{print $1}')" = "$SOURCE_PARENT"
test "$(git -C "$SOURCE_ROOT" show -s --format='%P' "$J" | awk '{print $2}')" = "$SWARM_PARENT"
```

## 5. Metadata and source artifact qualification

J carries the promoted graph. Version/changelog metadata is a separate source
release-preparation change after J reaches source main; never bump J.

> **0.11.0:** the version write already landed in ripr#1708, and
> `bump-version` rejects a no-op bump, so skip the `bump-version` line below.
> Per ripr#1466, do not open a replacement metadata PR. The release-copy PR
> that ripr#1466 owns (folding `Unreleased` into the staged `0.11.0` section)
> takes its place, and `SOURCE_RELEASE_HEAD` is source `main` after it merges.

```bash
set -euo pipefail
# [LOCAL-MUTATING] repo=source release-prep checkout at source join result; metadata only
git -C "$SOURCE_ROOT" switch -c "release/${VERSION}" "$SOURCE_JOIN_HEAD"
(cd "$SOURCE_ROOT" && cargo xtask bump-version "$VERSION")
git -C "$SOURCE_ROOT" add Cargo.toml Cargo.lock crates docs
git -C "$SOURCE_ROOT" commit -m "chore(release): bump ${VERSION} metadata"
git -C "$SOURCE_ROOT" push origin "release/${VERSION}"
SOURCE_METADATA_PR="$(gh pr create --repo EffortlessMetrics/ripr --head "release/${VERSION}" --base main --title "chore(release): bump ${VERSION} metadata" --body "Release metadata only; expected source join ${SOURCE_JOIN_HEAD}")"
SOURCE_METADATA_PR_NUMBER="${SOURCE_METADATA_PR##*/}"
gh pr view "$SOURCE_METADATA_PR_NUMBER" --repo EffortlessMetrics/ripr --json headRefOid --jq .headRefOid | tee "$PACKET_ROOT/source-metadata-pr-head.sha"
test "$(cat "$PACKET_ROOT/source-metadata-pr-head.sha")" = "$(git -C "$SOURCE_ROOT" rev-parse HEAD)"
gh pr merge "$SOURCE_METADATA_PR_NUMBER" --repo EffortlessMetrics/ripr --squash --match-head-commit "$(cat "$PACKET_ROOT/source-metadata-pr-head.sha")"
```

Set `SOURCE_RELEASE_HEAD` to the exact source-main SHA after that PR. The ship
packet needs fresh evidence for all of these, bound to that SHA:

```bash
set -euo pipefail
# [LOCAL-MUTATING] repo=source+packet; produce the release-head binding after metadata merge
git -C "$SOURCE_ROOT" fetch origin main
SOURCE_RELEASE_HEAD="$(git -C "$SOURCE_ROOT" rev-parse refs/remotes/origin/main)"
test "$(git -C "$SOURCE_ROOT" rev-parse "$SOURCE_RELEASE_HEAD^{commit}")" = "$SOURCE_RELEASE_HEAD"
git -C "$SOURCE_ROOT" switch --detach "$SOURCE_RELEASE_HEAD"
test -z "$(git -C "$SOURCE_ROOT" status --short)"
(cd "$SOURCE_ROOT" && cargo xtask release-readiness --version "$VERSION")
```

| Surface | Repository/worktree and mode | Proof | Boundary |
| Crate | source exact release checkout, **[LOCAL-MUTATING]** for package output; **[READ-ONLY]** dry-run query | `(cd "$SOURCE_ROOT" && cargo package -p ripr --list)`; `(cd "$SOURCE_ROOT" && cargo publish -p ripr --dry-run)` | Package/publishability, not publication |
| Installed CLI | source exact release checkout, **[LOCAL-MUTATING]** | `(cd "$SOURCE_ROOT" && cargo install --path crates/ripr --locked --force --root target/ripr/install-smoke)` plus version/doctor/fixture smoke | Local installed binary |
| Readiness/consumer | source exact release checkout, **[LOCAL-MUTATING]** | `(cd "$SOURCE_ROOT" && cargo xtask release-readiness --version <VERSION>)` | Version alignment and journey receipt |
| Linux/Windows | source release workflow artifacts, **[READ-ONLY]** when inspecting exact `SOURCE_RELEASE_HEAD` outputs | Workflow artifacts bound to `SOURCE_RELEASE_HEAD` | Platform proof; one OS is not the other |
| Server | source exact release checkout, **[LOCAL-MUTATING]** | `(cd "$SOURCE_ROOT" && cargo xtask release-server-archive ...)`; `(cd "$SOURCE_ROOT" && cargo xtask release-server-manifest ...)` | Archive, manifest, checksum shape |
| VSIX | source exact release checkout, **[LOCAL-MUTATING]** | `npm --prefix editors/vscode ci`, compile, package | Local package/version proof |
| Badge | source exact release checkout, **[LOCAL-MUTATING]** | `(cd "$SOURCE_ROOT" && cargo xtask repo-badge-artifacts)` plus generated-clean/badge policy checks | Generated endpoint/freshness proof |

Use [`RELEASE_BINARIES.md`](RELEASE_BINARIES.md),
[`RELEASE_MARKETPLACE.md`](RELEASE_MARKETPLACE.md),
[`INSTALLATION_VERIFICATION.md`](INSTALLATION_VERIFICATION.md), and
[`BADGE_POLICY.md`](BADGE_POLICY.md) for surface semantics. Record exact SHA,
command, status, artifact path, and limitation; never copy an old blanket
no-execution sentence.

## 6. Ship packet, #1470 authorization, channels, and verification

The packet is evidence, not permission. Before external operations, source
issue #1470 must name `VERSION`, `SOURCE_RELEASE_HEAD`, and the authorized
channels/order. If absent, stale, or silent, stop.

```bash
set -euo pipefail
# [LOCAL-MUTATING] repo=packet; capture #1470 authorization and public state
assert_live_pin_guard
AUTHORIZATION_RECEIPT="$PACKET_ROOT/publication-authorization.json"
AUTHORIZATION_BODY="$PACKET_ROOT/publication-authorization.md"
gh issue view 1470 --repo EffortlessMetrics/ripr --json state,title,body,url > "$AUTHORIZATION_RECEIPT"
gh issue view 1470 --repo EffortlessMetrics/ripr --json body --jq .body > "$AUTHORIZATION_BODY"
gh release list --repo EffortlessMetrics/ripr --limit 5 > "$PACKET_ROOT/releases-before.txt"
```

```bash
set -euo pipefail
# [READ-ONLY] repo=source/public services; fail closed on #1470 authorization
test "$(jq -r .state "$AUTHORIZATION_RECEIPT")" = OPEN
grep -Fxq "VERSION=${VERSION}" "$AUTHORIZATION_BODY"
grep -Fxq "SOURCE_RELEASE_HEAD=${SOURCE_RELEASE_HEAD}" "$AUTHORIZATION_BODY"
grep -Fxq "CHANNEL_ORDER=crate,github-release,server-binaries,vs-marketplace,open-vsx" "$AUTHORIZATION_BODY"
git -C "$SOURCE_ROOT" rev-parse "$SOURCE_RELEASE_HEAD"
```

Publish one channel at a time and receipt it before the next. These templates
require #1470; they are not permission to publish from this documentation PR.

```bash
set -euo pipefail
# [EXTERNAL-PUBLISHING] repo=source; explicit #1470 crate/tag authorization only
assert_live_pin_guard
test -z "$(git -C "$SOURCE_ROOT" ls-remote origin "refs/tags/v${VERSION}" | awk '{print $1}')"
git -C "$SOURCE_ROOT" push origin "$SOURCE_RELEASE_HEAD:refs/tags/v${VERSION}"
```

```bash
set -euo pipefail
# [LOCAL-MUTATING] repo=packet; capture the exact published tag target
gh api "repos/EffortlessMetrics/ripr/git/ref/tags/v${VERSION}" --jq .object.sha > "$PACKET_ROOT/tag-object.sha"
test "$(tr -d '\r\n' < "$PACKET_ROOT/tag-object.sha")" = "$SOURCE_RELEASE_HEAD"
```

```bash
set -euo pipefail
# [EXTERNAL-PUBLISHING] repo=source; explicit #1470 crate authorization only
(cd "$SOURCE_ROOT" && cargo publish -p ripr)
```

```bash
set -euo pipefail
# [READ-ONLY] repo=source/public; receipt crate publication before next channel
curl -fsSL -A "ripr-release-transaction/${VERSION} (+https://github.com/EffortlessMetrics/ripr)" "https://crates.io/api/v1/crates/ripr/${VERSION}" | jq -e --arg version "$VERSION" '.version == $version' >/dev/null
```

```bash
set -euo pipefail
# [EXTERNAL-PUBLISHING] repo=source; explicit #1470 GitHub-release authorization only
assert_live_pin_guard
gh release create "v${VERSION}" --repo EffortlessMetrics/ripr --target "$SOURCE_RELEASE_HEAD" --title "ripr ${VERSION}" --notes-file "$RELEASE_NOTES"
```

```bash
set -euo pipefail
# [READ-ONLY] repo=source/public; receipt GitHub release before next channel
gh release view "v${VERSION}" --repo EffortlessMetrics/ripr --json tagName,targetCommitish,isDraft,isPrerelease,assets,url
gh release view "v${VERSION}" --repo EffortlessMetrics/ripr --json tagName,targetCommitish,isDraft,isPrerelease > "$PACKET_ROOT/github-release-receipt.json"
jq -e --arg tag "v${VERSION}" --arg head "$SOURCE_RELEASE_HEAD" '.tagName == $tag and .targetCommitish == $head and .isDraft == false and .isPrerelease == false' "$PACKET_ROOT/github-release-receipt.json" >/dev/null
```

```bash
set -euo pipefail
# [EXTERNAL-PUBLISHING] repo=source; explicit #1470 server-artifact authorization only
DISPATCHED_AT="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
RELEASE_REF="v${VERSION}"
test "$(gh api "repos/EffortlessMetrics/ripr/git/ref/tags/${RELEASE_REF}" --jq .object.sha)" = "$SOURCE_RELEASE_HEAD"
gh api --paginate --slurp "repos/EffortlessMetrics/ripr/actions/workflows/release-server-binaries.yml/runs?per_page=100&event=workflow_dispatch" | jq '[.[] | .workflow_runs[]]' > "$PACKET_ROOT/server-runs-before.json"
gh workflow run release-server-binaries.yml --repo EffortlessMetrics/ripr --ref "$RELEASE_REF" -f version="$VERSION"
```

```bash
set -euo pipefail
# [LOCAL-MUTATING] repo=packet; paginate and bind the dispatched server run
gh api --paginate --slurp "repos/EffortlessMetrics/ripr/actions/workflows/release-server-binaries.yml/runs?per_page=100&event=workflow_dispatch" | jq '[.[] | .workflow_runs[]]' > "$PACKET_ROOT/server-runs-after.json"
SERVER_RUN_ID="$(bind_new_dispatch_run "$PACKET_ROOT/server-runs-before.json" "$PACKET_ROOT/server-runs-after.json" "$SOURCE_RELEASE_HEAD" "$RELEASE_REF" "$DISPATCHED_AT" "repos/EffortlessMetrics/ripr/actions/workflows/release-server-binaries.yml/runs?per_page=100&event=workflow_dispatch")"
wait_for_run_success "$SERVER_RUN_ID"
gh run view "$SERVER_RUN_ID" --repo EffortlessMetrics/ripr --json databaseId,headSha,status,conclusion,url > "$PACKET_ROOT/server-run-receipt.json"
jq -e --arg sha "$SOURCE_RELEASE_HEAD" '.headSha == $sha and .status == "completed" and .conclusion == "success"' "$PACKET_ROOT/server-run-receipt.json" >/dev/null
```

```bash
set -euo pipefail
# [EXTERNAL-PUBLISHING] repo=source; explicit #1470 VS Marketplace authorization only
DISPATCHED_AT="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
gh api --paginate --slurp "repos/EffortlessMetrics/ripr/actions/workflows/publish-extension.yml/runs?per_page=100&event=workflow_dispatch" | jq '[.[] | .workflow_runs[]]' > "$PACKET_ROOT/extension-runs-before.json"
gh workflow run publish-extension.yml --repo EffortlessMetrics/ripr --ref "$RELEASE_REF" -f version="$VERSION" -f publish_vs_marketplace=true -f publish_open_vsx=false
```

```bash
set -euo pipefail
# [LOCAL-MUTATING] repo=packet; paginate and bind the dispatched VS Marketplace run
gh api --paginate --slurp "repos/EffortlessMetrics/ripr/actions/workflows/publish-extension.yml/runs?per_page=100&event=workflow_dispatch" | jq '[.[] | .workflow_runs[]]' > "$PACKET_ROOT/extension-runs-after.json"
EXTENSION_RUN_ID="$(bind_new_dispatch_run "$PACKET_ROOT/extension-runs-before.json" "$PACKET_ROOT/extension-runs-after.json" "$SOURCE_RELEASE_HEAD" "$RELEASE_REF" "$DISPATCHED_AT" "repos/EffortlessMetrics/ripr/actions/workflows/publish-extension.yml/runs?per_page=100&event=workflow_dispatch")"
wait_for_run_success "$EXTENSION_RUN_ID"
gh run view "$EXTENSION_RUN_ID" --repo EffortlessMetrics/ripr --json databaseId,headSha,status,conclusion,url > "$PACKET_ROOT/vs-marketplace-run-receipt.json"
jq -e --arg sha "$SOURCE_RELEASE_HEAD" '.headSha == $sha and .status == "completed" and .conclusion == "success"' "$PACKET_ROOT/vs-marketplace-run-receipt.json" >/dev/null
```

```bash
set -euo pipefail
# [EXTERNAL-PUBLISHING] repo=source; explicit #1470 Open VSX authorization only
DISPATCHED_AT="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
gh api --paginate --slurp "repos/EffortlessMetrics/ripr/actions/workflows/publish-extension.yml/runs?per_page=100&event=workflow_dispatch" | jq '[.[] | .workflow_runs[]]' > "$PACKET_ROOT/extension-runs-open-vsx-before.json"
gh workflow run publish-extension.yml --repo EffortlessMetrics/ripr --ref "$RELEASE_REF" -f version="$VERSION" -f publish_vs_marketplace=false -f publish_open_vsx=true
```

```bash
set -euo pipefail
# [LOCAL-MUTATING] repo=packet; paginate and bind the dispatched Open VSX run
gh api --paginate --slurp "repos/EffortlessMetrics/ripr/actions/workflows/publish-extension.yml/runs?per_page=100&event=workflow_dispatch" | jq '[.[] | .workflow_runs[]]' > "$PACKET_ROOT/extension-runs-open-vsx-after.json"
OPENVSX_RUN_ID="$(bind_new_dispatch_run "$PACKET_ROOT/extension-runs-open-vsx-before.json" "$PACKET_ROOT/extension-runs-open-vsx-after.json" "$SOURCE_RELEASE_HEAD" "$RELEASE_REF" "$DISPATCHED_AT" "repos/EffortlessMetrics/ripr/actions/workflows/publish-extension.yml/runs?per_page=100&event=workflow_dispatch")"
wait_for_run_success "$OPENVSX_RUN_ID"
gh run view "$OPENVSX_RUN_ID" --repo EffortlessMetrics/ripr --json databaseId,headSha,status,conclusion,url > "$PACKET_ROOT/open-vsx-run-receipt.json"
jq -e --arg sha "$SOURCE_RELEASE_HEAD" '.headSha == $sha and .status == "completed" and .conclusion == "success"' "$PACKET_ROOT/open-vsx-run-receipt.json" >/dev/null
```

After partial publication, stop later channels, record public state, and obtain
a corrective-release decision through #1470. Never delete or retarget a public
tag, or claim a complete release.

## 7. Construct and transport K after public verification

Freeze exact post-publication inputs:

```text
SWARM_BEFORE        = exact swarm main before back-sync
SOURCE_RELEASE_HEAD = exact released source main
BACK_SYNC_TREE      = reviewed tree retaining swarm authority
K.parent[0]         = SWARM_BEFORE
K.parent[1]         = SOURCE_RELEASE_HEAD
```

Before leaving the source publication phase, write the frozen values once and
never replace them with a later floating-head lookup:

```bash
set -euo pipefail
# [LOCAL-MUTATING] repo=packet; persist frozen K inputs for later guards
test ! -e "$PACKET_ROOT/SWARM_BEFORE" && test ! -e "$PACKET_ROOT/SOURCE_RELEASE_HEAD"
printf '%s\n' "$SWARM_PARENT" > "$PACKET_ROOT/SWARM_BEFORE"
printf '%s\n' "$SOURCE_RELEASE_HEAD" > "$PACKET_ROOT/SOURCE_RELEASE_HEAD"
```

The exact-K contract is [`BACK_SYNC_VERIFIER.md`](BACK_SYNC_VERIFIER.md),
[`RIPR-SPEC-0149`](specs/RIPR-SPEC-0149-back-sync-verifier.md), and
[#3100](https://github.com/EffortlessMetrics/ripr-swarm/issues/3100), merged
at [`4589b85a`](https://github.com/EffortlessMetrics/ripr-swarm/commit/4589b85a8f63529c4838abe5ee54a78d28d989c7). Use its verifier and receipt;
this runbook does not redefine its semantics.

```bash
set -euo pipefail
# [READ-ONLY] repo=swarm+source; freeze K pair and pre-transport heads
test -s "$PACKET_ROOT/SWARM_BEFORE" && test -s "$PACKET_ROOT/SOURCE_RELEASE_HEAD"
SWARM_BEFORE="$(tr -d '\r\n' < "$PACKET_ROOT/SWARM_BEFORE")"
SOURCE_RELEASE_HEAD="$(tr -d '\r\n' < "$PACKET_ROOT/SOURCE_RELEASE_HEAD")"
CURRENT_SWARM_HEAD="$(git -C "$SWARM_ROOT" rev-parse refs/remotes/origin/main)"
CURRENT_SOURCE_HEAD="$(git -C "$SOURCE_ROOT" rev-parse refs/remotes/origin/main)"
test "$CURRENT_SWARM_HEAD" = "$SWARM_BEFORE"
test "$CURRENT_SOURCE_HEAD" = "$SOURCE_RELEASE_HEAD"
test "$(git -C "$SWARM_ROOT" rev-parse "$SWARM_BEFORE^{commit}")" = "$SWARM_BEFORE"
test "$(git -C "$SOURCE_ROOT" rev-parse "$SOURCE_RELEASE_HEAD^{commit}")" = "$SOURCE_RELEASE_HEAD"
```

```bash
set -euo pipefail
# [LOCAL-MUTATING] repo=swarm back-sync checkout; construct reviewed K
assert_live_pin_guard
git -C "$SWARM_ROOT" fetch "$SOURCE_ORIGIN" "$SOURCE_RELEASE_HEAD"
test "$(git -C "$SWARM_ROOT" rev-parse "$SOURCE_RELEASE_HEAD^{commit}")" = "$SOURCE_RELEASE_HEAD"
git -C "$SWARM_ROOT" switch -c "back-sync/${VERSION}" "$SWARM_BEFORE"
git -C "$SWARM_ROOT" merge --no-ff --no-commit "$SOURCE_RELEASE_HEAD"
# Retain swarm checks/runners/settings/authority; import release receipts only.
git -C "$SWARM_ROOT" commit -m "sync: back-sync released ${VERSION} from ripr"
K="$(git -C "$SWARM_ROOT" rev-parse HEAD)"
BACK_SYNC_TREE="${BACK_SYNC_TREE:?set to the separately reviewed back-sync tree SHA}"
test "$(git -C "$SWARM_ROOT" rev-parse "$K^{tree}")" = "$BACK_SYNC_TREE"
```

After K, BACK_SYNC_TREE, and all authorized channel receipts are available,
produce the exact release-receipt schema consumed by the K verifier. Do this
before either verifier invocation; its semantics remain owned by
[`BACK_SYNC_VERIFIER.md`](BACK_SYNC_VERIFIER.md).

```bash
set -euo pipefail
# [LOCAL-MUTATING] repo=packet; produce verifier-bound publication receipt
jq -n --arg version "$VERSION" --arg source_release_head "$SOURCE_RELEASE_HEAD" --arg join "$K" --arg tree "$BACK_SYNC_TREE" --arg source_release_tag "v${VERSION}" --slurpfile server "$PACKET_ROOT/server-run-receipt.json" --slurpfile marketplace "$PACKET_ROOT/vs-marketplace-run-receipt.json" --slurpfile open_vsx "$PACKET_ROOT/open-vsx-run-receipt.json" '{schema_version: 1, version: $version, source_release_head: $source_release_head, join: $join, tree: $tree, source_release_tag: $source_release_tag, channels: {server_binaries: $server[0], vs_marketplace: $marketplace[0], open_vsx: $open_vsx[0]}}' > "$PUBLICATION_RECEIPT"
jq -e --arg version "$VERSION" --arg source_release_head "$SOURCE_RELEASE_HEAD" --arg join "$K" --arg tree "$BACK_SYNC_TREE" '.version == $version and .source_release_head == $source_release_head and .join == $join and .tree == $tree and .source_release_tag == ("v" + $version) and (.channels | keys | sort) == ["open_vsx", "server_binaries", "vs_marketplace"]' "$PUBLICATION_RECEIPT" >/dev/null
```

```bash
set -euo pipefail
# [LOCAL-MUTATING] repo=swarm; write policy-before evidence
gh api repos/EffortlessMetrics/ripr-swarm/branches/main/protection > "$POLICY_BEFORE"
gh api --paginate --slurp repos/EffortlessMetrics/ripr-swarm/rulesets > "$PACKET_ROOT/policy-rulesets-before.json"
ACTIVE_RULESET_IDS="$(jq -r '.[][] | select(.enforcement == "active" and .target == "branch") | .id' "$PACKET_ROOT/policy-rulesets-before.json")"
for ruleset_id in $ACTIVE_RULESET_IDS; do
  gh api "repos/EffortlessMetrics/ripr-swarm/rulesets/${ruleset_id}" > "$PACKET_ROOT/ruleset-${ruleset_id}-before.json"
done
```

```bash
set -euo pipefail
# [LOCAL-MUTATING] repo=swarm+packet; fetch and verify before-state and owner authorization
jq -e '.allow_force_pushes.enabled == false and .allow_deletions.enabled == false' "$POLICY_BEFORE" >/dev/null
test -s "$POLICY_APPROVAL"
POLICY_OWNER_LOGIN="${POLICY_OWNER_LOGIN:?set the expected approving owner login}"
test "$(gh api user --jq .login)" = "$POLICY_OWNER_LOGIN"
POLICY_BEFORE_SHA="$(sha256sum "$POLICY_BEFORE" | awk '{print $1}')"
POLICY_VALIDATED_AT="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
POLICY_NOW_EPOCH="$(date -u -d "$POLICY_VALIDATED_AT" +%s)"
POLICY_EXPIRES_AT="$(jq -r '.expires_at // empty' "$POLICY_APPROVAL")"
printf '%s\n' "$POLICY_EXPIRES_AT" | grep -Eq '^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}(Z|[+-][0-9]{2}:[0-9]{2})$'
printf '%s\n' "$POLICY_EXPIRES_AT" | grep -Eq '^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z$'
POLICY_EXPIRES_EPOCH="$(date -u -d "$POLICY_EXPIRES_AT" +%s 2>/dev/null || true)"
test "$(date -u -d "@$POLICY_EXPIRES_EPOCH" +%Y-%m-%dT%H:%M:%SZ)" = "$POLICY_EXPIRES_AT"
test -n "$POLICY_EXPIRES_EPOCH"
test "$POLICY_EXPIRES_EPOCH" -gt "$POLICY_NOW_EPOCH"
jq -n --arg validated_at "$POLICY_VALIDATED_AT" --arg expires_at "$POLICY_EXPIRES_AT" --argjson validated_epoch "$POLICY_NOW_EPOCH" --argjson expires_epoch "$POLICY_EXPIRES_EPOCH" '{schema_version: 1, validated_at: $validated_at, expires_at: $expires_at, validated_epoch: $validated_epoch, expires_epoch: $expires_epoch}' > "$PACKET_ROOT/policy-approval-validation.json"
jq -e --arg owner "$POLICY_OWNER_LOGIN" --arg version "$VERSION" --arg swarm "$SWARM_BEFORE" --arg source "$SOURCE_RELEASE_HEAD" --arg before_sha "$POLICY_BEFORE_SHA" '
  .schema_version == 1 and .approval_issue == 3104 and
  .owner.login == $owner and .approved_for == "ancestry-preserving-back-sync" and
  .version == $version and .swarm_before == $swarm and
  .source_release_head == $source and .before_sha256 == $before_sha and
  (.protected_layers | type == "array") and
  (.expires_at | type == "string") and (.approved_at | type == "string")
' "$POLICY_APPROVAL" >/dev/null
for ruleset_id in $ACTIVE_RULESET_IDS; do
  jq -e --arg id "$ruleset_id" '.protected_layers | index("ruleset:" + $id) != null' "$POLICY_APPROVAL" >/dev/null
done
git -C "$SWARM_ROOT" fetch origin main
test "$(git -C "$SWARM_ROOT" rev-parse refs/remotes/origin/main)" = "$SWARM_BEFORE"
```

The following is the copyable, owner-authorized external-setting operation.
It is not run by this documentation PR. It preserves the current branch
protection payload while changing only `required_linear_history` for the
ancestry-preserving merge; if linear history is already disabled, it records
the before response as the effective exception without changing settings.

```bash
set -euo pipefail
# [LOCAL-MUTATING] repo=packet; build exact branch-protection exception request
jq '{
  required_status_checks: (if .required_status_checks == null then null else {strict: .required_status_checks.strict, contexts: .required_status_checks.contexts, checks: .required_status_checks.checks} end),
  enforce_admins: .enforce_admins.enabled,
  required_pull_request_reviews: null,
  restrictions: (if .restrictions == null then null else {users: .restrictions.users, teams: .restrictions.teams, apps: .restrictions.apps} end),
  required_linear_history: false,
  allow_force_pushes: .allow_force_pushes.enabled,
  allow_deletions: .allow_deletions.enabled,
  block_creations: .block_creations.enabled,
  required_conversation_resolution: .required_conversation_resolution.enabled,
  lock_branch: .lock_branch.enabled,
  allow_fork_syncing: .allow_fork_syncing.enabled
}' "$POLICY_BEFORE" > "$POLICY_EXCEPTION_REQUEST"
```

```bash
set -euo pipefail
# [EXTERNAL-PUBLISHING] repo=swarm branch protection; owner-approved setting mutation only
if test "$(jq -r '.required_linear_history.enabled' "$POLICY_BEFORE")" = true; then
  gh api --method PUT repos/EffortlessMetrics/ripr-swarm/branches/main/protection --input "$POLICY_EXCEPTION_REQUEST" > "$POLICY_EXCEPTION"
else
  cp "$POLICY_BEFORE" "$POLICY_EXCEPTION"
fi
for ruleset_id in $ACTIVE_RULESET_IDS; do
  RULESET_EXCEPTION_REQUEST="$PACKET_ROOT/ruleset-${ruleset_id}-exception-request.json"
  RULESET_BEFORE="$PACKET_ROOT/ruleset-${ruleset_id}-before.json"
  jq '(.rules // []) as $rules | ($rules | map(select(.type == "pull_request" or .type == "required_linear_history"))) as $approved | if ($approved | length) > 0 then .rules = ($rules | map(select(.type != "pull_request" and .type != "required_linear_history"))) else . end' "$RULESET_BEFORE" > "$RULESET_EXCEPTION_REQUEST"
  jq -e --slurpfile before "$RULESET_BEFORE" '((del(.rules) == ($before[0] | del(.rules))) and ((($before[0].rules // []) | map(select(.type == "pull_request" or .type == "required_linear_history")) | length) > 0) and ((.rules // []) | all(.type != "pull_request" and .type != "required_linear_history"))) or ((. == $before[0]) and (($before[0].rules // []) | map(select(.type == "pull_request" or .type == "required_linear_history")) | length) == 0)' "$RULESET_EXCEPTION_REQUEST" >/dev/null
  if test "$(jq '[.rules[]? | select(.type == "pull_request" or .type == "required_linear_history")] | length' "$RULESET_BEFORE")" -gt 0; then
    gh api --method PUT "repos/EffortlessMetrics/ripr-swarm/rulesets/${ruleset_id}" --input "$RULESET_EXCEPTION_REQUEST" > "$PACKET_ROOT/ruleset-${ruleset_id}-exception.json"
  else
    cp "$RULESET_BEFORE" "$PACKET_ROOT/ruleset-${ruleset_id}-exception.json"
    printf '%s\n' "no approved K-blocking rules present; no ruleset mutation" > "$PACKET_ROOT/ruleset-${ruleset_id}-no-mutation.txt"
  fi
  jq -e --slurpfile before "$RULESET_BEFORE" '((del(.rules) == ($before[0] | del(.rules))) and ((.rules // []) == (($before[0].rules // []) | map(select(.type != "pull_request" and .type != "required_linear_history"))))) or (. == $before[0])' "$PACKET_ROOT/ruleset-${ruleset_id}-exception.json" >/dev/null
done
```

Each active ruleset is reviewed as a separate protection layer. A ruleset may
be listed as unchanged in the owner receipt, but a layer that blocks the exact
K direct update requires its own owner-created PUT payload and temporary
response. The branch exception clears only the main-branch pull-request review
requirement needed for this direct K update; required status checks, force-push
protection, deletion protection, and every unrelated rule remain enabled. No
unrelated rule may be weakened.

```bash
set -euo pipefail
# [READ-ONLY] repo=swarm policy; verify effective exception before K transport
jq -e '.required_linear_history.enabled == false and .allow_force_pushes.enabled == false and .allow_deletions.enabled == false' "$POLICY_EXCEPTION" >/dev/null
jq -e '.required_pull_request_reviews == null' "$POLICY_EXCEPTION" >/dev/null
for ruleset_id in $ACTIVE_RULESET_IDS; do
  test -s "$PACKET_ROOT/ruleset-${ruleset_id}-exception.json"
done
```

Restore the recorded before-state immediately after transport. Never
force-push, squash, rebase, or merge unrelated work.

`POLICY_APPROVAL` is an owner-provided JSON receipt with this minimum schema;
the owner login is also checked against the authenticated `gh api user`:

```json
{
  "schema_version": 1,
  "approval_issue": 3104,
  "owner": {"login": "release-owner"},
  "approved_for": "ancestry-preserving-back-sync",
  "version": "0.11.0",
  "swarm_before": "<40-hex-sha>",
  "source_release_head": "<40-hex-sha>",
  "before_sha256": "<64-hex-sha256>",
  "protected_layers": ["branch:main", "ruleset:<id>"],
  "approved_at": "<RFC3339>",
  "expires_at": "<RFC3339>"
}
```

```bash
set -euo pipefail
# [EXTERNAL-PUBLISHING] repo=swarm remote/policy; owner-approved K transport only
git -C "$SWARM_ROOT" fetch origin main
test "$(git -C "$SWARM_ROOT" rev-parse refs/remotes/origin/main)" = "$SWARM_BEFORE"
git -C "$SWARM_ROOT" push origin "back-sync/${VERSION}:refs/heads/main"
for ruleset_id in $ACTIVE_RULESET_IDS; do
  RULESET_BEFORE_REQUEST="$PACKET_ROOT/ruleset-${ruleset_id}-before-request.json"
  test -s "$RULESET_BEFORE_REQUEST"
  gh api --method PUT "repos/EffortlessMetrics/ripr-swarm/rulesets/${ruleset_id}" --input "$RULESET_BEFORE_REQUEST" > "$PACKET_ROOT/ruleset-${ruleset_id}-after.json"
done
```

```bash
set -euo pipefail
# [LOCAL-MUTATING] repo=swarm remote; fetch and bind swarm main to K after transport
git -C "$SWARM_ROOT" fetch origin main
test "$(git -C "$SWARM_ROOT" rev-parse refs/remotes/origin/main)" = "$K"
```

```bash
set -euo pipefail
# [LOCAL-MUTATING] repo=packet; build exact before-state restoration request
jq '{required_status_checks: (if .required_status_checks == null then null else {strict: .required_status_checks.strict, contexts: .required_status_checks.contexts, checks: .required_status_checks.checks} end), enforce_admins: .enforce_admins.enabled, required_pull_request_reviews: (if .required_pull_request_reviews == null then null else {dismiss_stale_reviews: .required_pull_request_reviews.dismiss_stale_reviews, require_code_owner_reviews: .required_pull_request_reviews.require_code_owner_reviews, required_approving_review_count: .required_pull_request_reviews.required_approving_review_count, require_last_push_approval: .required_pull_request_reviews.require_last_push_approval} end), restrictions: (if .restrictions == null then null else {users: .restrictions.users, teams: .restrictions.teams, apps: .restrictions.apps} end), required_linear_history: .required_linear_history.enabled, allow_force_pushes: .allow_force_pushes.enabled, allow_deletions: .allow_deletions.enabled, block_creations: .block_creations.enabled, required_conversation_resolution: .required_conversation_resolution.enabled, lock_branch: .lock_branch.enabled, allow_fork_syncing: .allow_fork_syncing.enabled}' "$POLICY_BEFORE" > "$POLICY_RESTORE_REQUEST"
```

```bash
set -euo pipefail
# [EXTERNAL-PUBLISHING] repo=swarm branch protection; restore exact before-state
gh api --method PUT repos/EffortlessMetrics/ripr-swarm/branches/main/protection --input "$POLICY_RESTORE_REQUEST" > "$POLICY_AFTER"
```

```bash
set -euo pipefail
# [READ-ONLY] repo=swarm policy; prove normal enforcement is restored
jq -e --slurpfile before "$POLICY_BEFORE" '(.required_status_checks == $before[0].required_status_checks) and (.enforce_admins == $before[0].enforce_admins) and (.required_pull_request_reviews == $before[0].required_pull_request_reviews) and (.restrictions == $before[0].restrictions) and (.required_linear_history == $before[0].required_linear_history) and (.allow_force_pushes == $before[0].allow_force_pushes) and (.allow_deletions == $before[0].allow_deletions) and (.block_creations == $before[0].block_creations) and (.required_conversation_resolution == $before[0].required_conversation_resolution) and (.lock_branch == $before[0].lock_branch) and (.allow_fork_syncing == $before[0].allow_fork_syncing)' "$POLICY_AFTER" >/dev/null
for ruleset_id in $ACTIVE_RULESET_IDS; do
  jq -S '{name,target,enforcement,conditions,rules}' "$PACKET_ROOT/ruleset-${ruleset_id}-before.json" > "$PACKET_ROOT/ruleset-${ruleset_id}-before-shape.json"
  jq -S '{name,target,enforcement,conditions,rules}' "$PACKET_ROOT/ruleset-${ruleset_id}-after.json" > "$PACKET_ROOT/ruleset-${ruleset_id}-after-shape.json"
  cmp "$PACKET_ROOT/ruleset-${ruleset_id}-before-shape.json" "$PACKET_ROOT/ruleset-${ruleset_id}-after-shape.json"
done
```

```bash
set -euo pipefail
# [READ-ONLY] repo=swarm+source; verify exact K after transport and restoration
(cd "$SWARM_ROOT" && cargo xtask back-sync verify \
  --swarm-before "$SWARM_BEFORE" --source-release-head "$SOURCE_RELEASE_HEAD" \
  --source-release-tag "v${VERSION}" --join "$K" --tree "$BACK_SYNC_TREE" \
  --swarm-repo "$SWARM_ROOT" --source-repo "$SOURCE_ROOT" --version "$VERSION" \
  --release-receipt "$PUBLICATION_RECEIPT" --policy-before "$POLICY_BEFORE" \
  --policy-exception "$POLICY_EXCEPTION" --policy-after "$POLICY_AFTER" \
  --swarm-main "$K" --source-main "$SOURCE_RELEASE_HEAD" \
  --out "$PACKET_ROOT/back-sync-after")
```

J is `[SOURCE_PARENT, SWARM_PARENT]`; K is `[SWARM_BEFORE,
SOURCE_RELEASE_HEAD]`. Neither proves release correctness, artifact adequacy,
publication success, or semantic equivalence. Source publication workflows and
settings remain ancestry evidence and never become swarm authority.

## 8. Re-baseline and clean up

Only after K is reachable from swarm main, policy is restored, and the
publication receipt is complete may development resume. Re-baseline `0.11.1`
from exact K, not a candidate or floating branch.

```bash
set -euo pipefail
# [LOCAL-MUTATING] repo=swarm; fetch and reopen development only from published K
git -C "$SWARM_ROOT" fetch origin main
test "$(git -C "$SWARM_ROOT" rev-parse refs/remotes/origin/main)" = "$K"
git -C "$SWARM_ROOT" log --oneline --decorate -n 5 "$K"
```

Remove only clean worktrees, local branches, and scratch files created by this
transaction. Keep shared Cargo/npm caches and historical receipts.

```bash
set -euo pipefail
# [LOCAL-MUTATING] repo=swarm+source; lane-created clean surfaces only
test -z "$(git -C "$QUAL_ROOT" status --short)"
git -C "$SWARM_ROOT" worktree remove "$QUAL_ROOT"
git -C "$SWARM_ROOT" switch --detach "$K"
git -C "$SWARM_ROOT" branch -d "back-sync/${VERSION}"
git -C "$SOURCE_ROOT" switch --detach "$SOURCE_RELEASE_HEAD"
git -C "$SOURCE_ROOT" branch -d "promote/${VERSION}-swarm"
git -C "$SOURCE_ROOT" branch -d "release/${VERSION}" 2>/dev/null || true
```

## Failure recovery and rollback

- Before pin: fix premise or queue disposition; no release claim exists.
- After pin: changed input supersedes the receipt; retain it and repin only
  under section 2's rule.
- J conflict/verifier failure: regenerate preflight and rebuild J; never append
  a repair commit.
- Source metadata/artifact failure: do not tag or publish; refresh affected
  receipts after a separate source fix.
- Partial publication: stop later channels, record public state, and obtain a
  corrective-release decision through #1470; do not rewrite J.
- K expected-head/policy failure: restore policy, retain untransported K, and
  return to the owner-approved transport decision; never force-push.
- After K: do not rewrite it to repair a receipt; a corrective source release
  requires a new authorized transaction if swarm must receive it.

The safe rollback for this documentation lane is to revert its PR. This
runbook never authorizes direct `origin/main` mutation, force pushes, deletion
of ambiguous work, or deletion of historical evidence.
