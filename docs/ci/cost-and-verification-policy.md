# Cost and Verification Policy

## The verification-economics framing

We are not reducing CI because we want less verification.

We are reducing wasted CI so we can afford more verification, more often, at
agentic development volume.

Public cost evidence from other Rust-heavy open-source projects (directionally,
roughly $20/commit in hosted CI on some Blacksmith-class infrastructure) shows
that verification demand is rising faster than verification efficiency. We do not
read that as "those projects test too much." We read it as evidence that the
cost curve needs to change.

`ripr` exists to change that cost curve: mutation-testing-lite value at
static-analysis prices. It does not run mutants and does not report `killed` or
`survived` outcomes. It asks the mutation-testing-shaped question earlier and
cheaper: would the current tests appear to notice if the changed behavior were
wrong?

This framing has two consequences:

1. **`ripr` must be fast and cheap to run**, or the premise collapses. Every CI
   lane that runs `ripr` must be lean enough that running it often is obviously
   worthwhile compared to finding a test gap only after a mutation run.
2. **`ripr` must be the reference for disciplined CI posture**. If the repo that
   produces the verification-economics tool runs $1/commit CI for its own
   correctness, the argument fails. `ripr`'s CI should be a demonstration, not a
   counter-example.

## CI posture taxonomy

| Posture | Purpose | Default behavior |
| --- | --- | --- |
| Required | Cheap merge-safety and policy invariants. | Blocking on relevant PRs. |
| Advisory | Evidence that helps review but should not block routine work until calibrated. | Upload artifacts; do not fail PRs by default. |
| On-demand / release | Expensive or release-bearing proof. | Run on `main`, manual dispatch, or override labels. |

## What is not acceptable

- Gating ordinary Rust PRs behind VS Code extension e2e tests.
- Running release-surface proof (package dry-run, VSIX) on every PR.
- Enabling a soft gate before advisory data exists.
- Enforcing learned budgets before `ci-actuals.json` has accumulated history.
- Treating `ripr` findings as blocking before calibration demonstrates the
  gate has acceptable false-positive rate.

## Relationship to LEM

See `docs/ci/lem-budgeting.md` for the Local Evidence Minutes planning unit and
band definitions.

## Relationship to the soft gate

See `docs/ci/ripr-soft-gate.md` for when and how `ripr` findings become
acknowledgeable gates.
