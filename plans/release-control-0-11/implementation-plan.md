# 0.11 Release Control Lens

This campaign operationalizes the temporary release-selection boundary tracked
by [#2766](https://github.com/EffortlessMetrics/ripr-swarm/issues/2766) without
turning release work into a repository-wide task store or restoring singleton
active-goal authority.

The live baseline was initially `origin/main` at
`76193cf5da5a2ab7034a95752ff05bc3061a6228`; it advanced to
`19849177ce9418d0024e4cc839eb8382e4f716a8` and then to
`d1e43bbff1d7583cf276830f05bfdc445ad43d38` while this campaign was being
reconciled. The release authority remains
[#2379](https://github.com/EffortlessMetrics/ripr-swarm/issues/2379), while
#1704 and #1706 remain the longer-term portfolio and selected-work authorities.
Those identities must be reread before each release disposition; this plan is
execution context, not a replacement authority. The complete fixture is
updated to the later observed main and open-PR set; the earlier observation is
retained here as race evidence rather than treated as current authority.

## Objective

For the temporary 0.11 convergence window, make work selection and merge
eligibility consume one explicit, fail-closed release lens over the live
portfolio, current `main`, open PRs, and the accepted #2379 graph. Investigation
and branch preparation may continue outside that graph, but unrelated merges
must have an explicit hold or named authority before they can be treated as
eligible.

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

Observed after the latest live reconciliation: `origin/main` is
`d1e43bbff1d7583cf276830f05bfdc445ad43d38`, with open PR #2771 at head
`9762da8ecbabecb1fb585e59963b74f30ada88a3` and #2772 at head
`80bb56ad28112769d24d7af78c7f360de4d093c5`. Earlier PR #2770
merged as `d1e43bbff1d7583cf276830f05bfdc445ad43d38` during this campaign and
is therefore a denominator event for the later #2768 slice, not an open-PR
row in the current fixture.
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
`fixtures/release_denominator/current-main-provisional.json`: the first-parent
range `c86807ec..3c086540` contains 183 observed commits, in order, with
matching candidate-tree membership. That fixture is now historical evidence,
not a current-main census: #2797 and #2799 advanced `main` to
`62afe10678c93de3057153d703b41c5d2973d009`, and open PR #2800 still requires a
release disposition before merge. Every retained row is explicitly
`operator_decision_required`, so the fixture proves range completeness and
identity only; it is not a reviewed final denominator and cannot unblock #1609
or #2769. Refresh the range and candidate-tree record set only after the live
open-PR disposition is resolved.

## Non-goals and safety boundary

- no singleton active-goal restoration or automatic backlog priority;
- no issue closure, relabeling, PR merge, branch creation/deletion, rebase,
  force-push, source integration, version bump, tag, publish, signing,
  marketplace, or secret operation;
- no replacement for GitHub state, #2379, #1609, #1704, or #1706;
- no exact-candidate qualification until the later dependent slices are
  complete and the candidate identity is immutable.

## Closeout requirements

Before marking this campaign complete, reread live `main`, all open PRs,
active worktrees/claims, #2379, and the dependent issue state. Record merged
heads and proof per slice, distinguish infrastructure gaps from product
failures, clean only campaign-created worktrees/branches/artifacts, and leave
builder-ready follow-ups for anything still outside the claim boundary.
