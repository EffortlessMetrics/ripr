# First Successful PR Dogfood Receipts

Date: 2026-05-16

Campaign: First-Run UX and Adoption Hardening

Work item: `dogfood(ux): record first successful PR receipts`

## Scope

This receipt records the checked first successful PR start-here corpus in
`cargo xtask dogfood`. The dogfood report now includes a dedicated first-run
section that validates checked `start-here.{json,md}` outputs from
`fixtures/first_successful_pr/`.

The checked cases are:

| Case | Expected status | Expected state | Repair route |
| --- | --- | --- | --- |
| `boundary-gap` | `actionable` | `top_gap` | focused Rust test |
| `output-contract-gap` | `actionable` | `top_gap` | output or golden proof |
| `empty-diff` | `no_action` | `empty_diff` | no interruption |
| `blocked-ledger` | `blocked` | `blocked_artifact` | regenerate gap ledger |

## What Shipped

- `cargo xtask dogfood` reads the first successful PR corpus manifest.
- Dogfood validates every expected `start-here.json` and `start-here.md`.
- Actionable first-run receipts must name a verify command.
- Blocked first-run receipts must name a next regeneration command.
- The generated dogfood Markdown includes a first successful PR receipt table.
- The generated dogfood JSON includes a `first_successful_pr` section with
  case status, state, top gap, verify command, next command, and errors.

## Validation

```bash
cargo fmt
cargo test -p xtask dogfood_first_pr --bin xtask
cargo test -p xtask dogfood_reports_are_advisory --bin xtask
cargo xtask dogfood
```

Result: pass.

`cargo xtask dogfood` wrote advisory receipts to:

```text
target/ripr/reports/dogfood.json
target/ripr/reports/dogfood.md
```

The generated report recorded:

```text
boundary-gap: actionable / top_gap / MissingBoundaryAssertion
output-contract-gap: actionable / top_gap / MissingOutputContract
empty-diff: no_action / empty_diff / none
blocked-ledger: blocked / blocked_artifact / next command present
```

## Evidence

| Evidence | Path |
| --- | --- |
| Corpus manifest | `fixtures/first_successful_pr/corpus.json` |
| Checked fixture outputs | `fixtures/first_successful_pr/<case>/expected/start-here.{json,md}` |
| Dogfood implementation | `xtask/src/main.rs` |
| Generated receipt packet | `target/ripr/reports/dogfood.{json,md}` |
| Spec traceability | `.ripr/traceability.toml` |

## Non-Goals

- No analyzer behavior change.
- No hidden analysis rerun.
- No source edits or generated tests.
- No provider calls.
- No mutation execution.
- No gate, badge, policy, branch protection, or default CI blocking change.
- No preview-language promotion.

## Deferred Work

- Polish PR repair-card wording.
- Add the editor `Start Current Repair` orchestration command.
- Make agent repair packets directly pasteable.
- Add advisory first-run CI projection.
- Compress README and quickstart around the first repair loop.

## Next Lane

Continue the First-Run UX and Adoption Hardening campaign with:

```text
comments(ux): polish repair-card wording
```
