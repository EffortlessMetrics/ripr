# RIPR-SPEC-0043: Preview Evidence Promotion Packet

Status: proposed

## Problem

TypeScript and Python preview evidence is intentionally useful before it is
policy-authoritative. RIPR can surface preview findings in reports, summaries,
and editor surfaces, but preview evidence must not become gate-eligible, RIPR
Zero blocking debt, or calibrated confidence by accident.

Maintainers still need a future path to ask:

```text
What would be required before this preview-language class can govern policy?
```

Without a dedicated preview promotion packet, a generic policy promotion packet
could hide the language boundary, treat syntax-first preview evidence like
stable Rust evidence, or imply that visibility is enough for blocking policy.

## Behavior

The preview evidence promotion packet is a read-only advisory report for a
language and evidence class:

```text
ripr policy preview-promote \
  --language typescript \
  --class boundary_gap
```

The default result is blocked:

```text
allowed_now = false
reason = "preview promotion evidence not supplied"
```

The command must never mutate `ripr.toml`, baselines, suppressions, workflows,
branch protection, generated CI defaults, source files, history ledgers, or
preview eligibility. It must not execute a gate, post comments, run analysis,
generate tests, call providers, run mutation testing, or make CI blocking by
default.

Promotion means "allowed to review a future explicit policy change", not
"automatically promote this language or class". Preview evidence remains
visible and advisory unless a later implementation supplies and validates the
required receipts.

## Required Evidence

The report contract is satisfied only when implementation can show:

- `target/ripr/reports/preview-promotion-typescript-boundary-gap.json` and
  `.md`;
- `target/ripr/reports/preview-promotion-python-boundary-gap.json` and `.md`
  for the same command shape when Python is requested;
- `language`, `language_status`, `candidate_class`, `target_status`,
  `allowed_now`, `reason`, `required_evidence`, `supplied_evidence`,
  `missing_evidence`, `required_repairs`, `required_receipts`,
  `rollback_path`, `generated_ci_posture`, `input_artifacts`, `warnings`,
  `unknowns`, `non_goals`, and `limits_note` fields;
- default blocked behavior when promotion evidence is not supplied;
- explicit accounting for fixture corpus coverage, static-limit exclusions,
  false-positive review, recommendation calibration, optional mutation
  calibration, baseline behavior, waiver/suppression behavior, rollback path,
  and generated CI posture;
- language status remains `preview` unless a later explicit promotion policy
  changes it;
- no config, baseline, suppression, workflow, branch-protection, generated CI,
  history, source, gate, RIPR Zero, calibrated-confidence, or preview
  eligibility mutation.

## Inputs

The planned command is:

```text
ripr policy preview-promote \
  --language typescript \
  --class boundary_gap \
  --out target/ripr/reports/preview-promotion-typescript-boundary-gap.json \
  --out-md target/ripr/reports/preview-promotion-typescript-boundary-gap.md
```

Future implementations may add explicit evidence inputs, but missing evidence
must be represented as missing evidence, not silently inferred.

Input parameters:

| Input | Required? | Purpose |
| --- | --- | --- |
| Language | required | Preview language under review, initially `typescript` or `python`. |
| Candidate class | required | Evidence class under review, for example `boundary_gap`. |
| Evidence receipts | optional | Explicit future artifacts for fixture coverage, static limits, false-positive review, calibration, baseline behavior, waiver/suppression behavior, rollback, and generated CI posture. Missing receipts block promotion. |

## Outputs

The JSON report uses schema version `0.1`:

```json
{
  "schema_version": "0.1",
  "tool": "ripr",
  "kind": "preview_evidence_promotion_packet",
  "root": ".",
  "generated_at": "unix_ms:1778277000000",
  "language": "typescript",
  "language_status": "preview",
  "candidate_class": "boundary_gap",
  "target_status": "policy_eligible",
  "allowed_now": false,
  "reason": "preview promotion evidence not supplied",
  "required_evidence": [
    {
      "kind": "fixture_corpus_coverage",
      "required": true,
      "description": "Representative fixtures cover the candidate class and known static limits."
    },
    {
      "kind": "static_limit_exclusions",
      "required": true,
      "description": "Known static parser and language-adapter limits are excluded or labeled."
    },
    {
      "kind": "false_positive_review",
      "required": true,
      "description": "Maintainer-reviewed false-positive sample is documented for this language and class."
    },
    {
      "kind": "recommendation_calibration",
      "required": true,
      "description": "Same-class recommendation calibration supports policy eligibility."
    },
    {
      "kind": "mutation_calibration",
      "required": false,
      "description": "Optional runtime calibration exists for this language and class without being inferred from Rust."
    },
    {
      "kind": "baseline_behavior",
      "required": true,
      "description": "Baseline handling keeps preview debt visible and does not auto-adopt new preview findings."
    },
    {
      "kind": "waiver_suppression_behavior",
      "required": true,
      "description": "Waivers and suppressions preserve owner, reason, scope, and preview status."
    },
    {
      "kind": "rollback_path",
      "required": true,
      "description": "Manual rollback to advisory preview status is documented."
    },
    {
      "kind": "generated_ci_posture",
      "required": true,
      "description": "Generated CI remains advisory and non-blocking unless a later explicit gate mode is configured."
    }
  ],
  "supplied_evidence": [],
  "missing_evidence": [
    "fixture_corpus_coverage",
    "static_limit_exclusions",
    "false_positive_review",
    "recommendation_calibration",
    "baseline_behavior",
    "waiver_suppression_behavior",
    "rollback_path",
    "generated_ci_posture"
  ],
  "required_repairs": [
    "Supply explicit preview promotion evidence before policy eligibility review."
  ],
  "required_receipts": [
    "preview-promotion-typescript-boundary-gap.json",
    "preview-boundary report showing advisory language status",
    "fixture corpus coverage receipt for TypeScript boundary_gap",
    "false-positive review receipt for TypeScript boundary_gap",
    "recommendation-calibration receipt for TypeScript boundary_gap"
  ],
  "rollback_path": [
    "Keep TypeScript boundary_gap evidence advisory.",
    "Remove any manual preview promotion config if one was reviewed later.",
    "Regenerate policy operations and preview promotion packets after rollback."
  ],
  "generated_ci_posture": {
    "may_upload_artifact": true,
    "may_summarize_artifact": true,
    "may_fail_check": false,
    "may_post_comment": false,
    "may_mutate_config": false
  },
  "input_artifacts": [],
  "warnings": [],
  "unknowns": [],
  "non_goals": [
    "No actual promotion.",
    "No gate eligibility change.",
    "No RIPR Zero inclusion.",
    "No calibrated confidence.",
    "No CI blocking."
  ],
  "limits_note": "Read-only advisory preview promotion packet. Preview evidence remains visible and non-gating until a later explicit promotion policy is reviewed."
}
```

Field contract:

- `schema_version` - currently `"0.1"`.
- `tool` - always `"ripr"`.
- `kind` - always `"preview_evidence_promotion_packet"`.
- `language` - requested preview language.
- `language_status` - current status, initially `"preview"`.
- `candidate_class` - requested evidence class.
- `target_status` - requested future policy status. The packet may describe the
  requested status but must not apply it.
- `allowed_now` - false unless every required evidence item is supplied and a
  later implementation explicitly recognizes those receipts.
- `reason` - concise explanation for the decision.
- `required_evidence[]` - full evidence checklist for preview promotion.
- `supplied_evidence[]` - evidence receipts accepted by the packet.
- `missing_evidence[]` - required evidence still absent.
- `required_repairs[]` - concrete work before a maintainer can review
  promotion.
- `required_receipts[]` - artifacts reviewers should inspect before promotion.
- `rollback_path[]` - explicit return path to advisory preview status.
- `generated_ci_posture` - advisory CI permissions and hard denials.
- `input_artifacts[]` - future explicit evidence input status.
- `warnings[]` - malformed supplied inputs or target-language limitations.
- `unknowns[]` - unavailable context that must stay visible.
- `non_goals[]` - hard boundaries repeated in the packet.
- `limits_note` - read-only/manual-review/no-promotion boundary.

Markdown should fit in generated CI summaries and report packets:

```text
# RIPR Preview Evidence Promotion Packet

Language: typescript
Class: boundary_gap
Current status: preview
Target status: policy_eligible
Allowed now: no
Why: preview promotion evidence not supplied

## Missing Evidence

- fixture_corpus_coverage
- static_limit_exclusions
- false_positive_review
- recommendation_calibration
- baseline_behavior
- waiver_suppression_behavior
- rollback_path
- generated_ci_posture

## Required Receipts

- preview-boundary report showing advisory language status
- fixture corpus coverage receipt for TypeScript boundary_gap
- false-positive review receipt for TypeScript boundary_gap

## Generated CI Posture

- may upload artifact: yes
- may summarize artifact: yes
- may fail check: no
- may post comment: no
- may mutate config: no
```

## Default Rules

The packet must fail closed:

| Condition | Result |
| --- | --- |
| No promotion evidence supplied | `allowed_now = false`. |
| Language status is `preview` | Evidence remains advisory and non-gating. |
| Required evidence is missing | Promotion is blocked and missing evidence is listed. |
| Optional mutation calibration is missing | Promotion may still be blocked by other required evidence; missing optional mutation calibration must not imply calibrated confidence. |
| Generated CI wants to surface the packet | Artifact upload and summary are allowed; pass/fail authority, required checks, comment posting, config mutation, and default blocking remain denied. |

## Non-Goals

- No actual promotion.
- No gate eligibility change.
- No RIPR Zero inclusion.
- No calibrated confidence.
- No CI blocking.
- No config mutation.
- No baseline mutation or adoption.
- No suppression creation, deletion, or auto-expiry.
- No workflow, branch-protection, or generated CI mutation.
- No history append.
- No gate decision.
- No analyzer behavior changes.
- No evidence identity rewrites.
- No LSP or editor behavior changes.
- No generated tests.
- No provider calls.
- No mutation execution.
- No static evidence claim that runtime behavior was confirmed.

## Acceptance Examples

Default TypeScript packet:

- Command is `ripr policy preview-promote --language typescript --class boundary_gap`.
- `language_status = "preview"`.
- `allowed_now = false`.
- `reason = "preview promotion evidence not supplied"`.
- All required evidence kinds appear in `missing_evidence`.
- Generated CI posture allows upload and summary only.

Python packet with partial receipts:

- Supplied fixture coverage is listed in `supplied_evidence`.
- Missing false-positive review and recommendation calibration remain listed.
- `allowed_now = false`.
- Required repairs name the missing receipts.

Preview boundary protection:

- The packet repeats that preview evidence is not gate-eligible by default.
- The packet does not add RIPR Zero blocking debt.
- The packet does not claim calibrated confidence.
- The packet does not write config, baselines, suppressions, workflows, or
  history ledgers.

## Test Mapping

Follow-up implementation should include:

- Output builder tests for default blocked TypeScript and Python packets.
- Output builder tests for partial evidence receipts where missing required
  evidence still blocks promotion.
- JSON and Markdown shape tests for required/supplied/missing evidence,
  generated CI posture, rollback path, non-goals, and limits.
- Tests showing missing optional mutation calibration never creates calibrated
  confidence.
- Tests showing malformed supplied evidence becomes a warning and does not
  mutate or repair inputs.
- CLI option parsing tests for `--language`, `--class`, `--out`, and
  `--out-md`.
- CLI write-path tests proving only the requested JSON and Markdown reports are
  written.

## Implementation Mapping

This spec belongs to the focused Lane 2 tracker in
[Policy operations](../policy/POLICY_OPERATIONS.md).

Implementation should be split into later work items:

- `spec/preview-evidence-promotion-packet` defines this report contract.
- `policy/preview-promotion-packet-report` implements the read-only
  JSON/Markdown producer.
- `docs/policy-operator-workflow` explains how maintainers review preview
  promotion packets without changing preview defaults.
- `ci/policy-operations-advisory-projection` may surface preview promotion
  artifacts as advisory-only uploads and summaries.

No implementation PR may change analyzer truth, evidence identity, gate
semantics, LSP/editor behavior, provider behavior, mutation execution, source
files, generated tests, branch protection, generated CI defaults, config,
baselines, suppressions, history, RIPR Zero inclusion, calibrated confidence,
or preview-language eligibility to satisfy this spec.

## Metrics

The report makes these metrics available to later capability and trend
surfaces:

- `preview_promotion_packets`
- `preview_promotion_allowed`
- `preview_promotion_blocked`
- `preview_promotion_required_evidence`
- `preview_promotion_supplied_evidence`
- `preview_promotion_missing_evidence`
- `preview_promotion_warning_count`
- `preview_promotion_unknown_count`
