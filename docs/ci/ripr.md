# ripr PR lane

`ripr` is the repository's static mutation-exposure lane. It answers the PR-time
question from the product contract:

> For the behavior changed in this diff, do the current tests appear to contain
> a discriminator that would notice if that behavior were wrong?

This lane shifts mutation-shaped signal left. It looks for weak oracle exposure
statically before the repository spends runtime mutation budget.

## Claims

`ripr` may report conservative static exposure classifications:

- `exposed`
- `weakly_exposed`
- `reachable_unrevealed`
- `no_static_path`
- `infection_unknown`
- `propagation_unknown`
- `static_unknown`

It must not claim runtime mutation outcomes, proof of correctness, or general
test adequacy. Runtime mutation remains a separate backstop for selected risk
surfaces.

## Default placement

The intended PR lane is advisory by default:

```text
Default PR:   static mutation-exposure analysis for changed production Rust
Risk PR:      static analysis plus targeted runtime mutation when risk pays
Nightly/main: broader runtime mutation calibration and trend receipts
Release:      readiness receipts that include targeted runtime evidence
```

Soft gating is allowed only after calibration. The current soft-gate doctrine is
recorded in [`ripr-soft-gate.md`](ripr-soft-gate.md) and
`policy/ripr-soft-gate.toml`.

## Artifacts

The canonical PR packet should stay small and reviewable:

```text
target/ripr/pr/
  pr-summary.md
  repo-exposure.json
  review.md
  agent-packet.json
  first-useful-action.md
  first-useful-action.json
```

Large repository scans are build-heavy in this repository. Prefer summary
receipts, generated ledgers, and scoped reports for ordinary PRs. Run at most
one intentional full refresh at a time and remove ad-hoc large JSON outputs
after inspection.

## Relationship to other lanes

| Lane | Role |
| --- | --- |
| `ripr` | Static mutation-exposure signal for changed behavior. |
| `cargo-mutants` | Runtime mutation backstop where risk justifies cost. |
| Coverage / Codecov | Execution-surface telemetry; it does not establish discriminating oracles. |
| Focused tests | Local runtime evidence for the discriminator or behavior under review. |

## See also

- [`ripr-mutation-boundary.md`](ripr-mutation-boundary.md) — detailed language and mutation boundary.
- [`test-evidence-lanes.md`](test-evidence-lanes.md) — lane split across static, runtime, coverage, and release evidence.
- [`cost-and-verification-policy.md`](cost-and-verification-policy.md) — CI economics doctrine.
- [`lem-budgeting.md`](lem-budgeting.md) — LEM planning unit and budget bands.
