# `ripr` Evidence Policy

`ripr` runs against its own diffs as part of CI. This document defines what
`ripr` evidence in `ripr` itself is allowed to claim, how that evidence is
surfaced, and when it can become blocking.

## The product contract this protects

> For the behavior changed in this diff, do the current tests appear to
> contain a discriminator that would notice if that behavior were wrong?

`ripr` is a **static** RIPR (Reach-Infect-Propagate-Observe-Discriminate)
exposure analyzer. It does **not** run mutants. It does **not** claim
killed/survived outcomes.

## Allowed static language in `ripr` output

Findings in `ripr` output use conservative static language only:

- `exposed`
- `weakly_exposed`
- `reachable_unrevealed`
- `no_static_path`
- `infection_unknown`
- `propagation_unknown`
- `static_unknown`

The following terms are **forbidden** in static output: `killed`, `survived`,
`untested`, `proven`, `adequate`. They belong to runtime mutation testing,
not `ripr`.

This is enforced by `cargo xtask check-static-language`.

## Posture in CI

| Stage                                | Posture                |
| ------------------------------------ | ---------------------- |
| Initial self-dogfood (PR 10)         | advisory, non-blocking |
| With telemetry and budget data       | advisory               |
| Soft-gate (PR 14, after calibration) | acknowledgeable gate   |

`ripr` findings start advisory. They become acknowledgeable only after
`ci-actuals.json` data and per-finding stability is established.

## Soft-gate scope (post calibration)

The soft-gate is intentionally narrow. It triggers only when **all** of the
following are true:

- Finding class is `reachable_unrevealed` or `weakly_exposed`.
- Production Rust changed in this PR.
- No nearby test changed.
- The finding is not suppressed in `.ripr/suppressions.toml` (the
  canonical path used by `crates/ripr/src/config.rs`).
- The finding's numeric `confidence` field (documented in
  `docs/OUTPUT_SCHEMA.md`) clears the gate threshold. The threshold
  itself is intentionally not pinned here — it is decided in PR 14
  alongside the soft-gate implementation.

The soft-gate does not block on:

- Baseline (pre-existing) findings.
- `static_unknown` findings.
- Mutation outcomes (`killed` / `survived` are not produced by `ripr`).

Acknowledgement labels: `ripr-waive`, `ci-budget-ack`. The routing label
`full-ci` (documented in `docs/ci/labels.md`) is also recognized — it
adds the deep-validation lanes and as a side effect demotes `ripr-waive`
requirement on this gate.

## Suppression schema

Suppressions live in `.ripr/suppressions.toml` (the canonical path used by
`crates/ripr/src/config.rs`; the parser is in
`crates/ripr/src/output/suppressions.rs`; see `docs/CONFIGURATION.md`):

Two `kind` values are supported. Each has a different required
selector field — they are mutually exclusive.

```toml
# kind = "exposure_gap": finding_id is required; test/path are not used.
[[suppressions]]
kind = "exposure_gap"
finding_id = "<finding id>"
owner = "core/analysis"
reason = "Spec-required behavior is exposed indirectly via integration test."
expires = "2026-09-01"          # optional ISO-8601 YYYY-MM-DD

# kind = "test_efficiency": test is required; path is optional narrowing.
[[suppressions]]
kind = "test_efficiency"
test = "<test selector>"
path = "<optional path narrowing>"
owner = "core/analysis"
reason = "Test marked low-efficiency intentionally; reviewed."
expires = "2026-09-01"          # optional ISO-8601 YYYY-MM-DD
```

Every entry requires `owner` and `reason`. `expires` is optional;
**expired suppressions surface as warnings on the badge** rather than
failing CI hard, matching the badge policy in `docs/CONFIGURATION.md`
and `docs/IMPLEMENTATION_CAMPAIGNS.md` (the `suppressions/v1` campaign:
"Expired entries do not apply and surface as warnings — silent
green-forever debt is impossible"). Suppressions are reviewed at every
release readiness check.

## Reports and artifacts

The self-dogfood lane writes:

- `target/ripr/reports/ripr-diff.json`
- `target/ripr/reports/ripr-diff.sarif` (best-effort)
- `target/ripr/reports/index.md`

Artifacts upload on every run regardless of pass/fail. The PR step summary
links the artifact and prints a finding count.

## What `ripr` evidence does not replace

`ripr` is one rail in a layered evidence stack:

- Coverage proves execution.
- Mutation testing (calibration only) proves test-kill discriminators.
- Property tests prove invariants over input domains.
- Fixture goldens prove output stability.
- `ripr` proves that the changed behavior plausibly has somewhere to fail.

`ripr` does not replace any of the others. It compresses the
mutation-testing-shaped question into a static signal that is cheap enough
to run on every PR.
