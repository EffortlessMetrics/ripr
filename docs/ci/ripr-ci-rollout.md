# `ripr` CI Rollout

This is the staged rollout that makes `ripr` the reference implementation
for the wider Effortless Metrics verification-economics policy. Each stage
is its own PR. Stages are independent except where noted.

## Stages

| PR | Subject                                                   | Risk |
| -: | --------------------------------------------------------- | ---- |
| 01 | CI economics docs                                         | none |
| 02 | CI lane whitelist + LEM budget ledgers (TOML, no code)    | none |
| 03 | CI lane whitelist lint (xtask command)                    | low  |
| 04 | Non-Rust file allowlist migration to TOML                 | low  |
| 05 | No-panic allowlist schema upgrade to 0.3                  | low  |
| 06 | Clippy ledger cleanup, planned-flip tracking              | none |
| 07 | LEM-aware PR Plan (advisory)                              | low  |
| 08 | Cache + concurrency cleanup                               | low  |
| 09 | Split default vs. deep / release lanes                    | med  |
| 10 | `ripr` self-dogfood advisory                              | low  |
| 11 | CI actuals + structured test telemetry                    | low  |
| 12 | Soft budget guard                                         | med  |
| 13 | Future Clippy lane (advisory)                             | low  |
| 14 | `ripr` soft-gate policy (after calibration data exists)   | med  |

## Operating principles

- Do not create one mega PR.
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

## Final target

When this rollout is complete, `ripr` should be the repo other Rust repos
copy:

```text
MSRV 1.93
strict Clippy profile
no test carveouts
semantic no-panic TOML allowlist
TOML non-Rust file policy
CI lane whitelist
LEM-aware PR Plan
cache save-only-main
one required summary gate
ripr self-dogfood advisory
ci-actuals telemetry
soft budget guard
future Clippy upgrade ledger
```

Any deviation from this target needs a recorded ADR or learning entry.
