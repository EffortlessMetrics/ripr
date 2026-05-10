# ripr / Mutation Testing Boundary

This document records the deliberate boundary between what `ripr` does and what mutation
testing does. Both are verification tools. They answer different questions at different cost
points and belong in different lanes.

## What ripr does

`ripr` is a **static** RIPR (Reach-Infect-Propagate-Observe-Discriminate) exposure analyzer.
It answers one PR-time question:

> For the behavior changed in this diff, do the current tests appear to contain a discriminator
> that would notice if that behavior were wrong?

It does not run tests. It does not execute mutants. It does not report `killed` or `survived`.
It reads the diff and the test suite statically and asks whether the static structure of the
tests suggests a discriminating oracle exists.

This is mutation-testing-lite value at static-analysis prices.

## What mutation testing does

Mutation testing instruments the production source, runs tests against each mutant, and reports
whether each mutant was killed (a test failed) or survived (all tests passed despite the
behavioral change).

Mutation testing gives runtime evidence of test discriminator adequacy. It is authoritative
but expensive — typical mutation matrices cost orders of magnitude more CI time than static
analysis.

## Why the boundary matters

`ripr` exists to change the cost curve: find exposure gaps earlier and cheaper. If `ripr`
starts claiming mutation-testing-grade certainty, it misleads. If mutation testing is run as
a default PR gate, CI becomes too expensive to run at agentic development volume.

The boundary is enforced in two ways:

1. **Output language**: `cargo xtask check-static-language` rejects `killed`, `survived`,
   `untested`, `proven`, and `adequate` from static output. These terms belong to runtime
   mutation testing, not `ripr`.
2. **Lane placement**: Mutation testing belongs in targeted, nightly, and release lanes — not
   in the default PR lane.

## Lane placement doctrine

```text
Default PR:   ripr static exposure filter (advisory; not a blocking gate until calibrated)
Targeted PR:  mutation for changed analyzer/gate/report owner surface (scoped)
Nightly:      full mutation matrix
Release:      mutation/readiness evidence clean enough to ship
```

`ripr` runs cheaply on every PR. Full mutation runs where risk justifies the cost.

## Terminology boundary

| Term | Where it belongs | `ripr` static output? |
| --- | --- | --- |
| `exposed` | ripr static output | yes |
| `weakly_exposed` | ripr static output | yes |
| `reachable_unrevealed` | ripr static output | yes |
| `no_static_path` | ripr static output | yes |
| `infection_unknown` | ripr static output | yes |
| `propagation_unknown` | ripr static output | yes |
| `static_unknown` | ripr static output | yes |
| `killed` | runtime mutation testing | **no** |
| `survived` | runtime mutation testing | **no** |
| `untested` | runtime mutation testing | **no** |
| `proven` | formal verification | **no** |
| `adequate` | mutation adequacy research | **no** |

## Calibration path

`ripr` findings become calibrated gate material only after:

1. Advisory data has accumulated to establish false-positive rate.
2. The soft gate threshold is tuned against that data.
3. A `ripr-waive` label flow is available for false positives.

Until that calibration exists, `ripr` findings are advisory and do not block PRs.

See `docs/ci/ripr-soft-gate.md` for the soft gate calibration path.

## See also

- [`docs/ci/test-evidence-lanes.md`](test-evidence-lanes.md) — full lane split doctrine.
- [`docs/ci/rust-1.95-quality-rollout.md`](rust-1.95-quality-rollout.md) — 0.5.1 rollout anchor.
- [`docs/STATIC_EXPOSURE_MODEL.md`](../STATIC_EXPOSURE_MODEL.md) — RIPR evidence model.
- [`docs/ci/cost-and-verification-policy.md`](cost-and-verification-policy.md) — verification economics.
- [`docs/ci/ripr-soft-gate.md`](ripr-soft-gate.md) — soft gate calibration path.
