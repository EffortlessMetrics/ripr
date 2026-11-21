# Assistant Loop Health Fixture Corpus

These files pin the Campaign 23 assistant-loop-health corpus for
`RIPR-SPEC-0022`.

They are static fixture artifacts. They do not implement
`ripr assistant-loop health`, rerun hidden analysis, edit source, generate tests,
call providers, run mutation testing, change recommendation ranking, change gate
policy, change LSP/editor behavior, or change CI blocking behavior.

Files:

- `corpus.json` records proof-shaped input states and expected health-report
  summaries for the bounded cases in RIPR-SPEC-0022.
- `proofs/*.json` records representative `test-oracle-assistant-proof` inputs
  that later producer work can read explicitly.
- `<case>/assistant-loop-health.json` and
  `<case>/assistant-loop-health.md` pin the expected report output for each
  health route.

The corpus intentionally covers:

- complete proof with improved static movement;
- partial proof with missing optional context;
- missing required proof input;
- unchanged static movement after an attempt;
- regressed static movement after an attempt;
- warning-heavy proof grouping;
- multiple proof inputs with deterministic counts and ordering.

Case directories:

- `complete-improved/`
- `partial-missing-optional/`
- `missing-required-input/`
- `unchanged/`
- `regressed/`
- `warning-heavy/`
- `multi-proof/`

Each case pins status, proof counts, movement counts, warning counts, repair
queue counts, representative warnings, and static limits. The later report
producer, generated CI projection, and docs should use this corpus as the
regression contract.
