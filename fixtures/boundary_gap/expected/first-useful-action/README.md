# First Useful Action Fixture Corpus

These files pin the Campaign 22 first-useful-action routing corpus for
`RIPR-SPEC-0020`.

They are static fixture artifacts. They do not implement `ripr first-action`,
edit source, generate tests, call a provider, run mutation testing, rerun
hidden analysis, invent policy, or change CI blocking behavior.

Files:

- `corpus.json` records PR-shaped input states and expected first-action
  routing results for the bounded statuses in RIPR-SPEC-0020.
- `<case>/first-useful-action.json` and `<case>/first-useful-action.md` pin the
  expected report output for each route.

The corpus intentionally covers:

- actionable PR-local weak seam;
- stale evidence;
- missing required artifact;
- baseline-only debt;
- acknowledged item;
- waived item;
- suppressed item;
- no actionable seam;
- already-improved receipt state;
- unchanged-after-attempt receipt state.

Case directories:

- `actionable/`
- `stale/`
- `missing-required-artifact/`
- `baseline-only/`
- `acknowledged/`
- `waived/`
- `suppressed/`
- `no-actionable-seam/`
- `already-improved/`
- `unchanged-after-attempt/`

Each case pins the expected status, action kind, audience, selected seam,
target, routing reason, fallback state, command expectations, and static
limits. The future report producer should use this corpus before adding CI or
editor projection.
