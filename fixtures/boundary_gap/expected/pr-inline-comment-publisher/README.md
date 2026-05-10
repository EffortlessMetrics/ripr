# PR Inline Comment Publisher Fixture Corpus

These files pin the Campaign 26 PR inline comment publisher corpus for
`RIPR-SPEC-0025`.

They are static fixture artifacts for the future read-only publish-plan
producer. The producer must consume explicit `ripr review-comments` artifacts
and optional existing-comment metadata only. It must not rerun analysis, invent
placement, edit source, generate tests, call providers, run mutation testing,
change recommendation ranking, change gate policy, use `pull_request_target` by
default, post comments by default, or change CI blocking behavior.

Files:

- `corpus.json` records publish-plan input states and expected summary counts.
- `<case>/comment-publish-plan.json` and `.md` pin the expected plan output for
  each route.

The corpus intentionally covers:

- publishable changed-line comments;
- summary-only exclusion;
- cap overflow;
- dedupe update and keep behavior;
- stale existing comments;
- untrusted fork or missing-token blockers;
- missing PR guidance input.

Case directories:

- `publishable-changed-line/`
- `summary-only-excluded/`
- `cap-overflow/`
- `dedupe-upsert/`
- `stale-existing/`
- `fork-or-no-token/`
- `missing-input/`

Each case pins status, mode, publishable/skipped/blocked counts, summary-only
and cap skip reasons, operation vocabulary, blocked reason vocabulary, Markdown
headings, and advisory limits. The producer and later generated CI projection
should use this corpus as the regression contract.
