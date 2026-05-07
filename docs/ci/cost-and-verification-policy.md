# Cost and Verification Policy

`ripr` exists because ordinary CI signals are not enough for agentic
development. Coverage says code executed. Mutation testing says tests kill
concrete mutants. `ripr` asks the cheaper draft-time question:

> Does the changed behavior appear exposed to a meaningful test discriminator?

This document is the doctrinal anchor for how this repository spends CI
minutes. It is the reference implementation for the wider Effortless Metrics
verification-economics rollout.

## Doctrine

The goal is **not** lighter verification. The goal is **stronger,
better-scoped verification per CI minute**.

`ripr` itself is the key economic mechanism: it gives mutation-testing-lite
value at static-analysis prices. It does not run mutants. It does not claim
killed/survived outcomes. It asks whether changed behavior appears exposed to
a meaningful test discriminator.

Use this model:

- Rust makes local checks fast.
- Clippy catches known bad local code shapes.
- Semantic TOML allowlists make exceptions reviewable.
- `ripr` exposes oracle gaps cheaply.
- LEM budgeting makes CI cost visible.
- CI routing spends expensive lanes only where they buy proof.

## Hard rules

- Do not weaken the existing `ripr` product contract.
- Do not make `ripr` findings blocking until advisory data exists.
- Do not add Clippy test carveouts.
- Do not add bare `#[allow(...)]`.
- Keep `unsafe_code = "forbid"` unless a dedicated unsafe-island PR is
  explicitly justified.
- Keep runtime mutation testing as calibration / deep validation, not the
  ordinary PR default.
- Do not hard-enforce learned LEM budgets before `ci-actuals.json` data
  exists.

## Cheaper-signal-first decision order

When designing a CI lane, prefer cheaper signals first and only escalate when
the cheaper signal is provably insufficient:

1. Static type / syntax check (`cargo check`).
2. Clippy and workspace-level deny lists.
3. Policy TOML allowlists (no-panic, file policy, dependency, network).
4. Fast unit tests.
5. `ripr` static exposure analysis on the diff.
6. Integration tests / cli smoke / fixture goldens.
7. VS Code extension build and e2e.
8. Deep validation: mutation calibration, coverage, future-Clippy scan.
9. Release-only validation: package list, publish dry-run, marketplace
   publish dry-run.

Each step buys a different proof. Skipping a step requires a recorded
exception. Adding a step requires a recorded LEM impact.

## Related documents

- `docs/ci/lem-budgeting.md` — LEM definition and budget bands.
- `docs/ci/labels.md` — label semantics for routing and overrides.
- `docs/ci/ripr-ci-rollout.md` — how this rollout is staged in PRs.
- `docs/RIPR_EVIDENCE_POLICY.md` — `ripr`-specific evidence posture.
