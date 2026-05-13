# Generated CI Cockpit Baseline Audit

Status: complete

Lane: 4

Linked proposal:
[RIPR-PROP-0004: PR / CI Review Cockpit](../../docs/proposals/RIPR-PROP-0004-pr-ci-review-cockpit.md)

Linked spec:
[RIPR-SPEC-0038: Generated PR CI Review Workflow](../../docs/specs/RIPR-SPEC-0038-generated-pr-ci-review-workflow.md)

Linked gap map:
[generated-ci-gap-map.md](generated-ci-gap-map.md)

Linked manifest:
[.ripr/goals/lane4-pr-ci-review-cockpit.toml](../../.ripr/goals/lane4-pr-ci-review-cockpit.toml)

## Purpose

This audit records the generated GitHub workflow baseline that Lane 4 composes
today. It prevents later cockpit work from rebuilding shipped front-panel or
packet-index surfaces, and it gives the next generated-summary or dogfood PR a
bounded starting point.

This audit did not write a workflow file, run GitHub Actions, change generated
CI, change analyzer behavior, change gate policy, publish inline comments, edit
source, generate tests, call providers, or run mutation testing.

## Audit Inputs

Commands and files inspected:

```bash
rtk cargo run -p ripr -- init --help
rtk cargo run -p ripr -- init --ci github --dry-run
rtk powershell -NoProfile -Command 'Get-Content -LiteralPath "crates/ripr/src/cli/commands.rs" | Select-Object -Skip 990 -First 1450'
```

The current public dry-run command is:

```bash
ripr init --ci github --dry-run
```

The older command shape `ripr init ci github --dry-run` is not accepted by the
current CLI.

## Workflow Envelope

The generated workflow currently emits:

- workflow name `RIPR`;
- triggers `pull_request` and `workflow_dispatch`;
- permissions `contents: read`, `pull-requests: write`, and
  `security-events: write`;
- env knobs `RIPR_UPLOAD_SARIF`, `RIPR_GATE_MODE`, `RIPR_GATE_BASELINE`, and
  `RIPR_COMMENT_MODE`;
- a single `ripr` job named `RIPR advisory reports`;
- `continue-on-error` by default unless `RIPR_GATE_MODE` selects a configured
  non-visible gate mode;
- installation through `cargo install ripr --locked`;
- report upload through `actions/upload-artifact@v7` with
  `if-no-files-found: ignore` and 14-day retention.

## Public Commands Already Run

The generated workflow already runs or conditionally runs these public `ripr`
commands:

- `ripr pilot`;
- `ripr agent start`;
- `ripr agent packet`;
- `ripr check`;
- `ripr agent verify`;
- `ripr agent receipt`;
- `ripr outcome`;
- `ripr review-comments`;
- `ripr sarif`;
- `ripr badge`;
- `ripr gate evaluate`;
- `ripr baseline diff`;
- `ripr zero status`;
- `ripr pr-ledger record`;
- `ripr policy waiver-aging`;
- `ripr policy suppression-health`;
- `ripr policy readiness`;
- `ripr assistant-loop proof`;
- `ripr assistant-loop health`;
- `ripr first-action`;
- `ripr pr-review front-panel`;
- `ripr reports index`;
- `ripr agent status`;
- `ripr agent review-summary`.

The workflow also has one guarded repo-local step, `cargo xtask
operator-cockpit`, that runs only when the checked-out repository contains the
expected RIPR source files for that report.

## Cockpit Placement

The current generated workflow uses this ordering for the shipped Lane 4
surfaces:

```text
Generate advisory reports
-> render first useful action when explicit inputs exist
-> render PR review front panel when explicit inputs exist
-> render report packet index when packet inputs exist
-> render LLM work-loop summaries
-> append advisory job summary
-> upload report artifacts
```

The front panel is rendered into:

- `target/ripr/reports/pr-review-front-panel.json`;
- `target/ripr/reports/pr-review-front-panel.md`.

The packet index is rendered into:

- `target/ripr/reports/index.json`;
- `target/ripr/reports/index.md`.

## Job Summary Baseline

The generated job summary starts with:

```text
## RIPR advisory summary
RIPR is advisory static evidence. It does not edit source, generate tests, or
run mutation testing.
```

It then appends reviewer-facing sections including:

- `Start here`;
- `PR review summary`;
- `Recommended next test`;
- `Top recommendation`;
- policy readiness;
- suppression health;
- gate decision;
- baseline debt delta;
- RIPR Zero status;
- SARIF and badge status;
- PR guidance annotations;
- PR inline comment mode;
- known limits.

When `pr-review-front-panel.json` exists, the summary extracts the panel
status, headline, top issue state, policy state, placement, static movement,
coverage/grip state, counts, top issue, missing discriminator, suggested
focused test, related test, verify command, agent handoff, receipt, gate mode,
gate decision, warning count, and artifact paths. When
`pr-review-front-panel.md` exists, the workflow appends it after the compact
summary.

## Missing-Artifact Behavior

The generator guards optional cockpit producers with explicit input checks:

- `ripr first-action` runs only when at least one first-action input exists;
  otherwise the workflow prints `No RIPR first-useful-action inputs were
  available.`
- `ripr pr-review front-panel` runs only when at least one front-panel input
  exists; otherwise the workflow prints `No RIPR PR review front-panel inputs
  were available.`
- `ripr reports index` runs only when at least one known packet input exists;
  otherwise the workflow prints `No RIPR report-packet index inputs were
  available.`

The summary also states when the PR review summary was not generated and names
the classes of upstream artifacts that can enable it.

Follow-up status:
`ci/generated-summary-cockpit-contract` adds known regeneration commands for
the first-useful-action, PR review front-panel, and report packet-index
surfaces, and adds a `Start here` section to the generated job summary.

## Artifact Upload Baseline

The generated workflow uploads one `ripr-reports` artifact containing:

- `target/ripr/pilot`;
- `target/ripr/agent`;
- `target/ripr/workflow`;
- `target/ripr/reports`;
- `target/ripr/review`;
- `target/ci`.

It separately uploads SARIF when `RIPR_UPLOAD_SARIF` is true and the SARIF files
exist:

- `target/ripr/reports/ripr-findings.sarif` as `ripr-findings`;
- `target/ripr/reports/ripr-seams.sarif` as `ripr-seams`.

## Gate Authority Boundary

The generated workflow already keeps gate authority separate:

- the job is advisory and non-blocking by default;
- `ripr gate evaluate` only runs when `RIPR_GATE_MODE` is configured;
- the summary says pass/fail authority remains `ripr gate evaluate` when an
  explicit gate mode is configured;
- packet and summary surfaces summarize or point to gate artifacts but do not
  become the gate.

## Inline Comment Boundary

Inline PR comments remain opt-in. The generated workflow:

- keeps `RIPR_COMMENT_MODE` defaulted to `off`;
- can generate a publish plan in `plan` mode;
- publishes comments only in `inline` mode when same-repo permissions and
  changed-line placement are safe;
- always keeps inline comments separate from gate authority.

## Language Grouping Status

The generated workflow does not yet group cockpit output by preview language.
Rust-default output is the current baseline. Language-aware grouping remains a
separate Lane 4 slice and should wait until preview adapters provide enough
TypeScript and Python evidence, or until the lane explicitly defers Python.

## Remaining Gaps

| Gap | Next owner | Notes |
| --- | --- | --- |
| Summary wording and repair commands | `ci/generated-summary-cockpit-contract` | Done: generated summary starts with `Start here` and known missing-cockpit surfaces name regeneration commands. |
| Preview language grouping | `ci/language-aware-grouping` | Still blocked on preview-language evidence readiness or explicit deferral. |
| Dogfood receipts | `dogfood/lane4-cockpit-gap-receipts` | Done by [generated-CI cockpit dogfood receipts](../../docs/handoffs/2026-05-13-generated-ci-cockpit-receipts.md); Campaign 24 and 25 receipts remain the fixture-backed source for front-panel and packet-index states. |
| Closeout | `docs/lane4-closeout` | Close after the remaining implemented or deferred gaps are documented. |

## Non-Changes

This audit did not change:

- analyzer classification;
- recommendation ranking;
- PR review front-panel producer behavior;
- report packet-index producer behavior;
- output schemas;
- generated workflow YAML;
- editor or LSP behavior;
- gate policy semantics;
- default CI blocking;
- branch protection;
- inline comment defaults;
- source files or generated tests;
- provider calls or mutation execution.

## Validation

Docs-only audit updates should run:

```bash
rtk cargo xtask check-doc-index
rtk cargo xtask markdown-links
rtk cargo xtask check-static-language
rtk cargo xtask check-campaign
rtk cargo xtask check-pr
rtk git diff --check
```
