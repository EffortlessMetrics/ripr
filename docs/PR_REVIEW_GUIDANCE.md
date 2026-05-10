# PR Review Guidance

RIPR PR review guidance projects existing static seam evidence into a pull
request surface. It is meant to answer one narrow review question:

```text
For the changed behavior in this PR, which focused test would most improve the
static evidence, and what command verifies the before/after movement?
```

It is not a free-form code reviewer. It does not edit source files, generate
tests, run mutation testing, claim runtime confirmation, or make CI blocking by
default.

## Command

Run the pure report producer with an explicit base and head:

```bash
ripr review-comments \
  --root . \
  --base origin/main \
  --head HEAD \
  --out target/ripr/review/comments.json
```

The command writes:

```text
target/ripr/review/comments.json
target/ripr/review/comments.md
```

The command reads the pull request diff and existing RIPR evidence, then writes
bounded JSON and Markdown. It does not post to GitHub.

## What Reviewers See

The JSON separates recommendations by review placement:

- `comments[]` contains guidance that can safely attach to a changed line.
- `summary_only[]` contains useful guidance without a safe changed-line target.
- `suppressed[]` records candidates hidden by caps, nearby changed tests,
  configured severity, suppression policy, or missing guidance.
- `warnings[]` carries bounded selection warnings from the agent brief path.

Each line-placeable item carries the seam ID, grip class, severity, static
reason, missing discriminator when known, suggested test shape, and bounded LLM
handoff command. The command is the same existing agent loop command surface,
not a new review-specific workflow.

## Placement Rules

RIPR only attaches line guidance to changed lines. Placement falls through in
this order:

1. exact changed seam line;
2. nearest changed line in the same owner function;
3. nearest changed line in the same file;
4. summary-only guidance when no safe changed-line placement exists.

Bad placement is worse than no placement. If a seam cannot be tied to a changed
line, reviewers should see it in the summary rather than on an unrelated line.

## Generated CI Behavior

`ripr init --ci github` generates an advisory workflow that runs
`ripr review-comments` on pull requests before the existing RIPR advisory
summary and annotation consumer steps. The generated workflow:

- writes `target/ripr/review/comments.json` and `comments.md`;
- appends PR guidance counts and summary content to `$GITHUB_STEP_SUMMARY`;
- emits non-blocking GitHub check annotations from `comments[]`;
- uploads `target/ripr/review/` with the rest of the RIPR artifact packet;
- continues on error by default.

The generated workflow does not post inline PR review comments by default.
Inline review comments create durable review-thread noise and remain opt-in
through `RIPR_COMMENT_MODE`. See
[PR inline comment publisher workflow](PR_INLINE_COMMENT_PUBLISHER_WORKFLOW.md)
before enabling `plan` or `inline` mode.

## Inline Comment Boundary

If a repository builds its own inline review-comment publisher, it must be an
explicit opt-in outside the pure `ripr review-comments` command. The generated
publisher follows this same opt-in boundary through `RIPR_COMMENT_MODE` and the
[PR inline comment publisher workflow](PR_INLINE_COMMENT_PUBLISHER_WORKFLOW.md).
Any publisher must preserve the RIPR-SPEC-0012 boundaries and the
[RIPR-SPEC-0025](specs/RIPR-SPEC-0025-pr-inline-comment-publisher.md)
publish-plan contract:

- post only from `comments[]`, never from `summary_only[]`;
- target only changed lines;
- cap comments to three by default;
- deduplicate by `dedupe_key`;
- replace or upsert existing RIPR comments instead of adding duplicates;
- keep comments advisory and non-blocking.

Do not enable inline comments by default in generated workflows.

## Reviewer Loop

Use the PR guidance as the front door, then return to the normal RIPR evidence
loop:

1. Read the `RIPR advisory summary` in the GitHub job summary.
2. Inspect any non-blocking check annotations on changed lines.
3. Open `target/ripr/review/comments.md` when the guidance is summary-only.
4. Use the embedded `ripr agent brief` command for the selected seam.
5. Write one focused test outside RIPR.
6. Capture the after snapshot.
7. Run `ripr agent verify`.
8. Attach the receipt or `ripr agent review-summary` output when useful.

## Pinned Cases

The fixture matrix under
[`fixtures/boundary_gap/expected/pr-guidance`](../fixtures/boundary_gap/expected/pr-guidance/README.md)
pins the current behavior for:

- exact changed seam line;
- changed owner-function line;
- same-file changed line;
- summary-only fallback;
- cap suppression;
- configured-off suppression;
- nearby changed-test skip.

These fixtures are the regression contract for keeping PR guidance bounded and
placement-safe.

## Limits

PR review guidance remains static evidence:

- It does not establish runtime test strength.
- It does not run mutation testing.
- It does not decide whether a PR is mergeable.
- It does not inspect arbitrary diff regions like a general LLM reviewer.
- It does not create source edits or generated tests.

Real mutation testing, when supplied separately, belongs in calibration reports,
not in PR guidance claims.

Use [Recommendation calibration](RECOMMENDATION_CALIBRATION.md) when you need to
measure whether PR guidance was useful, correctly placed, properly suppressed,
and associated with better static evidence after one focused test.

Use [Calibrated gate policy](CALIBRATED_GATE_POLICY.md) only after the advisory
guidance and calibration loop are understood. Gates consume the same evidence;
they do not replace PR guidance or make it blocking by default.
