# Handoff: Campaign 13 Closeout

Date: 2026-05-08
Branch / PR: `campaign-pr-review-guidance-closeout` / pending
Latest merged PR: #534 `docs: document PR review guidance`

## Current Work Item

`campaign/pr-review-guidance-closeout`

Campaign 13 made the PR-facing surface match the existing editor, CLI, agent,
and CI evidence loop:

```text
changed seam -> focused test intent -> verification command -> review artifact
```

The campaign did not add analyzer behavior, LSP feature expansion, inline PR
comments by default, automatic edits, generated tests, runtime mutation
execution, CI blocking by default, public crate splits, SARIF schema changes,
or badge schema changes.

## Prompt-To-Artifact Audit

| Requirement | Evidence |
| --- | --- |
| PR guidance has a bounded contract | `campaign/pr-review-guidance-audit` opened Campaign 13 around RIPR-SPEC-0012 and the existing static evidence loop. |
| A pure report producer exists | `review/pr-guidance-renderer` added `ripr review-comments --root . --base <sha> --head <sha> --out target/ripr/review/comments.json` plus adjacent Markdown without posting to GitHub. |
| Generated CI consumes the report | `ci/run-pr-guidance-report` made generated GitHub workflows run `ripr review-comments` on pull requests before the existing advisory summary and changed-line check-annotation steps. |
| Placement and suppression behavior is pinned | `fixtures/pr-guidance-cases` added exact-line, owner-function-line, same-file-line, summary-only, capped, configured-off, and changed-test-skip fixture outputs. |
| Users have a dedicated guide | `docs/pr-review-guidance` added `docs/PR_REVIEW_GUIDANCE.md` covering the command, generated CI behavior, placement rules, reviewer loop, fixture matrix, and inline-comment opt-in boundary. |

## PR Chain

- #530 `campaign: open PR review guidance`
- #531 `review: add PR guidance renderer`
- #532 `ci: run PR guidance report`
- #533 `fixtures: pin PR guidance cases`
- #534 `docs: document PR review guidance`
- `campaign/pr-review-guidance-closeout`

## Verification Run

Closeout proof before opening this PR:

```bash
cargo xtask check-campaign
cargo xtask goals next
cargo xtask check-doc-index
cargo xtask markdown-links
cargo xtask check-static-language
cargo xtask check-capabilities
cargo xtask check-pr
git diff --check
```

The documentation and schema surfaces also ran:

```bash
cargo xtask check-traceability
cargo xtask check-spec-format
cargo xtask check-output-contracts
```

## Next Work Item

No ready work item remains in `.ripr/goals/active.toml`.

Choose the next product campaign explicitly before adding new behavior. Do not
reopen Campaign 13 unless user feedback shows the bounded PR guidance contract
itself is wrong.

## What Not To Do

- Do not turn PR guidance into a free-form LLM reviewer.
- Do not post inline PR review comments from generated workflows by default.
- Do not place review guidance on unchanged or unrelated lines.
- Do not add automatic source edits, generated tests, or runtime mutation
  execution under this closeout.
- Do not make generated CI blocking by default.
- Do not broaden SARIF, badge, or public crate surfaces as part of PR guidance
  maintenance.
