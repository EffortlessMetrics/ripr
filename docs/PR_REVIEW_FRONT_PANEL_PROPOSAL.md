# PR Review Front Panel Proposal

Campaign 24 should make the generated GitHub PR and CI first screen tell one
advisory test-oracle story from explicit existing RIPR artifacts.

The current system already has PR guidance, first useful action, assistant
proof, assistant-loop health, baseline debt delta, RIPR Zero status, PR
evidence ledger, gate decisions, recommendation calibration, optional mutation
calibration imports, and coverage/grip frontier signals. The next adoption risk
is not missing evidence. It is asking reviewers to assemble that evidence from
separate reports.

## Goal

Add a PR review front panel that answers:

```text
What matters in this PR?
Why does it matter?
What focused test or acknowledgement path is next?
How should it be verified?
What receipt exists?
What baseline, waiver, suppression, or gate state applies?
What optional coverage/grip or calibration context changes interpretation?
```

The panel is advisory by default. It must not become gate authority, rerun
hidden analysis, edit source, generate tests, call providers, run mutation
testing, or reinterpret analyzer, ranking, gate, editor, or policy semantics.

## Inputs

The panel may consume only explicit existing artifact paths, including:

- PR guidance JSON/Markdown
- first useful action JSON/Markdown
- test-oracle assistant proof JSON/Markdown
- assistant-loop health JSON/Markdown
- PR evidence ledger JSON/Markdown
- baseline debt delta JSON/Markdown
- RIPR Zero status JSON/Markdown
- gate decision JSON/Markdown
- recommendation calibration JSON/Markdown
- supplied mutation calibration JSON/Markdown
- coverage/grip frontier JSON/Markdown
- agent receipt and verify artifacts
- report index entries

Missing optional inputs should stay visible as missing context. Missing
required inputs should produce a repair-oriented advisory panel, not success.

## First-Screen Shape

Markdown should be compact enough for the GitHub step summary:

```text
RIPR PR Review

Status: advisory | acknowledged | blocked | pass | config_error

Top issue:
- file:
- class:
- missing discriminator:
- suggested focused test:
- related test:

Movement:
- baseline resolved:
- new policy-eligible:
- static movement:
- coverage movement:

Repair:
- human next step:
- agent handoff:
- verify command:
- receipt:

Policy state:
- baseline:
- waiver:
- suppression:
- gate mode / decision:

Artifacts:
- start here:
- repair:
- evidence:
- policy:
```

JSON should preserve the same fields with stable vocabulary and source artifact
paths so generated CI, agents, and future portfolio rollups can consume it
without scraping Markdown.

## Work Items

1. `spec/pr-review-front-panel-report`
   Define the report contract, required and optional inputs, status vocabulary,
   summary sections, artifact grouping, advisory limits, and generated-CI
   projection boundaries.

2. `fixtures/pr-review-front-panel-corpus`
   Pin representative cases before implementation: advisory-only, actionable,
   summary-only, acknowledged, suppressed, baseline-resolved, blocked,
   missing-proof, no-actionable, and coverage-flat-grip-improved.

3. `report/pr-review-front-panel`
   Add a read-only producer that emits `pr-review-front-panel.{json,md}` from
   explicit artifact paths only.

4. `ci/pr-review-front-panel-summary`
   Surface the report in generated GitHub CI only when inputs exist. Preserve
   advisory defaults and gate-decision pass/fail authority.

5. `docs/pr-review-front-panel-workflow`
   Explain how reviewers, maintainers, developers, and coding agents read the
   front panel, follow repair routes, inspect receipts, and understand limits.

6. `dogfood/pr-review-front-panel-receipts`
   Record repo-local receipts proving the front-panel cases and validation.

7. `campaign/pr-review-front-panel-closeout`
   Close the campaign with a prompt-to-artifact audit and next-lane boundary.

## Non-Goals

- No analyzer behavior changes.
- No recommendation ranking changes.
- No gate policy or waiver semantics changes.
- No LSP/editor feature changes.
- No hidden artifact discovery that changes behavior by filesystem accident.
- No source edits.
- No generated tests.
- No provider calls.
- No mutation execution.
- No runtime adequacy claims from static evidence.
- No default CI blocking.
- No inline PR comments by default.

## Success Criteria

Campaign 24 succeeds when a reviewer can read one generated CI summary section
and understand the PR's RIPR state without opening raw JSON:

- top issue or no-action reason
- missing discriminator or static limit
- suggested repair or acknowledgement path
- verify command and receipt path
- old baseline, new debt, waiver, suppression, and gate state
- optional coverage/grip and calibration context
- full artifact paths for depth

The deeper artifacts remain available, but the front panel is the first screen.
