# RIPR-SPEC-0216: Matched RIPR intervention-study preregistration

Status: proposed

Owner: product-eval

Created: 2026-09-29

Linked proposal:

- None yet

Linked ADRs:

- None yet

Linked plan:

- None yet

Linked issues:

- #4649 — preregister the matched RIPR intervention study (this slice)
- #3751 — parent intervention-value study
- #4652 — matched-condition runner (held; consumes this protocol)
- #4653 — independent adjudication (held; consumes this protocol)
- #4654 — first bounded pilot report (held)

Linked PRs:

- None yet

Support-tier impact:

- None. This spec freezes a study protocol. It does not promote a language,
  editor surface, gate, or public support claim.
- Claim boundaries remain governed by
  [support tiers](../status/SUPPORT_TIERS.md).

Policy impact:

- Register this spec in `policy/doc-artifacts.toml` and
  `.ripr/traceability.toml`.
- Publish `schemas/ripr/ripr-intervention-study.schema.json` as a
  preregistration-only contract.
- No new crates, binaries, dependencies, network allowlist rows, process
  spawners, LSP/MCP write tools, or support-tier changes.

## Problem

Intervention value is easy to collapse into judgment validity or transaction
usability. A matched comparison of RIPR-assisted repair against controls
needs a frozen protocol before outcomes exist. Without that freeze, later
slices can change assignment after a failure, give one condition extra
budget or RIPR output, drop timeouts from the denominator, or stop on a
favorable interim estimate.

## Behavior

`RiprInterventionStudyV1` is the single semantic object for
`ripr_intervention_study.v1`. It records:

```text
study identity and protocol version
exact repository / task-series identity
eligibility and exclusion rules
matched task strata and selected tasks
condition = control | ripr_assisted
assignment and counterbalancing law
shared model/operator/runtime/tool/resource budget
named RIPR evidence surface for the assisted condition only
contamination / prior-exposure controls
attempt and retry policy
primary outcome axes
independent adjudication plan
missing / invalid / instrument outcome cells
stopping rule
planned analysis and claim ceiling
```

Validation rejects the issue #4649 falsifiers. JSON and Markdown are
deterministic projections of one sealed object. The matched-condition runner
and adjudicator consume this protocol without reinterpreting assignment,
budgets, leakage rules, or axes.

`implementation_state` is `preregistration_only`. This document does not
execute agents, adjudicate repairs, or publish a pilot result. A valid
preregistration does not prove intervention value.

## Required Evidence

- One published JSON Schema pins `schema_version`
  `ripr_intervention_study.v1`, `kind` `ripr_intervention_study`, and
  `implementation_state` `preregistration_only`.
- A domain validator owns the study laws and stable rejection codes.
- Fixtures contain one sealed valid protocol and the ten falsifiers.
- JSON and Markdown projections of the valid protocol are byte-stable for
  the same object.
- Outcome axes remain non-compensating; invalid attempts stay in the
  denominator.

## Non-Goals

- Agent execution, repair implementation, or grader execution (#4652/#4653)
- Pilot results, intervention-value estimates, or publication (#4654)
- A generic A/B testing service, product telemetry, or model ranking
- Retrospective protocol fitting after outcomes are known
- Support-tier promotion, CI reduction, or release membership

## Acceptance Examples

1. The example IV01 protocol validates and seals a `sha256:` digest over the
   canonical payload. The digest binds the `serde_json` serialization of the
   v1 struct in field-declaration order, not a separate canonical-JSON form:
   reordering or re-tagging fields changes sealed digests and therefore
   requires a schema version change rather than a silent refactor. The
   published JSON schema pins identity (`schema_version`, `kind`,
   `implementation_state`) and shape; the study laws are owned by the domain
   validator.
2. Rendering JSON then parsing it yields the same object; Markdown from that
   object is unchanged on a second render.
3. Assignment that may change after an outcome is rejected as
   `assignment_after_outcome`.
4. Extra tokens, time, tools, or retries on one condition are rejected as
   `unequal_condition_budgets`.
5. A control surface that may read RIPR outputs is rejected as
   `control_can_read_ripr_outputs`.
6. Undeclared extra repository context on the assisted condition is rejected
   as `assisted_undeclared_extra_context`.
7. Replacing a task after a difficult failure is rejected as
   `task_replacement_after_failure`.
8. Unequal retry policy is rejected as `retry_only_in_weaker_condition`.
9. Dropping timeouts or invalid attempts is rejected as
   `drop_timeouts_or_invalid_from_denominator`.
10. Stopping on a favorable interim estimate is rejected as
    `stopping_on_favorable_interim`.
11. Missing grader identity or rubric version is rejected as
    `grader_or_rubric_absent`.
12. Mutating a started protocol under the same `study_id` is rejected as
    `protocol_mutation_requires_new_study_id`.

## Test Mapping

- `crates/ripr/src/domain/intervention_study.rs::tests::example_preregistration_satisfies_study_laws`
- `crates/ripr/src/domain/intervention_study.rs::tests::assignment_after_outcome_is_rejected`
- `crates/ripr/src/domain/intervention_study.rs::tests::unequal_condition_budgets_are_rejected`
- `crates/ripr/src/domain/intervention_study.rs::tests::control_reading_ripr_outputs_is_rejected`
- `crates/ripr/src/domain/intervention_study.rs::tests::assisted_undeclared_context_is_rejected`
- `crates/ripr/src/domain/intervention_study.rs::tests::task_replacement_after_failure_is_rejected`
- `crates/ripr/src/domain/intervention_study.rs::tests::retry_only_in_weaker_condition_is_rejected`
- `crates/ripr/src/domain/intervention_study.rs::tests::dropping_timeouts_or_invalid_attempts_is_rejected`
- `crates/ripr/src/domain/intervention_study.rs::tests::stopping_on_favorable_interim_is_rejected`
- `crates/ripr/src/domain/intervention_study.rs::tests::missing_grader_or_rubric_is_rejected`
- `crates/ripr/src/domain/intervention_study.rs::tests::protocol_mutation_after_first_attempt_requires_new_study_id`
- `crates/ripr/src/output/intervention_study.rs::tests::json_and_markdown_derive_from_one_sealed_object`
- `crates/ripr/src/output/intervention_study.rs::tests::committed_valid_fixture_matches_sealed_example`
- `crates/ripr/src/output/intervention_study.rs::tests::corpus_falsifiers_are_rejected_with_preregistered_codes`

## Implementation Mapping

- `crates/ripr/src/domain/intervention_study.rs` — protocol object and study-law validator
- `crates/ripr/src/output/intervention_study.rs` — digest sealing, JSON, Markdown
- `schemas/ripr/ripr-intervention-study.schema.json` — published wire shape
- `fixtures/intervention-study/` — valid protocol, expected Markdown, falsifier corpus

## Metrics

- `intervention_study_valid_protocols` — count of sealed valid protocol fixtures
- `intervention_study_falsifiers_rejected` — count of required falsifier codes covered
