# RIPR-SPEC-0024: Report Packet Index

Status: proposed

Owner: ripr maintainers

Lane: 4

Linked proposal:
[RIPR-PROP-0004: PR / CI Review Cockpit](../proposals/RIPR-PROP-0004-pr-ci-review-cockpit.md)

Linked plan:
[Lane 4 implementation plan](../../plans/lane4-pr-ci-review-cockpit/implementation-plan.md)

Authority: behavior contract for the report packet index

Non-authority: PR order, analyzer truth, gate policy, source edits, generated
tests, editor routing, mutation execution, provider calls, and default CI
blocking

## Role

This spec defines observable behavior and acceptance examples for the Lane 4
report packet index surface. It does not define PR order, active execution
state, or policy authority. Implementation sequencing lives in
`plans/lane4-pr-ci-review-cockpit/`; configured pass/fail authority remains
with explicit gate-decision artifacts.

## Problem

Lane 4 now has a rich PR-time artifact stack:

- PR review front panel;
- first useful action;
- test-oracle assistant proof;
- assistant-loop health;
- PR evidence ledger;
- baseline debt delta and RIPR Zero status;
- optional gate decisions;
- recommendation, mutation, and coverage/grip calibration context;
- review guidance comments, SARIF, badges, validation reports, and receipts.

Those reports are useful, but the uploaded `ripr-reports` packet can still feel
like a directory listing. A reviewer, maintainer, developer, or coding agent
should not need to know the internal artifact topology to answer:

```text
Where do I start?
Which artifact explains the PR story?
Which artifact is pass/fail authority when a gate is configured?
Which packet routes a repair to a human or agent?
Which receipt proves evidence movement?
Which expected artifact is missing, and what command regenerates it?
```

The report packet index is the reviewer-first map over that packet.

## Product Contract

The report packet index is a read-only index over explicit existing artifact
paths. It does not create new evidence, rerun hidden analysis, inspect source
to infer missing fields, edit source, generate tests, call providers, run
mutation testing, change recommendation ranking, change gate policy, change
LSP/editor behavior, publish inline comments, or change default CI blocking.

The report must:

- group known artifacts by reviewer use rather than filename order;
- identify the recommended start-here artifact;
- distinguish PR story, repair, evidence, policy, calibration, validation,
  SARIF, badge, and receipt surfaces;
- show missing expected surfaces and concrete regeneration commands;
- preserve gate, waiver, acknowledgement, suppression, baseline, missing-input,
  stale, warning, blocked, and no-action states instead of hiding them;
- keep optional gate decisions separate as the only configured pass/fail
  authority;
- emit JSON for tools and Markdown for GitHub job summaries and human review;
- remain advisory even when the indexed packet contains a blocking gate
  decision.

## Behavior

The canonical behavior is:

```text
explicit report/receipt/workflow directories
-> bounded artifact inventory
-> expected-surface checks
-> reviewer-use grouping
-> regeneration commands for missing expected surfaces
-> advisory JSON and Markdown index
```

The producer may enumerate explicit input directories and known expected paths.
It must not discover hidden state outside those inputs, rerun analysis to
populate missing artifacts, or reinterpret the semantics of upstream reports.

## Inputs

The public report producer writes:

```text
target/ripr/reports/index.json
target/ripr/reports/index.md
```

The first public command surface should accept explicit roots and directories:

```text
ripr reports index \
  --root . \
  --reports-dir target/ripr/reports \
  --review-dir target/ripr/review \
  --receipts-dir target/ripr/receipts \
  --workflow-dir target/ripr/workflow \
  --agent-dir target/ripr/agent \
  --pilot-dir target/ripr/pilot \
  --ci-dir target/ci \
  --out target/ripr/reports/index.json \
  --out-md target/ripr/reports/index.md
```

Repo-local automation may keep `cargo xtask reports index` as a wrapper, but
generated GitHub CI should not depend on unpublished `xtask` when the report is
intended for repos that installed `ripr`.

Input directories are optional only when a repo genuinely does not produce that
class of artifact. Missing expected artifacts must remain visible as warnings
or `missing_expected[]` entries with a command to regenerate them when the
command is known.

## Required Evidence

The index can only summarize evidence already present in supplied directories
and known explicit artifact paths. A useful index should provide:

- report root and generated-at timestamp;
- explicit input directories and paths inspected by the producer;
- grouped artifact entries with label, path, availability, status, requiredness,
  and authority flags;
- one recommended start-here artifact, preferably `start-here.md` when present,
  then `pr-review-front-panel.md` as the fallback PR review story;
- gate authority path when a gate decision exists;
- missing expected artifacts with bounded reasons and regeneration commands
  when known;
- warnings for unreadable, stale, malformed, incomplete, missing-input, or
  warning-state artifacts;
- advisory limits copied into both JSON and Markdown.

Missing evidence stays visible as `missing_expected[]`, warnings, `null`,
`unknown`, `not_applicable`, or unavailable entries. The index must not
synthesize missing evidence from the workspace, source inspection, provider
calls, runtime mutation execution, or another analyzer pass.

## Expected Surfaces

The index should account for these artifacts when present:

| Surface | Primary path | Group | Expected when |
| --- | --- | --- | --- |
| PR review front panel | `target/ripr/reports/pr-review-front-panel.md` | `start_here` | PR summary inputs exist |
| First useful action | `target/ripr/reports/first-useful-action.md` | `pr_review_story` | PR guidance or assistant proof exists |
| Assistant proof | `target/ripr/reports/test-oracle-assistant-proof.md` | `repair_agent_handoff` | proof inputs exist |
| Assistant loop health | `target/ripr/reports/assistant-loop-health.md` | `repair_agent_handoff` | assistant proof exists |
| PR evidence ledger | `target/ripr/reports/pr-evidence-ledger.md` | `evidence_movement` | PR guidance exists |
| Baseline debt delta | `target/ripr/reports/baseline-debt-delta.md` | `evidence_movement` | baseline and current gate inputs exist |
| RIPR Zero status | `target/ripr/reports/ripr-zero-status.md` | `evidence_movement` | baseline delta exists |
| Gate decision | `target/ripr/reports/gate-decision.md` | `policy_gates` | `RIPR_GATE_MODE` is configured |
| Recommendation calibration | `target/ripr/reports/recommendation-calibration.md` | `calibration` | calibration inputs exist |
| Mutation calibration | `target/ripr/reports/mutation-calibration.md` | `calibration` | imported runtime calibration exists |
| Coverage/grip frontier | `target/ripr/reports/coverage-grip-frontier.md` | `calibration` | coverage or PR movement inputs exist |
| Review comments | `target/ripr/review/comments.md` | `pr_review_story` | PR guidance was generated |
| Agent receipt | `target/ripr/reports/agent-receipt.json` | `validation_receipts` | repair verification was attempted |
| PR summary | `target/ripr/reports/pr-summary.md` | `validation_receipts` | local PR packet exists |
| Check PR report | `target/ripr/reports/check-pr.md` | `validation_receipts` | local readiness gate ran |
| SARIF output | `target/ripr/reports/ripr.sarif.json` | `sarif_badges` | SARIF rendering is enabled |
| Badge output | `target/ripr/reports/ripr-badge.json` | `sarif_badges` | badge rendering ran |

The implementation may add more entries, but it should keep the group names and
authority rules stable.

## Group Vocabulary

`group` must be one of:

- `start_here`: the first artifact a reviewer should open;
- `pr_review_story`: PR guidance, first action, and review-comment surfaces;
- `repair_agent_handoff`: assistant proof, health, agent packets, and workflow
  artifacts that route focused repair;
- `evidence_movement`: baseline, ledger, RIPR Zero, receipts, and movement
  reports;
- `policy_gates`: gate decision, waiver, acknowledgement, suppression, and
  configured pass/fail context;
- `calibration`: recommendation, mutation, and coverage/grip calibration
  context;
- `validation_receipts`: local validation, check, dogfood, and PR readiness
  receipts;
- `sarif_badges`: SARIF/code scanning and badge artifacts;
- `local_context`: repo-local or operator-only context that helps maintainers
  but is not a public PR decision surface.

## Status Vocabulary

Top-level `status` must be one of:

- `pass`: the index rendered and no expected required surface is missing.
- `warn`: the index rendered, but expected optional or recommended surfaces are
  missing, stale, warning, or incomplete.
- `fail`: at least one indexed artifact explicitly reports `fail`, `blocked`,
  or `config_error`.
- `incomplete`: the index could not read enough explicit inputs to produce a
  useful packet map.

`entry.status` must be one of:

- `available`;
- `missing`;
- `pass`;
- `warn`;
- `fail`;
- `blocked`;
- `acknowledged`;
- `suppressed`;
- `stale`;
- `incomplete`;
- `unreadable`;
- `not_applicable`.

`missing_reason` must be one of:

- `not_generated`;
- `input_not_available`;
- `configured_off`;
- `missing_required_input`;
- `stale_upstream`;
- `unknown`.

The index status is packet-health context only. It must not replace
`gate-decision.{json,md}` as the configured pass/fail authority.

## JSON Shape

The JSON report uses schema version `0.1`:

```json
{
  "schema_version": "0.1",
  "tool": "ripr",
  "kind": "report_packet_index",
  "status": "warn",
  "root": ".",
  "generated_at": "2026-05-10T12:00:00Z",
  "inputs": {
    "reports_dir": "target/ripr/reports",
    "review_dir": "target/ripr/review",
    "receipts_dir": "target/ripr/receipts",
    "workflow_dir": "target/ripr/workflow",
    "agent_dir": "target/ripr/agent",
    "pilot_dir": "target/ripr/pilot",
    "ci_dir": "target/ci"
  },
  "summary": {
    "entries": 18,
    "available": 14,
    "missing_expected": 4,
    "warnings": 3,
    "failures": 0,
    "start_here": "target/ripr/reports/pr-review-front-panel.md",
    "gate_authority": "target/ripr/reports/gate-decision.md",
    "advisory": true
  },
  "groups": [
    {
      "group": "start_here",
      "label": "Start here",
      "summary": "Reviewer-first PR story.",
      "entries": [
        {
          "id": "pr_review_front_panel",
          "label": "PR review front panel",
          "kind": "markdown",
          "path": "target/ripr/reports/pr-review-front-panel.md",
          "json_path": "target/ripr/reports/pr-review-front-panel.json",
          "status": "available",
          "available": true,
          "required": true,
          "authority": false,
          "description": "First-screen PR review story.",
          "next_command": null
        }
      ]
    }
  ],
  "missing_expected": [
    {
      "id": "assistant_loop_health",
      "label": "Assistant loop health",
      "group": "repair_agent_handoff",
      "path": "target/ripr/reports/assistant-loop-health.md",
      "required": false,
      "reason": "input_not_available",
      "next_command": "ripr assistant-loop health --proof target/ripr/reports/test-oracle-assistant-proof.json --out target/ripr/reports/assistant-loop-health.json --out-md target/ripr/reports/assistant-loop-health.md"
    }
  ],
  "warnings": [
    {
      "kind": "missing_expected",
      "message": "Assistant loop health was not generated because no proof input was present.",
      "source_artifact": null
    }
  ],
  "limits": [
    "Advisory report-packet index only.",
    "Does not rerun analysis.",
    "Does not edit source or generate tests.",
    "Does not call providers.",
    "Does not run mutation testing.",
    "Does not publish inline comments.",
    "Does not change default CI blocking.",
    "Gate decision remains pass/fail authority when configured."
  ]
}
```

Field contract:

- `schema_version` is `0.1` until the report shape changes.
- `kind` is always `report_packet_index`.
- `status`, `groups[].group`, `entries[].status`, and
  `missing_expected[].reason` use the bounded vocabularies in this spec.
- `inputs.*` records explicit directories and paths that the producer was
  allowed to inspect.
- `summary.start_here` is the first artifact to show in generated CI and human
  packet navigation. Prefer `pr-review-front-panel.md` when available.
- `summary.gate_authority` records `gate-decision.md` when supplied. The index
  must not mark itself as gate authority.
- `groups[]` preserves reviewer-use ordering. The same artifact should not
  appear in multiple groups unless the spec later adds explicit cross-links.
- `entries[].authority` is `true` only for an upstream artifact that is
  authority for its own domain, such as `gate-decision.md` for configured gate
  pass/fail. The index entry itself remains advisory.
- `missing_expected[]` keeps absent expected surfaces visible with a bounded
  reason and, when known, a command to regenerate the missing surface.
- `warnings[]` carries malformed, stale, unreadable, missing-input, and
  incomplete packet context without converting it to waiver, suppression,
  improvement, runtime confirmation, or pass/fail authority.
- `limits` must preserve the read-only, explicit-input, no-source-edit,
  no-generated-test, no-provider-call, no-runtime-mutation-execution,
  no-inline-comment, and advisory-default boundaries.

## Markdown Shape

The Markdown sibling should fit in a generated GitHub job summary while still
being useful as the uploaded packet front door:

```md
# RIPR Report Packet Index

Status: warn

Start here:
- PR review front panel: target/ripr/reports/pr-review-front-panel.md
- Gate authority: target/ripr/reports/gate-decision.md

Packet summary:
- Available artifacts: 14
- Missing expected artifacts: 4
- Warnings: 3
- Failures: 0

PR review story:
- first-useful-action: target/ripr/reports/first-useful-action.md
- review comments: target/ripr/review/comments.md

Repair and agent handoff:
- assistant proof: target/ripr/reports/test-oracle-assistant-proof.md
- assistant loop health: missing
  - next: `ripr assistant-loop health --proof target/ripr/reports/test-oracle-assistant-proof.json --out target/ripr/reports/assistant-loop-health.json --out-md target/ripr/reports/assistant-loop-health.md`

Policy and gates:
- gate decision: target/ripr/reports/gate-decision.md
- authority: gate decision controls configured pass/fail, not this index

Limits:
- Advisory index only.
- Does not rerun analysis.
- Does not run mutation testing.
- Does not edit source or generate tests.
```

When no useful packet map can be rendered, Markdown should put the repair first:

```md
# RIPR Report Packet Index

Status: incomplete

Next: generate the PR summary or report packet before using the index.
```

## Generated CI Projection

Generated GitHub CI may run the report packet index producer after the
individual report producers and before artifact upload. It may upload
`index.{json,md}` with the normal `ripr-reports` artifact and append a compact
index section to the job summary.

Generated CI must not:

- make the index pass/fail authority;
- change `RIPR_GATE_MODE` defaults;
- publish inline PR comments;
- skip uploading lower-level artifacts;
- hide gate, waiver, acknowledgement, suppression, baseline, missing-input,
  stale, warning, blocked, or no-action states;
- rerun hidden analysis to fill missing artifacts;
- fail on missing optional index entries unless the explicit gate decision
  already failed or reported `config_error`.

`ripr gate evaluate` and `gate-decision.{json,md}` remain the only configured
pass/fail authority.

## Acceptance Examples

- A complete PR packet returns `status = "pass"`, marks
  `pr-review-front-panel.md` as `start_here`, and groups all known artifacts by
  reviewer use.
- A sparse advisory packet returns `status = "warn"` and lists missing optional
  or recommended surfaces with regeneration commands where known.
- A missing PR review front panel keeps other artifacts visible and marks the
  front panel as a missing expected start-here surface.
- A configured blocked gate returns `status = "fail"` or an entry status of
  `blocked`, but points to `gate-decision.md` as authority.
- Missing assistant proof or health entries remain visible as repair or agent
  handoff gaps instead of being treated as clean success.
- A coverage/grip frontier artifact is grouped under `calibration` and never
  presented as runtime confirmation.
- Validation receipts such as `check-pr.md`, `fixtures.md`, `goldens.md`, and
  dogfood reports are grouped under `validation_receipts`.

## Test Mapping

Follow-up tests and fixtures should cover:

- complete packet;
- sparse advisory packet;
- missing front-panel start-here artifact;
- blocked gate with gate-decision authority preserved;
- missing assistant proof;
- missing receipt;
- coverage/grip-present packet;
- malformed or unreadable artifact;
- stale or warning artifact;
- missing optional artifact with regeneration command;
- JSON grouping and Markdown grouping;
- generated-CI projection that remains advisory.

## Implementation Mapping

Follow-up implementation belongs to Campaign 25:

- `fixtures/report-packet-index-corpus` pins the complete, sparse, missing
  front-panel, blocked-gate, missing-proof, missing-receipt, coverage/grip,
  malformed, stale, and warning packet cases before producer changes.
- `report/report-packet-index` adds the read-only public `ripr reports index`
  producer that emits grouped reviewer-first JSON/Markdown from explicit
  existing artifact paths.
- `ci/report-packet-index-summary` runs the public producer, uploads, and
  summarizes the index only when indexed artifacts exist and keeps gate
  decision as pass/fail authority.
- `docs/report-packet-index-workflow` explains reviewer, maintainer,
  developer, and coding-agent use of the packet index, missing-surface
  regeneration, grouped artifact navigation, and advisory gate boundaries.
- `dogfood/report-packet-index-receipts` records repo-local receipts for the
  representative packet states.
- `campaign/report-packet-index-closeout` records the final audit, validation,
  and future-lane boundary.

No follow-up surface may change analyzer behavior, recommendation ranking, gate
policy semantics, LSP/editor behavior, source files, generated tests, provider
calls, mutation execution, public crate shape, inline comment defaults, or
default CI blocking as part of this campaign.

## Metrics

The report should make these counts available to later metrics surfaces:

- `report_packet_index_reports`;
- `report_packet_index_pass`;
- `report_packet_index_warn`;
- `report_packet_index_fail`;
- `report_packet_index_incomplete`;
- `report_packet_index_available_entries`;
- `report_packet_index_missing_expected`;
- `report_packet_index_start_here_available`;
- `report_packet_index_gate_authority_present`;
- `report_packet_index_repair_commands`;
- `report_packet_index_warning_entries`.

## Non-Goals

- No analyzer behavior changes.
- No recommendation ranking changes.
- No gate policy semantic changes.
- No LSP or editor behavior changes.
- No generated workflow changes in this spec PR.
- No default CI blocking.
- No inline comment publishing.
- No source edits.
- No generated tests.
- No provider or API calls.
- No mutation execution.
- No implicit source inspection or hidden analysis reruns.
- No opaque score.
- No runtime correctness or sufficiency claims.

## Validation

The spec PR should run:

```bash
cargo xtask check-spec-format
cargo xtask check-doc-index
cargo xtask markdown-links
cargo xtask check-static-language
cargo xtask check-traceability
cargo xtask check-capabilities
cargo xtask check-campaign
cargo xtask goals next
cargo xtask check-pr
git diff --check
```
