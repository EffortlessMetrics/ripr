# PR Review Front Panel Fixture Corpus

These files pin the Campaign 24 PR review front-panel corpus for
`RIPR-SPEC-0023`.

They are static fixture artifacts used by `ripr pr-review front-panel`. The
producer does not rerun hidden analysis, edit source, generate tests, call
providers, run mutation testing, change recommendation ranking, change gate
policy, publish inline comments, or change CI blocking behavior.

Files:

- `corpus.json` records PR-shaped input states and expected front-panel
  summaries for the bounded cases in RIPR-SPEC-0023.
- `<case>/pr-review-front-panel.json` and
  `<case>/pr-review-front-panel.md` pin the expected report output for each
  route.

The corpus intentionally covers:

- advisory-only PR with no actionable seam;
- actionable PR-local weak seam;
- summary-only guidance;
- acknowledged or waived policy candidate;
- suppressed candidate;
- baseline-resolved movement;
- configured blocking gate;
- missing proof or first-action input;
- flat coverage with improved static grip.

Case directories:

- `advisory-only/`
- `actionable/`
- `summary-only/`
- `acknowledged/`
- `suppressed/`
- `baseline-resolved/`
- `blocked/`
- `missing-proof/`
- `coverage-flat-grip-improved/`

Each case pins status, top-issue state, policy state, placement, movement,
coverage/grip state, summary counts, artifact groups, warnings, and advisory
limits. The producer and later generated CI projection should use this corpus as
the regression contract.
