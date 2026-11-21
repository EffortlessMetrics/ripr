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
→ emit stratified confusion/agreement reports
→ turn confirmed failure families into permanent analyzer fixtures
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
