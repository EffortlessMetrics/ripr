# PR Inline Comment Publisher Dogfood Receipts

Date: 2026-05-10

Campaign: 26, PR Inline Comment Publisher

Work item: `dogfood/pr-inline-comment-publisher-receipts`

## Scope

This receipt records repo-local PR inline comment publisher cases checked by
`cargo xtask dogfood`. The checks read committed
`comment-publish-plan.{json,md}` fixture artifacts and verify mode, status,
publishable/skipped/blocked counts, safe-publish flags, operation vocabulary,
skip reasons, blocked reasons, default-off posture, and advisory limits.

The checks do not post real PR comments or call GitHub.

## Checked Plans

| Case | Mode | Purpose |
| --- | --- | --- |
| `publishable_changed_line` | `plan` | One changed-line comment is eligible for a create operation while posting stays disabled. |
| `summary_only_excluded` | `plan` | Summary-only guidance remains visible but never becomes an inline comment. |
| `cap_overflow` | `plan` | The default cap keeps only three comments publishable and skips overflow. |
| `dedupe_upsert` | `plan` | Existing dedupe keys produce update or keep operations instead of duplicates. |
| `stale_existing` | `plan` | Stale existing RIPR comments are recorded as review-only cleanup operations. |
| `fork_or_no_token` | `inline` | Inline mode visibly blocks when fork trust or token requirements are missing. |
| `missing_input` | `plan` | Missing PR guidance produces a repairable no-op plan. |

## Validation

```bash
cargo xtask dogfood
```

Result: pass.

The dogfood report writes advisory receipts to:

```text
target/ripr/reports/dogfood.json
target/ripr/reports/dogfood.md
```

## Limits

- Advisory publish-plan evidence only.
- No real PR comments posted.
- No hidden analysis rerun.
- No source edits or generated tests.
- No provider calls.
- No mutation execution.
- No recommendation ranking or gate-policy changes.
- No `pull_request_target`.
- No default inline comments or default CI blocking.
