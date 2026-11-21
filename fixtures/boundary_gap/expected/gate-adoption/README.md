# Gate adoption dogfood receipts

These fixtures pin Campaign 16 gate adoption receipts over the boundary-gap
PR guidance corpus. They are used by `cargo xtask dogfood` to prove the visible
adoption path from repo-local evidence:

| Case | Mode | Purpose |
| --- | --- | --- |
| `visible-only` | `visible-only` | Default policy visibility records evidence without blocking. |
| `acknowledged` | `acknowledgeable` | `ripr-waive` remains visible as an acknowledged decision. |
| `baseline-aware` | `baseline-check` | A reviewed baseline identity stays visible and non-blocking. |
| `baseline-new-gap` | `baseline-check` | A new policy-eligible identity blocks in explicit baseline-check mode. |
| `calibrated-gate` | `calibrated-gate` | Explicit calibrated mode blocks a new supported candidate. |
| `missing-baseline-config` | `baseline-check` | Missing required baseline input is reported as a repair-oriented config error. |

The fixtures do not change generated CI defaults. Repositories still opt into
gate evaluation by setting `RIPR_GATE_MODE`.

Policy-eligible advisory, acknowledged, and blocking receipts carry the exact
structured repair route directly in `gate-decision.{json,md}`. The checked
blocking dogfood test reads that gate output—not the PR-guidance input—and
requires gap/seam identity, changed behavior, missing discriminator, test target
and intent, verify/receipt commands, the producer-owned inspection command, and
the static authority boundary. It rejects `gh run download` or a PR-guidance
lookup as a remediation step.
