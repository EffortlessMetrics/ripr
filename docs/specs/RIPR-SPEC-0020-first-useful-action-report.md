# RIPR-SPEC-0020: First Useful Action Report

Status: proposed

## Problem

RIPR now has several advisory evidence surfaces:

- saved-workspace editor diagnostics, hover, actions, and context packets;
- PR guidance comments and summary-only recommendations;
- baseline debt deltas and RIPR Zero status;
- PR evidence ledgers and coverage/grip frontier reports;
- test-oracle assistant proof reports;
- targeted receipts and agent receipts;
- optional calibrated gate decisions.

Those artifacts are useful, but a developer, reviewer, or coding agent should
not need to learn the full artifact topology before deciding what to do next.
The first useful action report compresses existing evidence into one advisory
answer:

```text
What is the next useful test action, why this action first, where should it be
done, how should it be verified, and which receipt records the outcome?
```

## Product Contract

The first useful action report is a read-only routing layer over explicit RIPR
artifacts. It does not add analyzer semantics, run hidden analysis, edit
source, generate tests, call a provider, run mutation testing, invent policy,
or change default CI blocking.

The report must:

- select one top action or explain why no action should be taken;
- preserve the selected seam identity, missing discriminator, related test, and
  suggested focused-test shape when available;
- distinguish fresh, stale, missing, waived, acknowledged, suppressed,
  baseline-only, improved, unchanged, and no-actionable states;
- prefer explicit PR-local evidence over unrelated baseline debt;
- preserve receipts and static movement when supplied;
- keep optional gate decisions separate from first-action routing authority;
- emit JSON for agents and Markdown for PR summaries, CI summaries, and human
  review;
- keep static evidence vocabulary conservative and advisory.

## Behavior

The canonical behavior is:

```text
existing RIPR artifacts
-> deterministic input status checks
-> selected seam or fallback state
-> first useful action
-> verification command and receipt path
-> advisory JSON and Markdown
```

The report producer must read explicit input paths. It must not search hidden
state or rerun analysis to fill missing evidence.

## Required Evidence

The report can consume these inputs:

| Input | Role | Required |
| --- | --- | --- |
| PR guidance comments JSON | Changed-line or summary-only recommendation source | Recommended |
| Gap decision ledger | Policy-targeted gap IDs, repair routes, anchors, and verification commands | Recommended when available |
| PR evidence ledger | PR-local debt, resolved debt, waivers, acknowledgements, and repair receipts | Recommended |
| Baseline debt delta | Baseline-only, new, resolved, suppressed, stale, or invalid debt state | Recommended |
| Test-oracle assistant proof | Joined recommendation, handoff, before/after evidence, receipt, and optional CI projection | Recommended |
| Agent or targeted-test receipt | Static before/after movement and selected seam receipt | Optional |
| Gate decision | Optional policy context, not routing authority | Optional |
| Coverage/grip frontier | Optional coverage/grip context, not adequacy evidence | Optional |
| Editor evidence context | Optional saved-workspace seam context exported from the editor | Optional |
| Status/staleness input | Optional freshness state for editor, artifact, or CI-produced evidence | Optional |

Missing optional inputs must be reported as `null`, `unknown`,
`not_available`, omitted when the field is additive and unsupplied, or
warnings. Missing inputs must not be treated as
acknowledgement, waiver, suppression, improvement, or runtime confirmation.

## Report Contract

The public report producer will write:

```text
target/ripr/reports/first-useful-action.json
target/ripr/reports/first-useful-action.md
```

The command accepts explicit input paths:

```text
ripr first-action \
  --root . \
  --pr-guidance target/ripr/review/comments.json \
  --assistant-proof target/ripr/reports/test-oracle-assistant-proof.json \
  --gap-ledger target/ripr/reports/gap-decision-ledger.json \
  --ledger target/ripr/reports/pr-evidence-ledger.json \
  --baseline-delta target/ripr/reports/baseline-debt-delta.json \
  --receipt target/ripr/reports/agent-receipt.json \
  --gate-decision target/ripr/reports/gate-decision.json \
  --coverage-frontier target/ripr/reports/coverage-grip-frontier.json \
  --editor-context target/ripr/workflow/evidence-context.json \
  --out target/ripr/reports/first-useful-action.json \
  --out-md target/ripr/reports/first-useful-action.md
```

The command is advisory. It must not fail CI, post comments, edit source,
generate tests, call a provider, run mutation testing, or claim runtime
confirmation from static evidence.

## Status And Action Vocabulary

`status` must be one of:

- `actionable`: the report selected one PR-local action the user can take now.
- `stale`: the best available evidence is stale enough that refresh is the
  first action.
- `missing_required_artifact`: a needed artifact is missing, unreadable, or
  incompatible.
- `baseline_only`: evidence is real but not PR-local first-action work.
- `acknowledged`: the relevant item has an explicit acknowledgement.
- `waived`: the relevant item has an explicit waiver.
- `suppressed`: the relevant item is suppressed by configuration or existing
  suppression state.
- `no_actionable_seam`: no current seam has enough actionable evidence.
- `already_improved`: supplied movement or receipts show the selected static
  evidence already improved or resolved.
- `unchanged_after_attempt`: a supplied receipt shows unchanged static movement
  after a focused-test attempt.

`action_kind` must be one of:

- `write_focused_test`;
- `refresh_evidence`;
- `generate_missing_artifact`;
- `acknowledge_baseline`;
- `inspect_proof_report`;
- `revise_focused_test`;
- `no_action`.

`audience` must be one of:

- `developer`;
- `reviewer`;
- `agent`.

## Routing Contract

The first implementation should start with deterministic routing:

1. Stale evidence routes to `refresh_evidence`.
2. Missing required evidence routes to `generate_missing_artifact`.
3. Explicit repairable stable Rust `GapRecord` inputs with PR-local new or
   reintroduced policy state can route the first action without requiring a raw
   assistant-proof chain.
4. New PR-local actionable seams beat baseline-only debt.
5. Complete test-oracle assistant proof beats a raw artifact chain for the same
   seam.
6. Unchanged receipts route to `revise_focused_test`.
7. Waived, acknowledged, or suppressed items remain visible but are not the top
   action unless there is no unsuppressed PR-local work.
8. No actionable seam returns `no_actionable_seam` and `no_action` rather than
   silence.

The producer must expose why the selected action came first. It must not hide
lower-priority evidence when it explains the fallback state.

## JSON Shape

The JSON report uses schema version `0.1`:

```json
{
  "schema_version": "0.1",
  "tool": "ripr",
  "kind": "first_useful_action",
  "status": "actionable",
  "audience": "developer",
  "action_kind": "write_focused_test",
  "root": ".",
  "generated_at": "2026-05-09T12:00:00Z",
  "inputs": {
    "pr_guidance": "target/ripr/review/comments.json",
    "assistant_proof": "target/ripr/reports/test-oracle-assistant-proof.json",
    "gap_ledger": "target/ripr/reports/gap-decision-ledger.json",
    "ledger": "target/ripr/reports/pr-evidence-ledger.json",
    "baseline_delta": "target/ripr/reports/baseline-debt-delta.json",
    "receipt": "target/ripr/reports/agent-receipt.json",
    "gate_decision": "target/ripr/reports/gate-decision.json",
    "coverage_frontier": "target/ripr/reports/coverage-grip-frontier.json",
    "editor_context": "target/ripr/workflow/evidence-context.json"
  },
  "selected": {
    "source": "assistant_proof",
    "source_artifact": "target/ripr/reports/test-oracle-assistant-proof.json",
    "seam_id": "67fc764ba37d77bd",
    "seam_kind": "predicate_boundary",
    "path": "src/pricing.rs",
    "line": 88,
    "classification": "weakly_exposed",
    "missing_discriminator": "amount == discount_threshold",
    "gap_id": "gap:pr:pricing:threshold-boundary",
    "canonical_gap_id": "gap:rust:pricing:discount:threshold-boundary",
    "repair_route": "AddBoundaryAssertion"
  },
  "title": "Add equality-boundary discriminator test",
  "why": "Changed predicate boundary is weakly exposed and lacks an equality-boundary discriminator.",
  "why_first": [
    "The seam is PR-local.",
    "The assistant proof report links guidance, handoff, before/after evidence, and receipt inputs.",
    "No waiver, acknowledgement, or suppression applies."
  ],
  "target": {
    "file": "tests/pricing.rs",
    "related_test": "below_threshold_has_no_discount",
    "suggested_test_name": "discounted_total_boundary_discriminator",
    "suggested_assertion": "Assert the exact returned discount at the equality boundary."
  },
  "commands": {
    "context_packet": "ripr agent packet --root . --seam-id 67fc764ba37d77bd --json",
    "after_snapshot": "ripr check --root . --mode draft --format repo-exposure-json > target/ripr/workflow/after.repo-exposure.json",
    "verify": "ripr agent verify --root . --before target/ripr/workflow/before.repo-exposure.json --after target/ripr/workflow/after.repo-exposure.json --json",
    "receipt": "ripr agent receipt --root . --verify-json target/ripr/workflow/agent-verify.json --seam-id 67fc764ba37d77bd --json"
  },
  "evidence": {
    "pr_guidance": "target/ripr/review/comments.json",
    "assistant_proof": "target/ripr/reports/test-oracle-assistant-proof.json",
    "receipt": "target/ripr/reports/agent-receipt.json",
    "ledger": "target/ripr/reports/pr-evidence-ledger.json",
    "static_movement": "unknown"
  },
  "fallback": null,
  "warnings": [],
  "limits": [
    "Static evidence only.",
    "Does not prove runtime adequacy.",
    "Does not run mutation testing.",
    "Does not edit source or generate tests.",
    "Does not make CI blocking by default."
  ]
}
```

Field contract:

- `schema_version` is `0.1` until the report shape changes.
- `kind` is always `first_useful_action`.
- `status`, `action_kind`, and `audience` must use the bounded vocabularies in
  this spec.
- `inputs.*` records explicit input paths. Missing optional paths are `null`
  or omitted for additive unsupplied fields; missing required or invalid
  supplied paths produce warnings and an appropriate fallback status.
- `selected.*` is copied from existing RIPR artifacts. The producer must not
  mint a new seam identity or rerank with a model.
- `classification` must use conservative static classification vocabulary
  already emitted by RIPR.
- `why_first` records deterministic routing reasons. It must not be an opaque
  score.
- `target.*` records the recommended file, related test, suggested test name,
  and assertion shape when supplied by existing artifacts.
- `commands.*` records copyable commands from existing command templates or
  supplied artifacts. Missing commands become `null` and warnings.
- `evidence.*` records supporting artifact paths and static movement when
  supplied. Static movement is not runtime mutation confirmation.
- `fallback` records the reason for non-actionable statuses and the next safe
  command when available.
- `limits` must preserve the static-evidence, no-edit, no-generated-test,
  no-provider-call, no-runtime-mutation-execution, and advisory-default
  boundaries.

## Markdown Shape

The Markdown sibling should fit in a PR summary, generated CI job summary, or
editor status detail:

```md
# RIPR First Useful Action

Status: actionable
Audience: developer
Action: write_focused_test

## Next

Add equality-boundary discriminator test.

Why first:
- The seam is PR-local.
- The proof report links guidance, handoff, before/after evidence, and receipt
  inputs.
- No waiver, acknowledgement, or suppression applies.

Where:
- File: tests/pricing.rs
- Related test: below_threshold_has_no_discount
- Suggested test: discounted_total_boundary_discriminator

Verify:
`ripr agent verify --root . --before target/ripr/workflow/before.repo-exposure.json --after target/ripr/workflow/after.repo-exposure.json --json`

Receipt:
`ripr agent receipt --root . --verify-json target/ripr/workflow/agent-verify.json --seam-id 67fc764ba37d77bd --json`

Limits:
- Static evidence only.
- Does not run mutation testing.
- Does not edit source or generate tests.
```

For fallback statuses, the Markdown should put the fallback before any lower
priority details:

```md
# RIPR First Useful Action

Status: stale
Action: refresh_evidence

Next: refresh RIPR evidence before acting on this recommendation.
```

## Acceptance Examples

- New PR-local weak seam with a related test, missing discriminator, and
  verify command returns `status = "actionable"` and
  `action_kind = "write_focused_test"`.
- Stale editor or artifact state returns `status = "stale"` and
  `action_kind = "refresh_evidence"`.
- Missing assistant proof or PR guidance required for the selected route
  returns `status = "missing_required_artifact"` and
  `action_kind = "generate_missing_artifact"`.
- A supplied repairable stable Rust `GapRecord` with a PR-local new policy state
  returns `status = "actionable"` and uses the record's gap ID, repair route,
  target, and verification command instead of raw classifier labels.
- Baseline-only evidence returns `status = "baseline_only"` and keeps the debt
  visible without presenting it as PR-local first work.
- Waived, acknowledged, and suppressed findings are visible but do not outrank
  unsuppressed PR-local work.
- A receipt with improved or resolved static movement returns
  `status = "already_improved"` and `action_kind = "no_action"`.
- A receipt with unchanged static movement returns
  `status = "unchanged_after_attempt"` and
  `action_kind = "revise_focused_test"`.
- No actionable seam returns `status = "no_actionable_seam"` and
  `action_kind = "no_action"`.

## Test Mapping

Follow-up tests should cover:

- corpus fixtures for actionable, stale, missing-required-artifact,
  baseline-only, acknowledged, waived, suppressed, already-improved,
  unchanged-after-attempt, and no-actionable-seam cases;
- JSON and Markdown producer tests that pin the schema and fallback wording;
- GapRecord routing tests that prove first-action can use explicit gap decisions
  without assistant-proof inference and without projecting generic confidence;
- generated-CI summary tests that keep the report advisory;
- LSP projection tests that show the report in status without adding
  diagnostics or editor decorations;
- malformed, missing, stale, and incompatible input tests.

## Implementation Mapping

Follow-up implementation belongs to Campaign 22:

- `fixtures/first-useful-action-corpus` pins routing cases before code;
- `report/first-useful-action` adds the read-only producer;
- `report/first-useful-action-gap-record` adds optional gap decision ledger
  routing for explicit repairable stable Rust `GapRecord` inputs;
- `ci/first-useful-action-summary` surfaces the Markdown and JSON as advisory
  generated-CI artifacts;
- `lsp/first-useful-action-status` projects the report through existing editor
  status and Show Status;
- `docs/first-useful-action-workflow` explains the user workflow;
- `dogfood/first-useful-action-receipts` records repo-local receipt examples.

## Metrics

The report should make these counts available to later metrics surfaces:

- `first_useful_action_reports`;
- `first_useful_action_actionable`;
- `first_useful_action_stale`;
- `first_useful_action_missing_required_artifact`;
- `first_useful_action_baseline_only`;
- `first_useful_action_acknowledged`;
- `first_useful_action_waived`;
- `first_useful_action_suppressed`;
- `first_useful_action_no_actionable_seam`;
- `first_useful_action_already_improved`;
- `first_useful_action_unchanged_after_attempt`;

## Non-Goals

- analyzer changes;
- new ranking model or provider calls;
- source edits or generated tests;
- runtime mutation execution;
- default CI blocking;
- policy or gate semantic changes;
- hidden analysis reruns or implicit artifact discovery;
- new diagnostics, CodeLens, inlay hints, unsaved-buffer overlays, or other
  speculative editor surfaces;
- treating coverage, static movement, or first-action routing as runtime
  proof.

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
