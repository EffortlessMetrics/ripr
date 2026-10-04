# RIPR intervention study preregistration

- schema: `ripr_intervention_study.v1`
- kind: `ripr_intervention_study`
- implementation_state: `preregistration_only`
- study_id: `study:ripr-intervention:iv01:matched-rust-boundary`
- protocol_version: `1`
- protocol_digest: `sha256:40b1c536cf1984f27c1375443ccc54882dd684bc0fc4106cbb345fb6735e26dd`
- parent_issue: #3751
- sequence: `IV01`
- task_series: `series:rust-boundary-missing-discriminator`
- repository: `EffortlessMetrics/ripr-swarm` @ `aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa`

## Conditions

- `control`: repository change plus ordinary source, tests, and tooling
- `ripr_assisted`: same task context plus the named RIPR evidence surface

## Assignment

- freeze: before first outcome or grader signal; assignment cannot change afterward
- `pair:boundary-oracle-a` task `task:boundary-oracle-a` order `control` then `ripr_assisted`
- `pair:boundary-oracle-b` task `task:boundary-oracle-b` order `ripr_assisted` then `control`

## Shared budget

- model: `model:study-operator-profile-v1`
- operator: `operator:independent-study-agent`
- runtime: `runtime:isolated-worktree`
- tools: `cargo`, `rustc`, `git`
- wall_clock_ms: 1800000
- token_budget: 50000
- retry_limit: 0

## RIPR-assisted evidence surface

- receipts:
  - `agent_receipt.v0.5`
- views:
  - `agent_seam_packet.v0.4`
- commands:
  - `ripr check --format json`
  - `ripr agent packet --json`

Control receives none of those RIPR outputs.

## Outcome axes

Axes are independently observable and non-compensating.

- `behavior_alignment`: test targets the exact changed behavior, owner, and sink
- `discriminating_proof_quality`: test would notice if the selected behavior were wrong
- `change_scope_and_churn`: edit stays inside the allowed test surface
- `review_findings_and_repair_burden`: independent review findings and follow-up repairs
- `completion_terminal_state`: typed terminal completeness of the attempt
- `time_and_bounded_resource_use`: elapsed effort and budget consumption
- `maintainability_readability`: independent readability and maintenance quality
- `false_confidence_or_unsupported_claim_events`: unsupported or false-success claims

## Adjudication

- graders: `grader:independent-a`
- rubric: `ripr-intervention-rubric.v1` version `1`
- condition identity is withheld from graders

## Stopping and claim ceiling

- fixed sample of 2 matched pairs
- stopping does not depend on a favorable interim estimate
- conclusions are bounded to this task series, model/operator profile, and intervention form
- a valid preregistration does not prove intervention value

## Non-claims

- `no_agent_execution`
- `no_repair_adjudication`
- `no_pilot_result`
- `no_intervention_value`
- `no_generic_ab_platform`
