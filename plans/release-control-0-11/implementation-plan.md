# 0.11 Release Control Lens

This campaign operationalizes the temporary release-selection boundary tracked
by [#2766](https://github.com/EffortlessMetrics/ripr-swarm/issues/2766) without
turning release work into a repository-wide task store or restoring singleton
active-goal authority.

The live baseline was initially `origin/main` at
`76193cf5da5a2ab7034a95752ff05bc3061a6228`; it advanced through several
reconciliations and was observed at `fcbb30a7cf6a37027fa377abafb617632b2e6f57`
on 2026-08-02. The later current-main observation is recorded below. The
release authority remains
[#2379](https://github.com/EffortlessMetrics/ripr-swarm/issues/2379), while
#1704 and #1706 remain the longer-term portfolio and selected-work authorities.
Those identities must be reread before each release disposition; this plan is
execution context, not a replacement authority. The complete fixture is
updated only when a bounded capture is intentionally refreshed; earlier
observations are retained as race evidence rather than treated as current
authority.

## Objective

For the temporary 0.11 release window, make work selection and candidate
readiness consume one explicit, fail-closed release lens over the selected
claim set, the development cut, candidate-tree projection, and the accepted
#2379 graph. The live portfolio and open-PR inventory remain inputs for
ownership and disposition reconciliation; they are not a repository-wide
convergence requirement.

## Candidate-relative hard-cut authority

The hard cut is relative to one selected development cut `C` and one selected
candidate claim set `S`. Define the candidate-only exclusion set as `E` and
the reproducible candidate projection as `T = project(C, E)`. The readiness
predicate is:

```text
candidate_required_claims_pending == 0
```

This is true only when every claim in `S` is landed at or before `C`,
explicitly excluded from `T`, or explicitly deferred with a truthful release
non-claim; no known unresolved defect invalidates the selected claims; every
commit through `C` has a reviewed disposition and candidate-tree state; and
the projection from `C` to `T` is reproducible. The denominator is therefore
cut-relative and must not chase later development commits.

An open-PR count is informational. Open release-labelled PRs, later-release
implementation, corrective experiments, and work that merges after `C` do not
block this candidate unless they belong to `S` or disclose a defect that
invalidates `T`. Release control does not close, relabel, rebase, pause, or
seize independently owned lanes to manufacture readiness.

The control and dashboard vocabulary is candidate-relative:

```text
selected_candidate_claims
candidate_required_claims_pending
candidate_claims_landed
candidate_claims_excluded
candidate_claims_deferred
candidate_defects_unresolved
denominator_decisions_remaining
candidate_cut_selected
candidate_ref_created
```

`open_release_pr_count` may be reported as context, but it is never a hard-cut
gate.

## Multi-slice sequence

| Slice | Tracker | Production/evidence seam | Exit condition |
| --- | --- | --- | --- |
| `control/release-lens` | #2766 | Fixture-backed `xtask` report with live-input reconciliation, closed PR disposition vocabulary, and fail-closed `reconcile_required` state. | A fixed captured board produces byte-stable JSON/Markdown; missing or stale inputs cannot produce merge eligibility. |
| `release/execution-scope` | #2767 | Candidate-only execution-surface disposition for merged #2396 and strictly dependent paths, preserving development `main`. | Exactly one scope outcome is machine-readable and candidate/docs/schema surfaces agree. |
| `release/supplemental-denominator` | #2768 | Deterministic commit-range enumeration and reviewed disposition ledger. | Missing, duplicate, out-of-range, wrong-order, wrong-tree, and unresolved-operator cases fail validation. |
| `release/exact-candidate-bundle` | #2769 | Exact-#1609 candidate qualification bundle and result taxonomy. | Every claimed result binds to one immutable candidate; missing/skipped/stale evidence is not qualified. |
| `campaign/release-control-closeout` | #2766/#2379 | Closeout, cleanup, disposition audit, and successor capture. | No publication/tag/source mutation; all remaining blockers and claim boundaries are recorded. |

Dependencies are strict: the execution-scope decision follows the lens;
the denominator follows the scope decision; exact-candidate qualification
follows the denominator and #1609; closeout follows the resulting evidence.
The campaign may investigate later slices early, but it must not claim them
complete or merge dependent work before their prerequisites are satisfied.

## Reconciled execution-surface map

The merged execution change is PR #2396, merge commit
`365f7d61e27fd441e997e6e17e3f3e28859a3964`. Its execution-only surface is the
new `verify-execute` command, the `verification_execution` implementation and
its command/result/schema/spec/allowlist surfaces, plus the directly related
CLI smoke and verification-result tests. Its own contract retains the
unreachable command-not-found, timeout, and composed-assurance cases, so the
release non-claim cannot be copied onto development `main` without a
candidate-only decision.

The later path overlap in commit `28512d07` (#2584) is reusable agent-repair
workflow work, not a dependency of `verify-execute`; the generic unknown-flag
change `e7838e0f` is likewise not execution-only. The candidate-scope slice
must therefore identify paths by semantic dependency and public surface, not
by every later commit that happened to touch `crates/ripr/src/cli/agent.rs`.
The scope decision remains a later slice and does not rewrite development
history.

## First slice acceptance

The first PR will implement only `control/release-lens`:

- consume a supplied captured JSON input for offline replay, with a separate
  live collection path if the repository already has an approved `gh` seam;
- record the observed `main` SHA, release-authority identity, open PR identity,
  and input completeness before emitting merge dispositions;
- use only these release dispositions: `release_required`,
  `release_optional_pending_decision`, `hold_post_release`, and
  `blocked_on_named_authority`;
- represent investigation/branch preparation as visible but non-mergeable;
- classify missing, stale, or contradictory inputs as `reconcile_required`,
  never as an eligible merge;
- derive human Markdown from the same normalized DTO as JSON;
- preserve open issues and PRs; the report performs no close, merge, label,
  rebase, branch, or publication operation;
- add fixture tests for a complete board, an unrelated held PR, a resumed
  existing PR, reordered input, and missing/stale authority input.

## Proof

Focused proof for the first slice:

```text
cargo test -p xtask release_control -- --nocapture
cargo xtask release-control --input fixtures/release_control/complete.json
cargo xtask check-output-contracts
cargo xtask check-fixture-contracts
cargo xtask check-pr
git diff --check
```

The report establishes deterministic, explainable, fail-closed release
disposition for the captured inputs. It does not establish candidate
qualification, package readiness, source compatibility, merge approval,
release publication, or product correctness.

## Current implementation receipt

Observed at `2026-08-02` during that reconciliation: `origin/main` was
`fcbb30a7cf6a37027fa377abafb617632b2e6f57`; the later 2026-08-03 refresh below
records the current `origin/main`. PR #2869 merged at that SHA and
delivered the typed reference-authority and fail-closed offline denominator
slice. Its final routed result was not a clean qualification signal, so this
merge is recorded as delivered implementation evidence, not candidate or
release qualification. The current open-PR inventory observed at
`2026-08-03T02:42:33-04:00` contains #2863, #2864, #2865, #2866, #2868,
#2871, #2872, and #2873. Those rows remain ownership/disposition context and
do not form a global hard-cut predicate.
Passed locally: `cargo metadata --no-deps --locked`, `cargo fmt --all --
--check`, standalone rustfmt for every changed Rust file, `cargo check -p xtask
--locked`, the focused `cargo test -p xtask release_control -- --nocapture`
(8 passed), captured/stale/live executable report assertions, fixture and
output-contract checks, dogfood (advisory `warn`, zero fixture-run errors),
Clippy with `-D warnings`, JSON fixture parsing, fixture-shape assertions, the
live adapter source-shape probe, and `git diff --check`. The aggregate
`cargo run -p xtask -- check-pr` was not completed: its full ripr compilation
timed out after 300 seconds without diagnostics. No aggregate check-pass claim
is made.

This slice is not the full #2766 closeout yet: `--live` now inventories
`origin/main`, open PRs, and #2379 through bounded read-only adapters, but it
does not pretend that those observations prove portfolio or active-claim
completeness. The live path therefore remains explicitly
`reconcile_required` until an approved source supplies those missing
authorities; a captured snapshot can become `ready` only when all required
identities and dispositions are present.

## Execution-scope slice in progress

The next slice implements #2767 Outcome A from the reconciled execution-surface
map: the accepted 0.11 non-claim remains unchanged, while the exact #2396
execution-only commit and its 18 changed paths are recorded as a candidate-only
exclusion. The report validates the captured parent `origin/main` SHA and
preserved provenance/static-assurance paths, keeps #2332 open, and explicitly
reports that candidate construction is not performed here. Its source of truth
is `RIPR-SPEC-0145`; the supplemental denominator and exact-candidate bundle
remain dependent follow-up slices.

## Supplemental denominator slice

Status: in progress. The execution-surface decision from #2767 is merged at
`b797cca3e63cf78d96265d9843c9ab03e8399945`. This slice adds the captured/live
ledger validator and its deterministic JSON/Markdown report. The provisional
ledger is intentionally not the final candidate disposition: #1609 and the
dependent editor lane remain outstanding.

Proof routes for this slice are the focused `RIPR-SPEC-0146` tests, the
`fixtures/release_denominator/` contract, and hosted `Ripr Rust Small Result`.
No final candidate qualification or publication claim is made here.

## Current-main provisional denominator evidence

PR #2790 merged at `3c08654028dcf20eb9bee5fbf3c67b3ef6111891` with deterministic
synthetic fixtures for validator shape. PR #2795 then merged at
`f55df6f67797de5d2fe4515b689aa7cea57669b4`, retaining the real census in
`fixtures/release_denominator/current-main-provisional.json`. This final refresh
rebases the provisional census to `origin/main`
`c30a26831b75051813bfaa3dbd9378096ec6aa82`: the first-parent range
`c86807ec..c30a2683` contains 234 observed commits and the retained provisional
tree contains 219 records. The imported fixture now carries typed GitHub
capture for all 234 rows, but all rows remain `operator_decision_required` and
the fifteen inherited blanket exclusions are represented as
`candidate_tree_state_pending`. The fixed provisional review cutoff for #2832
is `fcbb30a7cf6a37027fa377abafb617632b2e6f57`; later rows are retained as
observed delta, not silently excluded. The fixture is not a reviewed final
denominator and cannot unblock #1609 or #2769.

## Live reconciliation boundary (2026-08-03)

The release-control checkout was refreshed from current `origin/main`
`36105bbf7e33c2403b87a521bfdc404606700699` after #2871 and the independent
output/review lanes landed. PR #2868 has already merged at `e7c700ff...` as a
provisional census refresh; it is not #2831 PR B. The current bounded slice is
the fresh #2831 B branch: it adds GitHub capture/import, the fixed cutoff
boundary, pending candidate-tree state, and optional #2766/#2871 selected-claim
references. Concurrent product and release lanes remain independently owned
and are not required to close for this plan slice. The focused denominator
suite has 29 tests. The imported fixture pins range digest
`sha256:b85b8314b5f738335ae63220fe5f0ea8ef4e6e1892124eea148ea49181168501`,
candidate-tree digest
`sha256:c1b3675b6b98f609343f35711898e805a6ad27577c8f9b351ae53718b91082ae`,
and record-set digest
`sha256:172ef3d76ae3db47b8f7abedae9151ce971d3941b5a9eeb18b4c824d25c9530d`.

## Non-goals and safety boundary

- no singleton active-goal restoration or automatic backlog priority;
- no issue closure, relabeling, merge of concurrent PRs, branch deletion,
  rebase, force-push, source integration, version bump, tag, publish, signing,
  marketplace, or secret operation; the scoped #2831 A branch is the only
  implementation branch created by this slice;
- no replacement for GitHub state, #2379, #1609, #1704, or #1706;
- no exact-candidate qualification until a cut-relative selected claim set,
  denominator, reproducible projection, and immutable candidate identity exist.

## Closeout requirements

Before marking this campaign complete, select and record development cut `C`,
claim set `S`, candidate exclusions `E`, candidate tree `T`, and the exact
immutable candidate ref. Adjudicate every commit through `C`, record merged
heads and proof per selected slice, distinguish infrastructure gaps from
product failures, qualify only `T`, hand that exact identity to source, and
leave builder-ready follow-ups for anything outside the candidate claim
boundary. Re-read the live board for ownership and defect discovery, but do
not require the repository-wide open-PR count to reach zero.
