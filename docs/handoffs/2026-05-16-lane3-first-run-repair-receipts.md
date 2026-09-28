# Lane 3 First-Run Repair Dogfood Receipts

Date: 2026-05-16

Lane: 3, Editor / LSP UX

Work item: `dogfood(lane3): record first-run repair receipts`

Branch / PR: `dogfood-first-run-repair-receipts` / #1038

Latest merged PRs:

- #1033 `fixtures(editor): add first-run usability fixtures`
- #1037 `docs(editor): write first-run-to-first-receipt guide`

## Scope

This receipt records the first-run-to-first-receipt editor path using existing
RIPR artifacts and fixture-backed dogfood checks. It proves the local repair
loop is explainable without changing analyzer behavior, editor behavior,
receipt production, policy, gates, generated CI, source files, generated tests,
providers, mutation execution, or PR comment publishing.

The user-facing loop is:

```text
install/open -> diagnose setup -> read status -> inspect one Rust gap
-> open related test or copy first repair packet -> verify
-> receipt -> refresh
```

## Proof Points

| Proof point | Evidence |
| --- | --- |
| Setup diagnosis | `cargo run -p ripr -- doctor --root .` passed. It reported missing config as the normal built-in default path, `enabled languages: rust`, and `cargo`/`rustc` 1.95.0 available. |
| First-run packet behavior | `cargo run -p ripr -- pilot --root . --out target/ripr/pilot` wrote `target/ripr/pilot/pilot-summary.{json,md}` with `status: partial` after the 30000 ms budget. |
| Budget retry behavior | `cargo run -p ripr -- pilot --root . --out target/ripr/pilot --mode draft --max-seams 5 --timeout-ms 120000` again wrote a safe partial packet and named the next larger retry command. |
| First actionable Rust gap | `cargo xtask dogfood` passed and the `boundary_gap` fixture recorded 1 finding with `weakly_exposed`; first-useful-action fixture `actionable/` routes to `write_focused_test`. |
| Verify movement | `fixtures/boundary_gap/expected/editor-agent-loop/agent-verify.json` records the boundary-gap seam before/after comparison, including `weakly_gripped -> weakly_gripped`, `change: unchanged`, a new observed value `100`, and related-test count +1. |
| Receipt emitted | `fixtures/boundary_gap/expected/editor-agent-loop/agent-receipt.json` records schema `0.3`, status `advisory`, movement `unchanged`, `safe_to_merge: false`, and static-artifact limits. |
| Refresh/editor projection | `cargo xtask lsp-cockpit-report` passed, proving the fixture-backed editor cockpit report still projects the checked editor surfaces. |
| No-action state | `cargo xtask dogfood` passed the first-useful-action `no-actionable-seam/` route, proving clean/no-action state remains explicit instead of silent. |

## Command Results

```bash
cargo run -p ripr -- doctor --root .
```

Result: pass.

Observed setup state:

```text
Config: not found; using built-in defaults
Enabled languages: rust
doctor checks passed
```

```bash
cargo run -p ripr -- pilot --root . --out target/ripr/pilot
```

Result: partial, fail-closed.

Observed packet state:

```text
Reason: analysis timed out after 30000 ms
Written:
  target/ripr/pilot/pilot-summary.json
  target/ripr/pilot/pilot-summary.md
Next:
  ripr pilot --root . --out target/ripr/pilot --mode draft --max-seams 5 --timeout-ms 120000
```

```bash
cargo run -p ripr -- pilot --root . --out target/ripr/pilot --mode draft --max-seams 5 --timeout-ms 120000
```

Result: partial, fail-closed.

Observed packet state:

```text
Reason: analysis timed out after 120000 ms
Written:
  target/ripr/pilot/pilot-summary.json
  target/ripr/pilot/pilot-summary.md
Next:
  ripr pilot --root . --out target/ripr/pilot --mode draft --max-seams 5 --timeout-ms 480000
```

```bash
cargo xtask lsp-cockpit-report
cargo xtask dogfood
```

Result: pass.

`cargo xtask dogfood` wrote advisory receipts to:

```text
target/ripr/reports/dogfood.json
target/ripr/reports/dogfood.md
```

The dogfood report recorded:

```text
boundary_gap: 1 finding, weakly_exposed: 1, errors: none
weak_error_oracle: 3 findings, weakly_exposed: 3, errors: none
```

It also checked first-useful-action receipt routes:

| Case | Status | Action | Static movement |
| --- | --- | --- | --- |
| `actionable` | `actionable` | `write_focused_test` | `unknown` |
| `baseline-only` | `baseline_only` | `acknowledge_baseline` | `unknown` |
| `stale` | `stale` | `refresh_evidence` | `unknown` |
| `missing-required-artifact` | `missing_required_artifact` | `generate_missing_artifact` | `unknown` |
| `unchanged-after-attempt` | `unchanged_after_attempt` | `revise_focused_test` | `unchanged` |
| `no-actionable-seam` | `no_actionable_seam` | `no_action` | `unknown` |

## Receipt Interpretation

The checked boundary-gap receipt intentionally records:

```text
weakly_gripped -> weakly_gripped
movement: unchanged
safe_to_merge: false
```

That is still useful first-run evidence. The after artifact records a new
observed value and related-test count increase, but the current static
classifier does not promote the seam to a stronger grip class. The correct next
action is to add or strengthen the missing discriminator named by the packet,
then rerun verify and receipt.

## Limits

- Static evidence only.
- No runtime adequacy claim.
- No mutation execution.
- No source edits.
- No generated tests.
- No provider or model calls.
- No PR comment publishing.
- No policy, gate, badge, baseline, waiver, or default-blocking change.
- No new receipt producer.
- No hidden editor analysis rerun.

## Next Work Item

`campaign(lane3): close editor first-run usability`

Close only after the campaign closeout names the landed PR chain, validation
commands, remaining first-run limitations, and confirms that setup diagnosis,
no-output status, receipt visibility, first repair packet, fixtures, docs, and
dogfood proof are all present.
