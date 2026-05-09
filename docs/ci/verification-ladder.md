# Verification Ladder

The verification ladder describes the spectrum of test-oracle strength available to a Rust
project, from no tests to full proof. `ripr` occupies a specific rung on this ladder—one
that is cheap enough to run on every PR but gives structurally meaningful evidence.

## The Ladder

```
Level 0  No tests
          ↓
Level 1  Compilation only (cargo check / clippy)
          ↓
Level 2  Unit tests (happy-path only, no edge cases)
          ↓
Level 3  Unit + integration tests with coverage
          ↓
Level 4  Static RIPR exposure analysis  ← ripr lives here
          ↓
Level 5  Mutation testing (actual test kills)
          ↓
Level 6  Property-based testing (auto-generated inputs)
          ↓
Level 7  Formal verification / proof
```

## Where `ripr` Fits

`ripr` is a **static** RIPR exposure analyzer. Given a diff, it asks:

> For the behavior changed in this diff, do the current tests appear to contain a discriminator
> that would notice if that behavior were wrong?

This is the mutation-testing-shaped question, answered at static-analysis prices.

The key structural insight is that mutation testing is expensive because it *runs* mutants.
`ripr` never runs anything—it reasons over the diff and the test graph to estimate whether
a mutation in the changed code would be *observable* by the current tests.

Findings use conservative static vocabulary:

| Finding | Meaning |
|---------|---------|
| `exposed` | A test appears to reach and observe the changed behavior |
| `weakly_exposed` | A test reaches the behavior but the oracle appears indirect |
| `reachable_unrevealed` | The diff is reachable from a test but no discriminator is visible |
| `no_static_path` | No static path from any test to the changed code |
| `infection_unknown` | Static analysis cannot determine whether a mutation would infect state |
| `propagation_unknown` | Static analysis cannot determine whether infection propagates |
| `static_unknown` | The analyzer cannot form a judgment |

`ripr` never reports `killed`, `survived`, `untested`, or `proven`. Those belong to
runtime mutation testing.

## Rollout Doctrine: Advisory First

The rollout plan enforces a strict ordering:

1. **Advisory phase** (PR 14): `ripr` runs on production Rust diffs, uploads SARIF and
   Markdown artifacts, writes a step summary. No gate. No blocking.

2. **Calibration window**: At least 2 weeks of `ci-actuals.json` data for the `ripr` lane.
   The distribution of finding rates is observed. False-positive patterns are documented.

3. **Soft gate** (PR 17): `ripr` becomes an acknowledgeable gate—soft, scoped, and
   calibrated. It blocks only on `new reachable_unrevealed or weakly_exposed findings on
   production Rust with no nearby test change and no suppression`. It never blocks on
   `static_unknown`, `no_static_path`, or baseline (pre-existing) findings.

Skipping the advisory phase would mean setting a threshold before knowing the distribution.
That is premature optimization and the rollout hard rules prohibit it.

## Cost vs. Ladder Level

| Level | Cost per PR | `ripr` replaces? |
|-------|------------|-----------------|
| Compilation only | < $0.01 | No |
| Unit tests | $0.01–$0.10 | No |
| Unit + integration + coverage | $0.20–$1.00 | No |
| **Static RIPR (ripr)** | **$0.01–$0.05** | Screens before mutation |
| Mutation testing | $5–$100+ | No — downstream of `ripr` |

`ripr` is most economically useful as a **pre-filter**: run it on every PR to surface
likely oracle gaps cheaply, then reserve mutation testing for the flagged areas.

## Self-Dogfood

`ripr` runs `ripr` on `ripr`. Every production Rust diff in this repository goes through the
same analysis that contributors would use on their own repos. This proves the tool works,
surfaces real gaps in the `ripr` test suite, and demonstrates the cost and output format.

Self-dogfood is advisory (PR 14). The soft gate follows after calibration (PR 17).

## See Also

- [`docs/STATIC_EXPOSURE_MODEL.md`](../STATIC_EXPOSURE_MODEL.md) — RIPR model reference
- [`docs/DOGFOODING.md`](../DOGFOODING.md) — self-dogfood workflow
- [`docs/ci/cost-and-verification-policy.md`](cost-and-verification-policy.md) — economics framing
- [`docs/ci/ripr-soft-gate.md`](ripr-soft-gate.md) — soft gate doctrine
