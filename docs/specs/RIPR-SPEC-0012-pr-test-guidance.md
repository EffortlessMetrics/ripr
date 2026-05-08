# RIPR-SPEC-0012: PR Test Guidance Annotations

Status: proposed

## Problem

CI-first users should not have to download RIPR artifacts before they can see
the next useful test target. The generated GitHub workflow already produces
pilot, agent, cockpit, SARIF, badge, verify, and receipt artifacts. The missing
surface is a small PR-facing projection that points to changed code when RIPR's
static evidence says a focused test would be useful.

This spec defines that projection. RIPR decides which changed-code seams have
weak static test evidence. Humans and LLM agents receive a bounded brief that
explains why a focused test is useful and how to verify the result.

This started as the Campaign 12 design contract for implementation planning.
Campaign 13 now uses it as the implementation contract: the pure
`review-comments` renderer exists and generated workflows run it before the
summary and check-annotation consumers. The placement and suppression fixture
matrix exists, and dedicated PR guidance documentation now pins the user-facing
command, CI, placement, and inline-comment boundaries. Optional comment
publishing remains future work and must preserve the boundaries below.

## Product Contract

PR test guidance annotations are advisory review hints derived from existing
RIPR evidence. They must not become an LLM reviewer that freely inspects a diff
and decides what matters.

The contract is:

- RIPR selects the seam and changed line from static evidence.
- RIPR explains the missing discriminator or evidence stage when it can.
- RIPR provides the smallest useful test intent, assertion shape, related test,
  and agent brief command available from existing packets.
- The LLM handoff is bounded to one focused test and a verification command.
- GitHub annotations and summaries are advisory and non-blocking by default.

This contract is pinned before implementation so that generated workflow
posting behavior has a fixed boundary. The first implementation slice may add
the pure report renderer and generated workflow projection, but it must preserve
the defaults here: job summary and check annotations first, inline PR review
comments only by explicit opt-in, and no CI failure policy by default.

## Behavior

`ripr review-comments` is a read-only report command. It joins the pull-request
diff, repo exposure, agent brief fields, related-test evidence, changed
production files, changed test files, configured severity, and suppression
policy. It writes review-ready JSON and Markdown without posting to GitHub.

Generated CI should publish that report through the least intrusive useful
surfaces first: job summary and check annotations by default, optional inline PR
review comments only when explicitly enabled.

## Surfaces

The default GitHub surface is:

- GitHub job summary for the top recommendations;
- check annotations for line-level guidance when a safe changed-line placement
  exists;
- uploaded JSON and Markdown artifacts for reviewers and agents.

Inline PR review comments are opt-in. A generated or copied workflow may enable
them with an explicit setting such as:

```yaml
env:
  RIPR_PR_COMMENTS: "true"
```

Opt-in review comments need pull-request write permission and must upsert or
replace prior RIPR comments instead of posting duplicates.

## Selection Rules

Only produce PR guidance when all of these are true:

1. Production Rust changed in the pull request.
2. RIPR finds an actionable seam tied to the changed region or owning function.
3. No nearby test changed in the same pull request.
4. The seam is visible under configured severity and suppression policy.
5. The seam has enough guidance to make the annotation useful.

Default included seam classes:

- `weakly_gripped`
- `ungripped`
- `reachable_unrevealed`
- `activation_unknown`
- `propagation_unknown`
- `observation_unknown`
- `discrimination_unknown`

Skip by default:

- `strongly_gripped`
- `intentional`
- `suppressed`
- configured-off seams
- `opaque` seams when no useful test guidance is available

The highest-value comments are seams with a missing discriminator, changed
expression, related test target, suggested assertion shape, or candidate values.

## Ranking And Caps

Rank candidate comments by:

1. changed line is the seam line;
2. changed line is inside the seam owner function;
3. missing discriminator exists;
4. suggested assertion exists;
5. related test exists;
6. seam class priority: `weakly_gripped`, `ungripped`,
   `reachable_unrevealed`, then unknown-stage classes;
7. stable tie-break: path, line, seam ID.

Default caps:

- `max_inline_comments = 3`
- `max_summary_items = 10`

The summary may list more recommendations than inline comments, but it should
still stay bounded enough for a reviewer to scan in the GitHub UI.

## Placement Rules

GitHub review comments and check annotations should only point at changed lines.

Placement order:

1. exact changed seam line;
2. nearest changed line in the same owner function;
3. nearest changed line in the same file;
4. summary-only recommendation when no safe changed-line placement exists.

Do not force a line-level comment onto an unrelated changed line. Bad placement
is noisier than a summary-only recommendation.

## JSON Shape

The renderer writes:

```text
target/ripr/review/comments.json
target/ripr/review/comments.md
```

The JSON report uses schema version `0.1`:

```json
{
  "schema_version": "0.1",
  "tool": "ripr",
  "status": "advisory",
  "root": ".",
  "base": "origin/main",
  "head": "HEAD",
  "mode": "draft",
  "limits": {
    "max_inline_comments": 3,
    "max_summary_items": 10
  },
  "summary": {
    "comments": 2,
    "summary_only": 1,
    "suppressed": 1,
    "unchanged_tests": true
  },
  "comments": [
    {
      "id": "ripr-review-67fc764ba37d77bd",
      "seam_id": "67fc764ba37d77bd",
      "dedupe_key": "ripr:67fc764ba37d77bd:src/pricing.rs:88",
      "placement": {
        "path": "src/pricing.rs",
        "line": 88,
        "side": "RIGHT",
        "mode": "exact_seam_line"
      },
      "kind": "predicate_boundary",
      "grip_class": "weakly_gripped",
      "severity": "warning",
      "reason": "Related tests reach and observe the owner but miss the equality boundary.",
      "missing_discriminator": "amount == discount_threshold",
      "suggested_test": {
        "intent": "Add an equality-boundary test.",
        "candidate_values": ["amount == discount_threshold"],
        "assertion_shape": "Assert the returned discount behavior directly.",
        "assertion_kind": "exact_value",
        "recommended_file": "tests/pricing.rs",
        "recommended_name": "discounted_total_boundary",
        "near_test": "applies_discount_above_threshold"
      },
      "llm_guidance": {
        "prompt": "Write one focused Rust test for the missing equality boundary. Place it near tests/pricing.rs::applies_discount_above_threshold. Do not change production code. Preserve existing fixture style. Verify with ripr agent verify.",
        "command": "ripr agent brief --root . --seam-id 67fc764ba37d77bd --json > target/ripr/workflow/agent-brief.json",
        "verify_command": "ripr agent verify --root . --before target/ripr/workflow/before.repo-exposure.json --after target/ripr/workflow/after.repo-exposure.json --json"
      }
    }
  ],
  "summary_only": [],
  "suppressed": [],
  "warnings": [],
  "limits_note": "Advisory static evidence only; no automatic edits, generated tests, runtime mutation execution, or CI blocking."
}
```

## Field Contract

- `schema_version` - currently `"0.1"`.
- `status` - always `"advisory"`; this report is review guidance, not a CI
  policy.
- `root`, `base`, `head`, and `mode` - the workspace root, compared revisions,
  and RIPR analysis mode used to render the report.
- `limits.max_inline_comments` - default cap for changed-line annotations.
- `limits.max_summary_items` - default cap for total recommendations.
- `summary.comments` - count of line-placeable comments.
- `summary.summary_only` - count of recommendations without a safe changed-line
  placement.
- `summary.suppressed` - candidate recommendations hidden by configured
  severity, suppression, caps, or missing guidance.
- `summary.unchanged_tests` - `true` when the pull request did not change a
  nearby test for the selected recommendations.
- `comments[].id` - stable report-local ID.
- `comments[].dedupe_key` - stable key based on seam ID, path, and seam line.
- `comments[].placement` - GitHub-compatible changed-line placement.
- `comments[].placement.mode` - `"exact_seam_line"`,
  `"owner_function_changed_line"`, or `"same_file_changed_line"`.
- `comments[].reason` - short static-evidence explanation for why the focused
  test would be useful.
- `comments[].missing_discriminator` - missing value, branch, or observation
  when available.
- `comments[].suggested_test` - bounded test intent from the seam packet or
  related evidence.
- `comments[].llm_guidance` - bounded handoff prompt and command. It is not a
  request for free-form diff review.
- `summary_only[]` - same item shape without a `placement` object when the seam
  cannot be safely attached to a changed line.
- `suppressed[]` - bounded records for recommendations hidden by caps or nearby
  test changes.
- `warnings[]` - selection warnings from the agent brief selection path.
- `limits_note` - static-evidence boundary text for downstream summaries.

## GitHub Projection

Generated CI should publish this report in two default levels:

1. Append a concise Markdown summary to `$GITHUB_STEP_SUMMARY`.
2. Emit GitHub check annotations from `comments[]`.

Check annotations are the default line-level surface because they provide file
and line guidance without adding persistent review-thread noise.

A custom inline review-comment publisher may be added by a repository as an
explicit opt-in. Inline review comments must:

- post at most three comments by default;
- target only changed lines;
- use `dedupe_key` to update or replace prior RIPR comments;
- never invent placement for `summary_only[]` guidance;
- remain advisory and non-blocking.

## LLM Guidance Boundary

The prompt or command embedded in a comment must ask for one focused test. It
should include:

- seam kind;
- changed expression when known;
- missing discriminator;
- candidate values;
- assertion shape;
- recommended test file;
- related test to imitate;
- patterns to avoid when available;
- verification command.

The prompt must not ask the LLM to freely choose important diff regions, rewrite
production code, generate broad snapshots, run mutation testing, or claim
runtime confirmation.

## Implementation Mapping

The pure renderer is:

```text
ripr review-comments --root . --base <sha> --head <sha> --out target/ripr/review/comments.json
```

It reads existing evidence and writes JSON plus Markdown. It must not post to
GitHub by itself.

The generated GitHub workflow can then:

- run `ripr review-comments` for pull requests;
- upload `target/ripr/review/comments.{json,md}`;
- emit check annotations from the JSON;
- leave inline PR review comments disabled by default.

Campaign 11 workflow manifests and shared command templates provide the
artifact paths and agent commands linked from comments, so this surface does
not create another command-template source of truth.

## Required Evidence

The first implementation slice requires:

- a `review-comments` JSON report with schema version `0.1`;
- Markdown summary output for GitHub job summaries;
- changed-line placement rules with summary-only fallback;
- selection tests for production changes, nearby test changes, configured-off
  seams, suppressed seams, and missing-guidance seams;
- deterministic ranking and cap tests;
- stable dedupe keys;
- output schema, traceability, capability, and campaign entries that point to
  the behavior;
- generated CI documentation showing annotations by default and inline review
  comments as opt-in.

## Non-Goals

PR test guidance must not:

- add analyzer families;
- run mutation testing;
- edit source files;
- generate tests;
- enable unsaved-buffer overlays;
- make CI blocking by default;
- post inline PR review comments by default;
- ask an LLM to decide which diff regions matter;
- change SARIF or badge schemas;
- split the public crate surface.

## Acceptance Examples

- A pull request with one changed predicate and a visible `weakly_gripped` seam
  gets at most one line annotation when the seam line is changed.
- A pull request with an actionable seam outside the diff gets summary-only
  guidance, not a misplaced line annotation.
- A pull request that changes a nearby focused test does not receive a duplicate
  "write a test" recommendation for that seam.
- Inline review comments are absent unless the workflow explicitly opts in.
- The generated JSON contains enough bounded guidance for a human or LLM agent
  to draft one focused test and then run `ripr agent verify`.
- The report remains advisory; a failing or empty report does not block CI by
  default.

## Test Mapping

Initial implementation should add tests for:

- changed-line placement and summary-only fallback;
- selection rules for production changes, nearby test changes, configured-off
  seams, and suppressed seams;
- ranking and cap behavior;
- dedupe key stability;
- Markdown and annotation-safe escaping;
- generated workflow annotation emission;
- optional review-comment upsert behavior if a future publisher is implemented.

## Implementation Mapping

The first implementation should map this spec to:

- `crates/ripr/src/cli/commands.rs` or a focused CLI adapter for the
  `review-comments` command;
- an app/use-case module that joins existing repo exposure, agent packet, diff,
  config, and suppression evidence;
- an output module that renders JSON and Markdown without GitHub side effects;
- generated workflow wiring that emits job-summary and check-annotation output;
- optional GitHub review-comment publishing only after the pure renderer and
  annotation projection are fixture-backed.

## Metrics

- `pr_test_guidance_comments`
- `pr_test_guidance_summary_only`
- `pr_test_guidance_suppressed`
- `pr_test_guidance_opt_in_review_comments`
