# Documentation System

`ripr` uses Diataxis so docs answer the reader's immediate problem instead of
mixing tutorials, references, and design arguments in one place.

## Tutorials

Tutorials help a new user succeed once.

Current and planned tutorial docs:

- [Quickstart](QUICKSTART.md) - first-hour paths for VS Code, CI, CLI, and
  agent or reviewer handoff
- README quick start
- future first-extension-install walkthrough
- future first-fixture walkthrough

## How-To Guides

How-to guides solve concrete tasks.

Current how-to docs:

- [Contributing](../CONTRIBUTING.md)
- [Testing](TESTING.md)
- [CI strategy](CI.md)
- [Security policy](../SECURITY.md)
- [Repository settings](REPO_SETTINGS.md)
- [Fix CI shape failures](how-to/fix-ci-shape-failures.md)
- [Run Codex Goals](how-to/run-codex-goals.md)
- [PR automation](PR_AUTOMATION.md)
- [Roll out Factory Droid review](how-to/roll-out-droid.md)
- [Dogfooding](DOGFOODING.md)
- [Targeted test workflow](TARGETED_TEST_WORKFLOW.md)
- [Targeted test boundary-gap case study](case-studies/TARGETED_TEST_BOUNDARY_GAP.md)
- [Agent workflows](AGENT_WORKFLOWS.md)
- [LLM operator guide](LLM_OPERATOR_GUIDE.md)
- [Recommendation calibration](RECOMMENDATION_CALIBRATION.md)
- [Calibrated gate policy](CALIBRATED_GATE_POLICY.md)
- [RIPR blocking readiness](BLOCKING_READINESS.md)
- [Baseline ledger workflow](BASELINE_LEDGER_WORKFLOW.md)
- [RIPR Zero reporting workflow](RIPR_ZERO_REPORTING_WORKFLOW.md)
- [PR evidence ledger workflow](PR_EVIDENCE_LEDGER_WORKFLOW.md)
- [Test-oracle assistant workflow](TEST_ORACLE_ASSISTANT_WORKFLOW.md)
- [Test-oracle assistant proof report](TEST_ORACLE_ASSISTANT_PROOF_REPORT.md)
- [First useful action workflow](FIRST_USEFUL_ACTION_WORKFLOW.md)
- [Assistant loop health workflow](ASSISTANT_LOOP_HEALTH_WORKFLOW.md)
- [Assistant loop health proposal](ASSISTANT_LOOP_HEALTH_PROPOSAL.md)
- [PR review front panel proposal](PR_REVIEW_FRONT_PANEL_PROPOSAL.md)
- [Lane 3 editor/LSP tracker](lanes/LANE_3_EDITOR_LSP.md)
- [Release](RELEASE.md)
- [Installation verification](INSTALLATION_VERIFICATION.md)
- [Publishing](PUBLISHING.md)
- [Editor extension](EDITOR_EXTENSION.md)
- [Editor evidence workflow](EDITOR_EVIDENCE_WORKFLOW.md)
- [Editor evidence UX](EDITOR_EVIDENCE_UX.md)
- [Server provisioning](SERVER_PROVISIONING.md)
- [Server binary release](RELEASE_BINARIES.md)
- [Marketplace release](RELEASE_MARKETPLACE.md)
- [Open VSX](OPENVSX.md)

## Reference

Reference docs define stable commands, schemas, config, and enum meanings.

Current reference docs:

- [Output schema](OUTPUT_SCHEMA.md)
- [Static exposure model](STATIC_EXPOSURE_MODEL.md)
- [Configuration](CONFIGURATION.md)
- [Badge policy](BADGE_POLICY.md)
- [Defaults-first adoption](specs/RIPR-SPEC-0009-defaults-first-adoption.md)
- [Spec-test-code traceability](SPEC_TEST_CODE.md)
- [Spec format](SPEC_FORMAT.md)
- [Fixture contracts](../fixtures/README.md)
- [Defaults-first example corpus](../fixtures/EXAMPLE_CORPUS.md)
- [Calibration corpus index](../fixtures/CALIBRATION_CORPUS.md)
- [Recommendation calibration](RECOMMENDATION_CALIBRATION.md)
- [Calibrated gate policy](CALIBRATED_GATE_POLICY.md)
- [RIPR blocking readiness](BLOCKING_READINESS.md)
- [Baseline ledger workflow](BASELINE_LEDGER_WORKFLOW.md)
- [RIPR Zero reporting workflow](RIPR_ZERO_REPORTING_WORKFLOW.md)
- [PR evidence ledger workflow](PR_EVIDENCE_LEDGER_WORKFLOW.md)
- [Test-oracle assistant workflow](TEST_ORACLE_ASSISTANT_WORKFLOW.md)
- [Test-oracle assistant proof report](TEST_ORACLE_ASSISTANT_PROOF_REPORT.md)
- [First useful action workflow](FIRST_USEFUL_ACTION_WORKFLOW.md)
- [Assistant loop health workflow](ASSISTANT_LOOP_HEALTH_WORKFLOW.md)
- [Assistant loop health proposal](ASSISTANT_LOOP_HEALTH_PROPOSAL.md)
- [PR review front panel proposal](PR_REVIEW_FRONT_PANEL_PROPOSAL.md)
- [Test taxonomy](TEST_TAXONOMY.md)
- [Engineering rules](ENGINEERING.md)
- [File policy](FILE_POLICY.md)
- [No-panic policy](NO_PANIC_POLICY.md)
- [Policy allowlists](POLICY_ALLOWLISTS.md)
- [Changelog policy](CHANGELOG_POLICY.md)
- [Capability matrix](CAPABILITY_MATRIX.md)
- [No-panic semantic allowlist](NO_PANIC_SEMANTIC_ALLOWLIST.md)
- [Droid rollout checklist](agent-context/droid-rollout.md)
- [CI verification ladder](ci/verification-ladder.md)
- [CI current state](ci/current-state.md)
- [CI LEM budgeting](ci/lem-budgeting.md)
- [CI labels](ci/labels.md)
- [CI cost and verification policy](ci/cost-and-verification-policy.md)
- [MSRV 1.95 rollout plan](ci/ripr-rollout-plan.md)
- [Rust 1.95 compatibility audit](ci/msrv-1.95-audit.md)

Planned reference docs:

- SARIF output reference
- LSP diagnostic code reference

Templates:

- [ADR template](templates/ADR_TEMPLATE.md)
- [Spec template](templates/SPEC_TEMPLATE.md)

## Explanation

Explanation docs record why the product and architecture are shaped this way.

Current explanation docs:

- [Charter](CHARTER.md)
- [Architecture](ARCHITECTURE.md)
- [Roadmap](ROADMAP.md)
- [Implementation plan](IMPLEMENTATION_PLAN.md)
- [Implementation campaigns](IMPLEMENTATION_CAMPAIGNS.md)
- [Codex Goals](CODEX_GOALS.md)
- [Scoped PR contract](SCOPED_PR_CONTRACT.md)
- [PR automation](PR_AUTOMATION.md)
- [Metrics](METRICS.md)
- [Capability matrix](CAPABILITY_MATRIX.md)
- [Learnings](LEARNINGS.md)
- [Friction log](FRICTION_LOG.md)
- [Deferred decisions](DEFERRED.md)
- [Agent handoff protocol](reference/AGENT_HANDOFF_PROTOCOL.md)
- [Handoff ledger](handoffs/README.md)
- [ADRs](adr/)
- [Specs](specs/)
- [Agent workflows](AGENT_WORKFLOWS.md)
- [Agent dispatch workflow](AGENT_DISPATCH_WORKFLOW.md)
- [Editor agent integration](EDITOR_AGENT_INTEGRATION.md)
- [Editor evidence workflow](EDITOR_EVIDENCE_WORKFLOW.md)
- [Editor evidence UX](EDITOR_EVIDENCE_UX.md)
- [LLM operator guide](LLM_OPERATOR_GUIDE.md)
- [Recommendation calibration](RECOMMENDATION_CALIBRATION.md)
- [Calibrated gate policy](CALIBRATED_GATE_POLICY.md)
- [RIPR blocking readiness](BLOCKING_READINESS.md)
- [Baseline ledger workflow](BASELINE_LEDGER_WORKFLOW.md)
- [RIPR Zero reporting workflow](RIPR_ZERO_REPORTING_WORKFLOW.md)
- [PR evidence ledger workflow](PR_EVIDENCE_LEDGER_WORKFLOW.md)
- [Test-oracle assistant proof report](TEST_ORACLE_ASSISTANT_PROOF_REPORT.md)
- [First useful action workflow](FIRST_USEFUL_ACTION_WORKFLOW.md)
- [Assistant loop health workflow](ASSISTANT_LOOP_HEALTH_WORKFLOW.md)
- [Assistant loop health proposal](ASSISTANT_LOOP_HEALTH_PROPOSAL.md)
- [PR review front panel proposal](PR_REVIEW_FRONT_PANEL_PROPOSAL.md)
- [Lane 3 editor/LSP tracker](lanes/LANE_3_EDITOR_LSP.md)

## README Rule

The README is the front door. It should stay problem-first and include:

- what `ripr` is
- what question it answers
- where it fits against coverage and mutation testing
- quick start
- current capability state
- important metrics and engineering status
- links to the deeper docs

Avoid turning the README into the full roadmap or full schema reference.

## Index Check

Run:

```bash
cargo xtask check-doc-index
```

The check verifies that spec and ADR indexes list current files and that README
and this documentation map still point at the active planning, metrics, spec,
ADR, and PR automation docs.
