# RIPR-SPEC-0144: Release-Control Lens and Live-Head Authority

Status: accepted

Owner: release control / swarm operations

Created: 2026-07-29

Linked issues:

- [#2766](https://github.com/EffortlessMetrics/ripr-swarm/issues/2766) — bind
  0.11 work selection and merge eligibility to the live #2379 graph.
- [#2379](https://github.com/EffortlessMetrics/ripr-swarm/issues/2379) — live
  0.11.0 release authority and exact transaction-boundary head.
- [`0.11.0-live-head-selection.json`](../release-candidates/0.11.0-live-head-selection.json)
  — checked-in superseding decision.

Support-tier impact:

- No product support-tier change. This is a maintainer-facing, read-only
  control-plane report and cannot strengthen analyzer, release, or merge claims.
- Reference: [support tiers](../status/SUPPORT_TIERS.md).

Policy impact:

- No release publication, source integration, version, credential, workflow,
  dependency, or secret change.

## Problem

The old 0.11 hard-cut authority selected a development cut `C` and a
candidate-only projection `T`. That authority is historical. The live release
must instead select the exact `ripr-swarm/main` SHA at the release transaction
boundary, pin it immutably, and qualify that same object. An open-ended session
must not let later main movement silently enter the release.

## Behavior

`cargo xtask release-control --input <snapshot.json>` replays a captured,
schema-versioned snapshot. `cargo xtask release-control --live` collects the
current `origin/main`, open PR inventory, and #2379 state through bounded
read-only adapters. Both paths produce a snapshot containing the observed
`main` SHA, #2379 authority, portfolio and active-claim completeness, and every
open PR. The command sorts PR records by number and derives both JSON and
Markdown from one normalized report.

Every PR must carry one of these dispositions:

- `release_required`;
- `release_optional_pending_decision`;
- `hold_post_release`;
- `blocked_on_named_authority`.

Only a complete, current snapshot can produce `status = ready`, and only a
non-draft `release_required` row can be `merge_eligible = true`. Missing,
stale, contradictory, duplicated, or malformed authority input produces
`status = reconcile_required` and clears merge eligibility for every row.
Missing per-PR disposition defaults visibly to
`blocked_on_named_authority` while retaining the reconciliation reason.

The report is advisory and report-only. It does not close issues, relabel
items, merge or rebase PRs, create or delete branches, select a candidate,
qualify a release, or mutate development `main`.

## Active live-head boundary

The active release candidate is exactly the accepted transaction-boundary
`ripr-swarm/main` commit, its actual tree and complete reachable history.
[#1609](https://github.com/EffortlessMetrics/ripr-swarm/issues/1609) owns the
reviewed manifest and pin. A later control commit is not candidate product code.

### Manifest schema 1.1

The checked-in JSON is an `active_selection_template`, never an executable
qualification grant. Its nullable transaction fields become present only in
one `pinned_exact_head` manifest. The direct consumer rejects older/unknown
schemas, unknown fields, the template state, and missing/null bindings.

```text
schema_version = "1.1"
kind = "ripr_swarm_live_head_release_authority"
release_line = "0.11.0"
authority_issue = 2379
candidate_owner_issue = 1609
status = "pinned_exact_head"
candidate = repository, sha, tree, ref, package
package = name, version, workspace_manifest_sha256,
          package_manifest_sha256, lock_sha256
range = last_integrated_swarm_parent, all_reachable_count,
        first_parent_count, all_reachable_sha256, first_parent_sha256,
        record_set_sha256
prerequisites = selected_claims(#2766), denominator(#2768), audit(#3807)
pin = remote_ref_readback(#1609), ruleset(#1609)
qualification = state("required_not_run"), required_execution_owners,
                proof_inputs
source_parent = null
non_claims = nonempty explicit claim limits
```

Every evidence reference carries `owner_issue`, a controller-relative `path`,
and the raw lowercase 64-hex `sha256`. Each of the three prerequisite references
has the shape `{ packet: Evidence, acceptance: OwnerAcceptance }`. The proposed
small `OwnerAcceptance` envelope is part of this same manifest, not another
registry or an audit engine:

```text
status = "accepted"
candidate_sha / candidate_tree = the exact manifest candidate
reviewed_packet_sha256 = packet.sha256
decision_ref = exact existing owner issue's native acceptance-comment URL
```

The trusted release controller imports this envelope from the independently
reviewed owner decision. The producer must not manufacture it. Missing,
non-accepted, wrong-candidate, wrong-packet or absent-owner-reference status is
`not_established`, even when the expected manifest hash matches. A syntactically
valid URL is a binding to the reviewed decision, not independent authentication
that GitHub or a human approved it. The release operator must verify the native
decision before accepting the manifest's raw digest. This adapter does not
re-prove the human #3807 judgment or interpret historical green report fields. Candidate commit/tree IDs are complete
lowercase 40-hex Git objects; the protected ref is exactly
`refs/tags/ripr-release-0.11.0-<candidate.sha>`. Package identities bind actual
committed `Cargo.toml`, `crates/ripr/Cargo.toml` and `Cargo.lock` bytes. The
package/version must equal the admitted source. The repository is exactly
`EffortlessMetrics/ripr-swarm`. The direct consumer supports exactly these
case-sensitive origin strings: `https://github.com/EffortlessMetrics/ripr-swarm`,
`https://github.com/EffortlessMetrics/ripr-swarm.git`, and
`git@github.com:EffortlessMetrics/ripr-swarm.git`. Other transports, aliases,
ports and spellings refuse; this is not a general Git URL normalizer.

The range starts after `45b56c0957ad7e7360114edceca4b844c85f846e`. Schema 1.1
aligns with the existing source-promotion ordered-range helper:

- all reachable: `git rev-list --topo-order --reverse BASE..CANDIDATE`;
- first parent: `git rev-list --first-parent --reverse BASE..CANDIDATE`;
- each full SHA followed by LF, including the final LF; hash those UTF-8 bytes
  with SHA256 and retain lowercase hex without the `sha256:` prefix.

The adapter checks base ancestry, recomputes both counts and ordered digests
against the admitted source, and rechecks them at source custody boundaries.
Replacement refs are disabled. These digests are not interchangeable with the
historical denominator's JSON hashes. The accepted #2768 packet must supply
this exact recipe. `record_set_sha256` remains the reviewed owner's normalized
record-set claim bound by the retained packet and independent manifest digest;
this adapter does not reconstruct or adjudicate the ledger.
The selected-claim/denominator/audit owners retain semantic acceptance. Their
raw packet bindings are required, but a digest or self-issued `passed` label
cannot establish that their predicates were accepted.

The pin readback is the exact candidate SHA followed by one newline. The
retained ruleset names `release-transaction-pins`, targets the exact
`refs/tags/ripr-release-*` namespace without exclusions or bypass actors, is
active and contains update and deletion protection. The authorized operator
must retain fresh native remote/ref/protection observations before and after
execution. Local custody checks do not manufacture remote authenticity.

### Independent acceptance and explicit admission modes

#1609 accepts the complete manifest and its prerequisite packet identities
before qualification. The operator supplies that independently accepted raw
manifest SHA256 through `--candidate-manifest-sha256`; a producer must never
compute its own output digest and silently treat that as acceptance. The
constructor compares the expected raw digest before parsing and revalidates
all captured bytes. Retained input limits are 64 proof references, 16 MiB per
file, and 64 MiB total including the manifest and five prerequisite/pin files.
Reads use a file handle with a limit+1 byte cap, regular-file checks, and
before/after observed length/modification time (plus device/inode on Unix).
Paths are rechecked for containment. These are unlocked snapshots: swaps
between observations, same-size/timestamp-preserving writes and later mutation
remain possible; no atomic, locked or authenticated filesystem claim is made.
It does not approve a release or authenticate the actor.
An unreviewed sidecar, active template, same-version executable, or historical
receipt cannot stand in for this accepted input.

The existing #4915 corpus arguments select two explicit modes:

- controller/source/artifact only: retained historical registry admission;
- that complete group plus accepted manifest SHA256: direct schema-1.1 #1609
  admission, independent of milestone policy/registry state.

A partial, malformed or refused direct input fails before packaging. There is
no automatic fallback between modes or into ambient legacy smoke. The broader
#3924 lifecycle-consumer defer is preserved; existing registry policy checks
remain enforced separately.

### Consumed downstream native decisions

Digest-pinned direct custody remains preparation/execution evidence. The source
handoff now requires SPEC-0148's native #1609 and #2769 decisions, retrieved
through the existing authenticated read-only controller route. #1609's strict
acceptance payload binds this whole raw manifest, the #2766 packet and native
comment-body digest, selected applicable owner roster, proof-input identities,
complete required qualification rows and any explicit excluded subjects.
#2769 binds that same native selection body and the complete raw result bundle.
The broad matrix and selected successor obligations remain owner judgments;
no fixed template roster or generic CI result substitutes for their acceptance.

The actual source-promotion command compares these bindings, verifies trusted
GitHub association/location, reads every required result packet, rejects missing
or non-positive selected rows, and rereads custody before output. Native payload
unknown fields refuse. A locally authored acceptance file cannot stand in for
a retrieved decision. Existing historical registry/direct corpus modes do not
issue this handoff. The full field, count, budget and trusted-operator limitations
are in [SOURCE_PROMOTION_PREFLIGHT.md](../SOURCE_PROMOTION_PREFLIGHT.md#native-selection-and-complete-qualification-admission).

### Source and qualification stage separation

`source_parent` is null here: source #1769 binds it only after swarm
qualification, together with source preflight/guarded-join identities. The
swarm pin does not predict or hold source main. Required candidate executions
remain `required_not_run` in the immutable manifest, and later qualification
receipts bind its digest rather than editing that state in place. Proof-input
identities do not establish that their executions passed.

Later main movement does not retarget the pin. Changed candidate/ref/input
bytes refuse and require the governed successor. Full source/swarm histories
remain mandatory; the historical C-to-T recipes below do not become current
because their implementation still exists.

### Discriminating controls and implementation

`reports::release::candidate_harness::live_head` owns the direct typed input;
its sibling source/archive owners retain real Git, package and install custody.
Tests refuse the active template, missing bindings, unknown schema/fields,
self-issued input without an independently pinned digest, wrong repository/
ref/package, changed raw prerequisite bytes, wrong remote readback, inadequate
protection, empty required rows and predicted source parent. Existing real Git
controls retain moved-ref/HEAD/tree and source-substitution refusals. Actual
package/install and final-candidate proof are separate from those data controls.

## Historical candidate-relative hard-cut boundary

The open-PR inventory and each row's `merge_eligible` value are work-selection
and ownership observations. Neither is a repository-wide candidate-readiness
gate. In particular, the report must not require the open release-PR count to
reach zero before a candidate can be selected.

Candidate readiness is evaluated against a selected development cut `C`, a
selected claim set `S`, candidate-only exclusions `E`, and a reproducible
candidate tree `T = project(C, E)`. The hard-cut predicate is:

```text
candidate_required_claims_pending == 0
```

That predicate requires every selected claim to be landed by `C`, explicitly
excluded from `T`, or explicitly deferred with a truthful release non-claim;
no known unresolved defect may invalidate the selected claims; every commit
through `C` must have a reviewed disposition and candidate-tree state; and the
projection from `C` to `T` must be reproducible. Commits and PRs outside `S`
may remain open or land after `C` without affecting this candidate. They are
relevant only if they disclose a defect that invalidates `T`.

The historical candidate-control vocabulary is
`selected_candidate_claims`, `candidate_required_claims_pending`,
`candidate_claims_landed`, `candidate_claims_excluded`,
`candidate_claims_deferred`, `candidate_defects_unresolved`,
`denominator_decisions_remaining` (the schema-0.1 provisional-cutoff field),
`denominator_decisions_remaining_through_selected_cut`, `candidate_cut_selected`, and
`candidate_ref_created`. A hard-cut decision also requires a `final_cut_authority`
ledger bound to the selected cut. Its record-derived authority must report zero
provisional decisions, zero unreviewed records after the provisional cutoff
through the selected cut, and zero final-cut decisions; its review flag must be
true. An informational `open_release_pr_count` must not be used as a readiness
predicate.

The release-control snapshot may carry an optional historical `candidate_selection` DTO.
When it is absent, candidate state is `scope_pending`; the ordinary PR lens
remains replayable for disposition work, but it cannot imply candidate
readiness. The DTO is the #2766 authority for the selected claim set:

```text
CandidateSelection
  schema_version
  selected_cut_sha
  selected_claims[]
  candidate_exclusions[]
  known_candidate_defects[]
  denominator_decisions_remaining_through_provisional_cutoff
  denominator_decisions_remaining_through_selected_cut
  final_cut_authority
  projection
  qualification
```

`final_cut_authority` carries `cut_sha`, a normalized denominator
`record_set_digest`,
`provisional_decisions_remaining`,
`unreviewed_post_provisional_records_through_cut`,
`final_cut_decisions_remaining`, and `reviewed_through_selected_cut`. The
denominator normalizer derives these values and the digest from records keyed
by commit SHA and rejects disagreement with supplied authority. A
release-control snapshot must carry the same digest as normalized denominator
provenance before the final-cut authority can advance; a missing or mismatched
digest remains reconciliation-required. The legacy
`denominator_decisions_remaining` serialized field remains the schema-0.1 wire
name; the longer internal name is accepted as an input alias only.

Each selected claim carries `claim_id`, `owner_issue`,
`required_for_candidate`, one resolution (`pending`, `landed`,
`accepted_defer`, `candidate_exclusion`, or `failed`), evidence/commit/artifact
references, `candidate_effect`, an explicit `non_claim` when deferred or
excluded, and `reviewed`. A `landed` claim must bind at least one delivery
commit or artifact; generic issue references or an `acceptance_owner`
field cannot establish selected-claim satisfaction.

The staged candidate states are fail-closed and ordered:

```text
scope_pending
  → scope_closed
  → hard_cut_eligible
  → candidate_materialized
  → qualification_eligible
```

`scope_closed` requires a non-empty, unique, reviewed claim set with a current
resolution and explicit non-claims for defers/exclusions. `hard_cut_eligible`
also requires zero required claims pending, zero unresolved candidate defects,
the valid final-cut authority, a selected `C`, and a reproducible projection.
`candidate_materialized` additionally requires a candidate tree
whose parent is `C` and matching exclusion/preservation digests.
`qualification_eligible` additionally requires an immutable candidate ref, a
manifest naming the materialized tree, and available qualification instruments.
The immutable ref must use the repository-controlled
`refs/ripr/candidate-<identifier>` namespace; mutable branch refs such as
`refs/heads/main`, blank references, and whitespace-only values are not
qualification evidence. The immutable ref is intentionally not required for
hard-cut eligibility.

## Input contract

The input envelope has `schema_version = "0.1"` and
`kind = "release_control_snapshot"`. Captured input must use
`source.mode = "captured"`; `source.mode = "live"` is admitted only for the
internal `--live` collector, not for an input file. `source` must identify the
current main SHA, open #2379 state, matching authority/main identity, complete
portfolio and claim inventory observations, worktree inventory, current
freshness, and a non-empty graph digest. The live collector records worktree
inventory but deliberately leaves authority-main identity, portfolio
completeness, and active-claim completeness unresolved because its bounded
inputs do not prove those facts; the resulting report is therefore
`reconcile_required` until an approved source supplies them. PR rows carry a
number, title, open state, head SHA, `main` base ref, and explicit
disposition/reason.
An optional `candidate_selection` object carries the historical #2766
selected-claim authority and candidate-state inputs described above; its
absence is `scope_pending`, not a successful empty selection. This compatibility
input is not the active 0.11.0 release authority; the live-head transaction
receipt is authoritative for the current train.

The fixture corpus in `fixtures/release_control/` is manifest-only and is
validated by `cargo xtask check-fixture-contracts`. It includes a complete
snapshot and a stale/incomplete snapshot that must reconcile.

## Output contract

The command writes `target/ripr/reports/release-control.json` and
`target/ripr/reports/release-control.md`. JSON is schema `0.1` and contains the
normalized source observation, sorted PR rows, `reconciliation_reasons`, a
`status` of `ready` or `reconcile_required`, per-row `merge_eligible`, a
`candidate_state`, `next_action`, and the explicit
`authority_boundary`/`must_not_claim` fields. `candidate_state` is a staged
control signal and never changes the report's non-qualification boundary.
Markdown is a projection of that same normalized DTO; it cannot strengthen a
reconciliation-required state or any per-PR disposition.

## Acceptance

- fixed captured inputs produce byte-stable normalized JSON and Markdown;
- PR input order cannot change the normalized report;
- unrelated work remains visible and non-mergeable;
- live collection inventories current main/open PR/#2379 inputs;
- missing or stale authority never becomes merge eligibility;
- the report preserves an explicit claim boundary and next action;
- the command performs no external state mutation.

## Required Evidence

- `xtask/src/reports/release_control.rs` owns the captured-input schema,
  bounded live collectors, deterministic normalization, shared JSON/Markdown
  projection, and fail-closed merge eligibility.
- `xtask/src/reports/candidate_control.rs` owns the selected-claim DTO,
  candidate-state transitions, and fail-closed false-ready checks.
- `fixtures/release_control/complete.json` proves a complete current snapshot
  with both required and held rows; `fixtures/release_control/reconcile-required.json`
  proves stale authority cannot produce eligibility.
- `xtask/src/command.rs` and `xtask/src/dispatch.rs` expose the report as a
  report-only command, while `xtask/src/fixture_contracts/mod.rs` validates the
  fixture corpus shape.
- `.ripr/traceability.toml` links this specification to the focused tests,
  fixtures, implementation, and report outputs.

## Non-Goals

- no singleton active-goal restoration or automatic backlog priority;
- no candidate denominator, exact-candidate qualification, package proof,
  source handoff, version bump, tag, publication, signing, or marketplace;
- no repository-wide convergence requirement or open-PR-zero gate;
- no issue closure, merge queue, branch operation, or GitHub mutation;
- no replacement for #2379, #1609, #1704, or #1706;
- no historical candidate-only C-to-T construction as the active 0.11.0 path;
- exact live-head qualification and source preflight remain required after pinning;
- no source integration, versioning, tagging, publication, or marketplace
  mutation.

## Acceptance Examples

### Complete captured input is deterministic

```text
Given a current captured snapshot with complete authority and explicit PR
dispositions,
when `cargo xtask release-control --input` replays it,
then the JSON and Markdown reports are normalized in PR-number order and only
non-draft `release_required` rows are merge-eligible.
```

### Stale authority fails closed

```text
Given a snapshot whose authority main SHA or completeness fields are stale,
when the snapshot is normalized,
then the report is `reconcile_required` and every PR remains non-mergeable.
```

### Live collection discloses missing authority

```text
Given the bounded live collector can observe main, open PRs, and #2379,
when portfolio or active-claim authority is not supplied by that collector,
then the report remains `reconcile_required` and names the missing inputs.
```

### An over-bound open-PR inventory fails closed

```text
Given the live open-PR inventory reaches its bounded collection limit,
when the sentinel row shows that more rows exist,
then the inventory is marked incomplete and no row can become merge-eligible.
```

## Test Mapping

- `xtask/src/reports/release_control.rs::tests::complete_snapshot_is_ready_and_only_required_rows_are_merge_eligible`
- `xtask/src/reports/release_control.rs::tests::missing_candidate_selection_is_exposed_as_scope_pending`
  — absent candidate selection is reported as `scope_pending`.
- `xtask/src/reports/release_control.rs::tests::missing_disposition_fails_closed`
  — missing PR authority clears all eligibility.
- `xtask/src/reports/release_control.rs::tests::input_order_does_not_change_normalized_output`
  — JSON and Markdown are stable under input reordering.
- `xtask/src/reports/release_control.rs::tests::stale_authority_cannot_be_merge_eligible`
  — stale source identity cannot produce eligibility.
- `xtask/src/reports/release_control.rs::tests::unsupported_live_mode_cannot_be_merge_eligible`
  — captured replay cannot impersonate the live collector.
- `xtask/src/reports/release_control.rs::tests::collector_error_fails_closed_and_clears_eligibility`
  — collector failures remain visible and non-mergeable.
- `xtask/src/reports/release_control.rs::tests::non_main_base_cannot_be_merge_eligible`
  — PRs targeting a non-release base are rejected.
- `xtask/src/reports/release_control.rs::tests::bounded_live_collector_normalizes_success_and_failure_inputs`
  — live command outputs and bounded failures are normalized explicitly.
- `xtask/src/reports/release_control.rs::tests::live_open_pr_bound_is_disclosed_and_fails_closed`
  — the sentinel row prevents a truncated open-PR inventory from appearing
  complete.

## Implementation Mapping

- `xtask/src/reports/release_control.rs` — snapshot types, live `git`/`gh`
  adapters, validation, disposition normalization, and report renderers.
- `xtask/src/command.rs` — command parsing and report-only command catalog
  entries.
- `xtask/src/dispatch.rs` — dispatch to the release-control report.
- `xtask/src/fixture_contracts/mod.rs` — release-control fixture contract
  validation.
- `fixtures/release_control/` — complete and reconcile-required captured
  inputs plus their fixture specification.
- `docs/OUTPUT_SCHEMA.md` — JSON/Markdown output shape and claim boundary.

## Metrics

- Focused release-control tests cover captured normalization, stale and
  malformed inputs, bounded live collection, and output escaping.
- `cargo xtask check-fixture-contracts` and `cargo xtask check-output-contracts`
  validate the fixture and report contracts.
- The report is advisory and does not publish a readiness, merge, or release
  metric; no product support-tier metric changes in this slice.

## Proof

```text
cargo test -p xtask release_control -- --nocapture
cargo xtask release-control --input fixtures/release_control/complete.json
cargo xtask release-control --live
cargo xtask check-output-contracts
cargo xtask check-fixture-contracts
cargo xtask check-pr
```

## Claim boundary

This spec proves only that a captured input or bounded live observation is
normalized into an explicit, deterministic, fail-closed disposition report. It
does not prove that a PR is correct, that a release candidate is qualified, or
that a merge is approved.
