# Rust Judged Behavior Panel Seed

Authority: [#3164](https://github.com/EffortlessMetrics/ripr-swarm/issues/3164).

This directory fixes the first selected denominator for auditing both visible
bad repairs and silent over-credit in Rust analysis. It is deliberately a
**metrics seed**, not a runnable RIPR fixture, accuracy result, or judging
engine.

## Why emitted findings are insufficient

Reviewing only RIPR findings can identify a visible error:

```text
false_actionable
  RIPR routed a repair for behavior that was already discriminated
```

It cannot identify the quiet error:

```text
false_exposed
  RIPR credited exposed/strongly_gripped and emitted no repair,
  but no aligned oracle actually discriminates the behavior
```

The selected panel therefore contains all three directions:

| Direction | Expected static behavior | Error exposed by the case |
| --- | --- | --- |
| `should_gap` | report a gap, weakness, or exact missing discriminator | silent false-`exposed` if RIPR stays quiet |
| `should_stay_quiet` | retain exposed/strong evidence and route no repair | false-actionable if RIPR emits repair work |
| `should_limit` | retain a typed limitation and no repair | false-actionable or false-`exposed` if RIPR crosses the unresolved edge |

A gap-only panel cannot measure either side correctly.

## Files

- `manifest.json` — selected, unjudged cases. Every judgment label is `null`.
- `diffs/*.diff` — the exact changed production behavior for each synthetic
  seed case.
- `subjects.json` and `subjects/*` — independent checked source, test, config,
  diff, semantic-selection, and deterministic Git identity authority for later
  replay materialization.

The seed records the test/oracle evidence required for later reconstruction.
The subject authority proves that the three selected repositories materialize
at exact base/head/tree identities from independently hashed inputs. A later
execution PR must still build and invoke RIPR and retain host run receipts before
any replay result exists or any label is populated.

`cargo xtask rust-judged-panel check` is the retained semantic guard for this
seed, its subject authority, and the complete portable packet set. It validates the typed manifest contract,
selected directions, explicit null-as-unjudged state, exact Rust-token anchors,
subject digests, manifest joins, deterministic Git materialization, exact
direction-specific packet joins, packet self-digests, and the complete
packet-index/current chain. The same
checker is reached by required precommit policy. Passing it establishes only
that the selected seed, independent subject inputs, and retained bounded static
projection are internally coherent. It does not build or run RIPR, consult the
ignored host receipts, or establish analyzer correctness, judgment, mutation,
rate, badge, gate, or support.

## Host-bound replay receipts

`cargo xtask rust-judged-panel replay --out target/ripr/rust-judged-panel`
owns a fresh `cargo build -p ripr --locked --offline` in a run-local target,
then invokes those exact hashed bytes against all three materialized subjects.
It retains byte-exact stdout/stderr plus typed source, build, binary, host,
argv, config, diff, process, timeout, and analyzer-input identities below the
ignored `target/` tree.

The owned build removes inherited `CARGO_TARGET_DIR` and
`CARGO_BUILD_BUILD_DIR` before starting Cargo. The latter matters even with a
private `--target-dir`: Cargo can otherwise reuse a same-name/version path
dependency from shared intermediate storage and link old behavior into a new
application. With no `build.build-dir` configuration, Cargo places its
intermediate artifacts in the owned target as well. Normal caller overrides
outside this transaction, the workspace toolchain pin, and the existing
`CARGO_HOME` registry cache remain unchanged.

This is **environment-override isolation, not complete Cargo configuration
isolation**. A `build.build-dir` setting in a workspace, ancestor, or Cargo-home
configuration can still redirect intermediates. The separate #5038 claim
owns explicit per-command binding and the compatibility decision for historical
host receipts. The current receipt format records the final target command and
binary bytes; neither a current application stamp, successful build, correct
version nor a new binary hash establishes which dependency implementation was
linked. This limitation does not establish that any historical RIPR receipt was
contaminated.

The regression at `host_run/build_tests.rs` calls the real `build_fresh_binary`
owner in an isolated test subprocess. Two dependency-free, version-valid
workspaces have different same-name/version path-dependency implementations.
Both ordinary and inherited-shared-intermediate A → B → A sequences execute
three distinct binaries and compare actual subject/core behavior, with source
and binary hashes, raw build/behavior streams, and receipts under
`target/ripr/rust-judged-panel-host-tests/`. Failed attempts retain their evidence.
Successful test fixtures are removed by default; deliberate qualification can
retain the complete bytes with `RIPR_HOST_BUILD_KEEP_EVIDENCE=1`:

```sh
RIPR_HOST_BUILD_KEEP_EVIDENCE=1 cargo test -p xtask rust_judged_panel::host_run::build_tests::fresh_build_ignores_inherited_intermediates -- --exact --nocapture
```

Each attempt is staged under an exclusive lock. Only a validated three-case
generation receives `run-index.json`, is moved into the immutable `runs/`
namespace, and advances `current.json` last. A failed, partial, or concurrent
attempt cannot become current. The build has no network fallback: an offline
cache miss is a failed attempt.

These files are host-bound run receipts. They do not by themselves select or
bless findings, interpret quiet output, populate judgments, or support a
mutation, rate, gate, badge, or support-tier claim.

## Portable bounded projection

`cargo xtask rust-judged-panel packet --host-current
target/ripr/rust-judged-panel/current.json` first validates the complete
host-current/index/receipt/raw chain, then projects all three cases into the
checked `portable/` generation. Packets bind the governed subject, exact run
plan and input identity, exact probe path/line/family/expression and enclosing
Rust owner, observed class, and direction-specific witness. `should_stay_quiet`
is exactly one `exposed` finding with no action evidence; the macro case is a
complete `no_static_path` finding with its named static limitation, not a
timeout or incomplete run.

Each packet separates its portable semantic self-identity from host provenance.
Absolute checkout roots, run IDs, host targets, binary and raw-output digests
are excluded from the semantic digest, while the packet retains relative
host-receipt/raw references and their digests for provenance on the producing
host. The ignored raw files are intentionally unavailable in a clean checkout;
the retained checker revalidates the packet against the manifest and subject
authority without pretending to re-run or re-open that host evidence.

Publication stages and validates all three packets, publishes one immutable
content-addressed generation, then advances `portable/current.json` last under
an exclusive writer lock. A partial or concurrent attempt is non-authoritative.
Judgment remains explicitly null and runtime calibration remains `not_run`.

## Release-challenge selection

`release-selection.json` is the frozen 0.11 release challenge
([#3804](https://github.com/EffortlessMetrics/ripr-swarm/issues/3804)). It
carries the source-side freeze (EffortlessMetrics/ripr#1675) plus the three
real `should_limit` rows added under
[#3805](https://github.com/EffortlessMetrics/ripr-swarm/issues/3805), with the
committed diff captures under `diffs/release_*.diff`.
`cargo xtask check-release-challenge-selection` (also run by precommit)
validates it and reports the acceptance floors without lowering them.

- Every selected repository has one `repository_scopes` record naming the exact
  commit subjects, allowed read-only operations, network and retention policy,
  bounds, actor, and expiry. `proposed_unauthorized` is the state until the
  owner grants the scope; `authorized` requires an https link to that grant. Rows in an
  unauthorized repository stay selected but are not runnable.
- A `should_limit` row names a registered product `StaticLimitKind`, expects a
  conservative class, routes no repair, credits no aligned observer, and states
  the missing edge. Other directions carry no limit kind.
- New rows are `selected_unjudged`: labels null and no judgment provenance.
  Adjudication belongs to
  [#3806](https://github.com/EffortlessMetrics/ripr-swarm/issues/3806).
- `production_like_targets` names files the replay must opt in through
  `[analysis] production_like_targets` because the default source-role policy
  skips them (the `xtask/` cases). Each entry must be a file the frozen diff
  changes.
- The same behavior under two ids fails; row slices of one capture may share an
  anchor only with different directions.

## Release-challenge judgments

`release-judgments.json` is the independent adjudication of every frozen row
([#3806](https://github.com/EffortlessMetrics/ripr-swarm/issues/3806)).
`cargo xtask check-release-challenge-judgments` (also run by precommit)
validates it against the selection bytes it names.

- `selection_sha256` binds the packet to the exact selection; a stale digest
  rejects before any row is parsed or read.
- Every selection row has exactly one judgment, and `expected_direction` is
  copied, not re-decided. `terminal` is one of `confirmed_should_gap`,
  `confirmed_should_stay_quiet`, `confirmed_should_limit`,
  `inconclusive_missing_evidence`, `inconclusive_disagreement` or
  `invalid_case_identity`; a confirmed terminal that differs from the expected
  direction is a departure, not an error.
- Limit rows, gap rows, departures and disputed rows need two independent
  declared roles. A disagreement stays recorded; a confirmed terminal needs its
  cited resolution, and a resolved disagreement cannot stay inconclusive.
- Each review verdict (`discriminated`, `no_production_behavior`,
  `weakly_discriminated`, `not_discriminated`, `limited`) supports one
  direction. Reviews that support different directions, or not the confirmed
  one, must carry a recorded disagreement.
- `reference_outcome` labels compare with one named analyzer run, not the #1609
  candidate. `false_actionable` and `false_exposed` are mutually exclusive,
  `false_exposed` can be true only on a confirmed gap or limit row, and
  `false_actionable` only on a confirmed quiet or limit row;
  `under_credit` and `limitation_correct` are separate observations; `null`
  means not established. Inconclusive rows carry no labels.
- Judgments bind to the anchored behavior (file, line, expression). The
  historical probe ids in the selection reasons are evidence of what was
  selected, not identities the current analyzer reproduces.

## Rolling production-quiet and actionability observation

`rolling-observation.json` is the #4578 extension of this same panel. It does
not replace the seed, the portable packets, or the frozen #3806 judgments.

`cargo xtask rust-judged-panel check` (and the precommit alias) now also
validates that rolling packet against the retained subjects:

- Production quiet, gap, and limitation coverage is filled from the authorized
  seed production subjects. The three #3806 test-only quiet controls remain
  `test_only_quiet` and cannot occupy the production-quiet row.
- Classification is observed from the check-JSON findings already projected
  into the portable packets.
- Canonical repair actionability is observed only from a non-blocked
  `gap_decision_ledger`. A missing adapter, missing ledger, blocked ledger, or
  governed-manifest expected label is `not_observed` with a precise cause. It
  is not `no_action` and not `false_actionable=false`.
- Unauthorized real-repository production-quiet replay stays named as an unmet
  row rather than invented.

The CLI accepts `check`, `replay`, `packet`, `calibrate`, and `feedback`. Bounded real
ledger replay and live mutation campaigns remain outside the routine PR path.
`calibrate --check` (and `rust-judged-panel check`) re-derives the retained
scorecard from the #3806 judgments plus any exact receipts. Runtime results
cannot rewrite structural judgment bytes.

## Runtime calibration scorecard

`calibration-scorecard.json` and `calibration-scorecard.md` are the #4795
join of independently judged rows to exact runtime receipts.

`cargo xtask rust-judged-panel calibrate` writes a fresh JSON/Markdown pair
under `target/ripr/rust-judged-panel/calibration`. `--check` compares the
retained files to that derivation.

Every judged row has an explicit calibration eligibility and a terminal
runtime disposition from:

```text
caught
survived
inconclusive
equivalent_or_unusable
not_run
instrument_failure
stale_or_wrong_subject
```

Current release-challenge repositories remain `proposed_unauthorized`, so the
retained scorecard records `ineligible_unauthorized` / `not_run` rather than
inventing caught or survived labels from the qualitative #3806 mutation
reviews. Those reviews are not exact receipts (no runner hash, selector
identity, or executed-subject counts). A later authorized receipt can join
without rewriting the judgment packet.

Rates always show numerator and denominator. No denominator is
`not_measurable`, not a fake zero percent. Survived mutants stay visible
without an automatic false-exposed conclusion. #3076 route-yield and #4578
rolling-observation denominators are bound by identity and never merged.

## Analyzer feedback ledger

`feedback-ledger.json` is the #4796 sidecar over the same frozen #3806
judgments. It does not replace the seed, packets, rolling observation, or
independent judgments, and it does not absorb #4795 calibration.

`cargo xtask rust-judged-panel check` also validates that ledger:

- Every terminal judged case has one failure-direction disposition derived
  from immutable labels. Human notes cannot strengthen inconclusive or
  accepted-limitation rows.
- Confirmed analyzer defects stay `replay_only` with a named
  materialization/authorization boundary unless a producer-path fixture can
  retain the exact mechanism. Fixture ids must not be the case id, and
  `expected_class` shortcuts are rejected.
- A merged repair without original-case replay remains
  `repaired_pending_replay`. Wrong-target rows cannot close on a nearby
  observer identity.
- Runtime calibration is recorded as `not_run` while the landed #4795
  receipts stay unauthorized (`proposed_unauthorized`). Those results cannot
  set the static class.
- Owner search receipts are recorded. The ledger does not create, assign,
  close, or label GitHub objects.

`cargo xtask rust-judged-panel feedback [--out] [--check]` derives JSON and
Markdown from one DTO. There is still no accuracy `report` command and no
overall analyzer score.

## Item contract

Each `items[]` row carries:

- stable case identity;
- exact diff path and behavior anchor;
- one of the three expected directions;
- behavior family, required discriminator, and expected static class/limit;
- relation, oracle, activation, observer, propagation, and target dimensions;
- all-null adjudication labels;
- explicit runtime calibration status;
- non-claims and a load-bearing reason.

`false_actionable` and `false_exposed` remain separate. At most one may be true
for a completed case. `static_under_credit` is separately retained because a
conservative static gap can coexist with a caught runtime mutant without being
a false-actionable result.

## Selection and denominator rules

1. Every seed item is selected before judgment.
2. A difficult, unfavorable, unsupported, timeout, or inconclusive item remains
   in the denominator.
3. Null labels are not passes and do not support rates.
4. Structural judgment must cite exact owner, activation, path, sink, and
   observer evidence rather than copying RIPR output.
5. Runtime mutation calibration is a separate fact. It may challenge the
   static judgment but never silently rewrites it.
6. A rate always names its numerator, denominator, excluded/inconclusive count,
   case coverage, and exact as-of identities.
7. The panel never becomes a default gate, badge, RIPR Zero input, or support
   claim without a later explicit promotion decision.

## Seed coverage

The initial selected cases are:

1. **Boundary should gap** — direct owner tests have exact return assertions but
   omit the equality input that distinguishes `>=` from `>`.
2. **Boundary should stay quiet** — the exact equality input and aligned return
   assertion are present; repair routing would be false-actionable.
3. **Macro reach should limit** — a macro witness suggests a test path, but the
   expanded owner-to-observer path is unresolved; crediting exposed would be a
   silent over-promotion.

These cases establish the schema shape only. They are not representative error
rates.

## Next slices

```text
materialize exact replay repositories and analysis identities
→ retain RIPR outputs and bounded evidence packets
→ record independent structural judgments
→ resolve disagreements visibly
→ add exact targeted mutation receipts where safe
→ emit stratified confusion/agreement reports   (#4795, this slice)
→ turn confirmed failure families into permanent analyzer fixtures (#4796)
```

## Boundaries

No analyzer change, automated judgment, mutation execution, provider call,
generated test, repair assignment, gate, or support-tier promotion is made by
this seed.

Additional stated boundaries (audit #3297, see `AUDIT-3198.md`):

- Retained attestations are **reviewed, unsigned** authority. They
  establish reviewed coherence, not cryptographic authenticity; a
  coherent full-authority rewrite is rejected by the digest chain, and
  authenticity beyond review remains out of scope.
- **Windows junction/reparse-point confinement is `NOT_ESTABLISHED`.**
  The symlink-escape discriminators run only where symlink support
  exists; no junction- or reparse-point proof has been produced on any
  host. This is a stated boundary, not a claim.
