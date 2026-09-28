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
- fresh PR whose review card carries the repair start (`repair-start/`): the
  top issue and Repair block lead with that exact command. The panel carries
  `repair_command` from first-action `commands.repair`, a review card's
  `llm_guidance.repair_command`, or a gate route's `repair_command`, and never
  synthesizes an `agent start` or `agent repair` command from a bare seam id;
  without a carried command `agent_command` is a carried read-only inspection
  command or `null`;
- summary-only guidance;
- acknowledged or waived policy candidate;
- suppressed candidate;
- baseline-resolved movement;
- configured blocking gate;
- missing proof or first-action input;
- flat coverage with improved static grip;
- mixed assistant-health outcomes that keep repair-bearing proof visible.
- same-path regenerated receipt content that fails closed when its movement no
  longer matches the selected proof.

Case directories:

- `advisory-only/`
- `actionable/`
- `repair-start/`
- `summary-only/`
- `acknowledged/`
- `suppressed/`
- `baseline-resolved/`
- `blocked/`
- `missing-proof/`
- `coverage-flat-grip-improved/`
- `mixed-health/`
- `stale-same-path-receipt/`

Each case pins status, top-issue state, policy state, placement, movement,
coverage/grip state, summary counts, artifact groups, warnings, and advisory
limits. The producer and later generated CI projection should use this corpus as
the regression contract.
