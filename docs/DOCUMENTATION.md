# Documentation System

`ripr` uses Diataxis so docs answer the reader's immediate problem instead of
mixing tutorials, references, and design arguments in one place.

## Tutorials

Tutorials help a new user succeed once.

Current and planned tutorial docs:

- [Quickstart](QUICKSTART.md) - first-hour paths for VS Code, CI, CLI, and
  agent or reviewer handoff
- [First successful PR workflow](FIRST_PR_WORKFLOW.md) - one Rust PR,
  one repairable gap, one focused proof, one receipt
- [First successful PR demo](demo/first-successful-pr.md) - fixture-backed
  boundary, output-contract, no-action, and blocked start-here cases
- [Editor install to first PR](EDITOR_INSTALL_TO_FIRST_PR.md) - VS Code
  install/open through setup diagnosis, repair, receipt, and first-pr packet
- [Editor actionable gap queue](EDITOR_ACTIONABLE_GAP_QUEUE.md) - VS Code
  current repair queue, repair packet, repo gap map, and no-action states
- [Editor first run to first receipt](EDITOR_FIRST_RUN_TO_FIRST_RECEIPT.md) -
  install/open, diagnose setup, repair one Rust gap, verify, receipt, refresh
- [Editor first-pr bridge workflow](EDITOR_FIRST_PR_BRIDGE_WORKFLOW.md) -
  local handoff from receipt and refresh to first-pr `start-here` packet
- [RIPR swarm human workflow](RIPR_SWARM_HUMAN_WORKFLOW.md) - one bounded
  actionable-gap repair attempt from packet to receipt and outcome
- [Start-here surface convergence proposal](proposals/RIPR-PROP-0011-start-here-surface-convergence.md) -
  cross-surface plan for making editor, CLI, PR/CI, receipts, and preview
  promotion use the same safe next-action unit
- [Editor adoption assurance proposal](proposals/RIPR-PROP-0012-editor-adoption-assurance.md) -
  Lane 3 plan for compatibility, active root, multi-root, and fail-closed
  first-use diagnosis
- [Editor actionable gap queue proposal](proposals/RIPR-PROP-0013-editor-actionable-gap-queue.md) -
  Lane 3 plan for projecting existing `actionable-gaps` artifacts as a
  read-only editor repair queue
- README quick start
- future first-extension-install walkthrough
- future first-fixture walkthrough

## How-To Guides

How-to guides solve concrete tasks.

Current how-to docs:

- [Contributing](../CONTRIBUTING.md)
- [Testing](TESTING.md)
- [CI strategy](CI.md)
- [Source-of-truth control plane](source-of-truth/README.md)
- [Security policy](../SECURITY.md)
- [Repository settings](REPO_SETTINGS.md)
- [Swarm development](swarm-development.md)
- [Fix CI shape failures](how-to/fix-ci-shape-failures.md)
- [Run Codex Goals](how-to/run-codex-goals.md)
- [PR automation](PR_AUTOMATION.md)
- [Merge freshness and watcher policy](MERGE_WATCH_POLICY.md)
- [Generated evidence discipline](GENERATED_EVIDENCE.md)
- [Roll out Factory Droid review](how-to/roll-out-droid.md)
- [Dogfooding](DOGFOODING.md)
- [Bun UB TypeScript preview runbook](BUN_UB_TYPESCRIPT_PREVIEW_RUNBOOK.md)
- [Targeted test workflow](TARGETED_TEST_WORKFLOW.md)
- [RIPR swarm human workflow](RIPR_SWARM_HUMAN_WORKFLOW.md)
- [Language adapter preview workflow](LANGUAGE_ADAPTER_PREVIEW.md)
- [Python repair routing proposal](proposals/RIPR-PROP-0017-python-repair-routing-lane.md)
- [Static limits](STATIC_LIMITS.md)
- [Targeted test boundary-gap case study](case-studies/TARGETED_TEST_BOUNDARY_GAP.md)
- [Agent workflows](AGENT_WORKFLOWS.md)
- [LLM operator guide](LLM_OPERATOR_GUIDE.md)
- [Recommendation calibration](RECOMMENDATION_CALIBRATION.md)
- [Calibrated gate policy](CALIBRATED_GATE_POLICY.md)
- [RIPR blocking readiness](BLOCKING_READINESS.md)
- [Baseline ledger workflow](BASELINE_LEDGER_WORKFLOW.md)
- [RIPR Zero reporting workflow](RIPR_ZERO_REPORTING_WORKFLOW.md)
- [PR evidence ledger workflow](PR_EVIDENCE_LEDGER_WORKFLOW.md)
- [Policy operations workflow](POLICY_OPERATIONS_WORKFLOW.md)
- [Test-oracle assistant workflow](TEST_ORACLE_ASSISTANT_WORKFLOW.md)
- [Test-oracle assistant proof report](TEST_ORACLE_ASSISTANT_PROOF_REPORT.md)
- [First useful action workflow](FIRST_USEFUL_ACTION_WORKFLOW.md)
- [Assistant loop health workflow](ASSISTANT_LOOP_HEALTH_WORKFLOW.md)
- [Assistant loop health proposal](ASSISTANT_LOOP_HEALTH_PROPOSAL.md)
- [PR review front panel workflow](PR_REVIEW_FRONT_PANEL_WORKFLOW.md)
- [PR review front panel proposal](PR_REVIEW_FRONT_PANEL_PROPOSAL.md)
- [Report packet index workflow](REPORT_PACKET_INDEX_WORKFLOW.md)
- [Report packet index proposal](REPORT_PACKET_INDEX_PROPOSAL.md)
- [PR inline comment publisher workflow](PR_INLINE_COMMENT_PUBLISHER_WORKFLOW.md)
- [PR inline comment publisher proposal](PR_INLINE_COMMENT_PUBLISHER_PROPOSAL.md)
- [Lane tracker source-of-truth model](lanes/README.md)
- [Lane 1 evidence spine tracker](lanes/LANE_1_EVIDENCE_SPINE.md)
- [Lane 1 evidence accuracy tracker](lanes/LANE_1_EVIDENCE_ACCURACY.md)
- [Lane 1 evidence quality leadership tracker](lanes/LANE_1_EVIDENCE_QUALITY_LEADERSHIP.md)
- [Lane 1 user-visible output evidence tracker](lanes/LANE_1_USER_VISIBLE_OUTPUT_EVIDENCE.md)
- [Lane 1 finding alignment burn-down tracker](lanes/LANE_1_FINDING_ALIGNMENT_BURNDOWN.md)
- [Lane 1 finding alignment burn-down closeout](handoffs/2026-05-22-lane1-finding-alignment-burndown-closeout.md)
- [Lane 1 value resolution audit fixes tracker](lanes/LANE_1_VALUE_RESOLUTION_AUDIT_FIXES.md)
- [Lane 1 evidence-to-repair closeout audit](handoffs/2026-05-27-lane1-evidence-to-repair-closeout-audit.md)
- [Lane 1 TypeScript preview completion tracker](lanes/LANE_1_TYPESCRIPT_PREVIEW_COMPLETION.md)
- [TypeScript enablement lane](lanes/TYPESCRIPT_ENABLEMENT.md)
- [TypeScript preview completion closeout](handoffs/2026-05-30-typescript-preview-completion-closeout.md)
- [Bun UB TypeScript preview closeout](handoffs/2026-06-03-bun-ub-typescript-preview-closeout.md)
- [Lane 1 language-aware placement navigation closeout](handoffs/2026-06-03-lane1-language-aware-placement-navigation-closeout.md)
- [Lane 1 large-repo runtime completeness closeout](handoffs/2026-06-03-lane1-large-repo-runtime-completeness-closeout.md)
- [Python repair routing usable-alpha closeout](handoffs/2026-05-31-python-repair-routing-usable-alpha-closeout.md)
- [Lane 2 policy readiness tracker](policy/POLICY_READINESS.md)
- [Lane 2 policy operations tracker](policy/POLICY_OPERATIONS.md)
- [Preview promotion criteria](policy/PREVIEW_PROMOTION_CRITERIA.md)
- [Lane 3 editor/LSP tracker](lanes/LANE_3_EDITOR_LSP.md)
- [Lane 4 PR / CI review cockpit tracker](lanes/LANE_4_PR_CI_REVIEW.md)
- [Release](RELEASE.md)
- [Installation verification](INSTALLATION_VERIFICATION.md)
- [First successful PR workflow](FIRST_PR_WORKFLOW.md)
- [First successful PR demo](demo/first-successful-pr.md)
- [Start-here convergence receipts](handoffs/2026-05-22-start-here-surface-convergence-receipts.md)
- [Start-here convergence closeout](handoffs/2026-05-22-start-here-surface-convergence-closeout.md)
- [Publishing](PUBLISHING.md)
- [Editor extension](EDITOR_EXTENSION.md)
- [Editor install to first PR](EDITOR_INSTALL_TO_FIRST_PR.md)
- [Editor first run to first receipt](EDITOR_FIRST_RUN_TO_FIRST_RECEIPT.md)
- [Editor actionable gap queue](EDITOR_ACTIONABLE_GAP_QUEUE.md)
- [Editor first-pr bridge workflow](EDITOR_FIRST_PR_BRIDGE_WORKFLOW.md)
- [RIPR swarm human workflow](RIPR_SWARM_HUMAN_WORKFLOW.md)
- [Editor gap cockpit workflow](EDITOR_GAP_COCKPIT_WORKFLOW.md)
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
- [Support tiers](status/SUPPORT_TIERS.md)
- [Repo tracking model](REPO_TRACKING_MODEL.md)
- [Context system](agent-context/CONTEXT_SYSTEM.md)
- [Language adapter preview workflow](LANGUAGE_ADAPTER_PREVIEW.md)
- [Bun UB TypeScript preview runbook](BUN_UB_TYPESCRIPT_PREVIEW_RUNBOOK.md)
- [Python repair routing proposal](proposals/RIPR-PROP-0017-python-repair-routing-lane.md)
- [Static limits](STATIC_LIMITS.md)
- [Badge policy](BADGE_POLICY.md)
- [Badge adoption](BADGE_ADOPTION.md)
- [Generated evidence discipline](GENERATED_EVIDENCE.md)
- [Verification](VERIFICATION.md)
- [Verification contracts](verification/README.md)
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
- [Policy operations workflow](POLICY_OPERATIONS_WORKFLOW.md)
- [Test-oracle assistant workflow](TEST_ORACLE_ASSISTANT_WORKFLOW.md)
- [Test-oracle assistant proof report](TEST_ORACLE_ASSISTANT_PROOF_REPORT.md)
- [First useful action workflow](FIRST_USEFUL_ACTION_WORKFLOW.md)
- [Assistant loop health workflow](ASSISTANT_LOOP_HEALTH_WORKFLOW.md)
- [Assistant loop health proposal](ASSISTANT_LOOP_HEALTH_PROPOSAL.md)
- [PR review front panel workflow](PR_REVIEW_FRONT_PANEL_WORKFLOW.md)
- [PR review front panel proposal](PR_REVIEW_FRONT_PANEL_PROPOSAL.md)
- [Report packet index workflow](REPORT_PACKET_INDEX_WORKFLOW.md)
- [Report packet index proposal](REPORT_PACKET_INDEX_PROPOSAL.md)
- [PR inline comment publisher workflow](PR_INLINE_COMMENT_PUBLISHER_WORKFLOW.md)
- [PR inline comment publisher proposal](PR_INLINE_COMMENT_PUBLISHER_PROPOSAL.md)
- [Test taxonomy](TEST_TAXONOMY.md)
- [Engineering rules](ENGINEERING.md)
- [File policy](FILE_POLICY.md)
- [No-panic policy](NO_PANIC_POLICY.md)
- [Policy allowlists](POLICY_ALLOWLISTS.md)
- [Preview promotion criteria](policy/PREVIEW_PROMOTION_CRITERIA.md)
- [Changelog policy](CHANGELOG_POLICY.md)
- [Capability matrix](CAPABILITY_MATRIX.md)
- [Presentation text evidence](specs/RIPR-SPEC-0043-presentation-text-evidence.md)
- [Finding-to-gap alignment](specs/RIPR-SPEC-0045-finding-to-gap-alignment.md)
- [No-panic semantic allowlist](NO_PANIC_SEMANTIC_ALLOWLIST.md)
- [Droid rollout checklist](agent-context/droid-rollout.md)
- [CI verification ladder](ci/verification-ladder.md)
- [CI current state](ci/current-state.md)
- [CI LEM budgeting](ci/lem-budgeting.md)
- [CI labels](ci/labels.md)
- [CI cost and verification policy](ci/cost-and-verification-policy.md)
- [MSRV 1.95 rollout plan](ci/ripr-rollout-plan.md)
- [Rust 1.95 compatibility audit](ci/msrv-1.95-audit.md)
- [Rust 1.95 / 0.6.0 release shaping](ci/rust-1.95-quality-rollout.md)
- [Test evidence lanes](ci/test-evidence-lanes.md)
- [ripr / mutation boundary](ci/ripr-mutation-boundary.md)
- [Rust 1.95 consistency audit](ci/rust-1.95-consistency-audit.md)

Planned reference docs:

- SARIF output reference
- LSP diagnostic code reference

Templates:

- [ADR template](templates/ADR_TEMPLATE.md)
- [Spec template](templates/SPEC_TEMPLATE.md)
- [Proposal template](templates/PROPOSAL_TEMPLATE.md)
- [Handoff template](templates/HANDOFF_TEMPLATE.md)
- [Closeout template](templates/CLOSEOUT_TEMPLATE.md)
- [Plan item template](templates/PLAN_ITEM_TEMPLATE.md)

## Explanation

Explanation docs record why the product and architecture are shaped this way.

Current explanation docs:

- [Charter](CHARTER.md)
- [Architecture](ARCHITECTURE.md)
- [Roadmap](ROADMAP.md)
- [Repo tracking model](REPO_TRACKING_MODEL.md)
- [Repo context system](agent-context/CONTEXT_SYSTEM.md)
- [Implementation plans index](../plans/README.md)
- [Implementation plan](IMPLEMENTATION_PLAN.md)
- [Implementation campaigns](IMPLEMENTATION_CAMPAIGNS.md)
- [Campaign 27 plans](../plans/campaign-27/README.md)
- [Proposals](proposals/README.md)
- [Python repair routing proposal](proposals/RIPR-PROP-0017-python-repair-routing-lane.md)
- [Python repair routing plan](../plans/python-repair-routing/implementation-plan.md)
- [Python repair routing usable-alpha closeout](handoffs/2026-05-31-python-repair-routing-usable-alpha-closeout.md)
- [TypeScript preview completion plan](../plans/typescript-preview-completion/implementation-plan.md)
- [TypeScript enablement lane](lanes/TYPESCRIPT_ENABLEMENT.md)
- [TypeScript preview completion closeout](handoffs/2026-05-30-typescript-preview-completion-closeout.md)
- [Bun UB TypeScript preview closeout](handoffs/2026-06-03-bun-ub-typescript-preview-closeout.md)
- [Lane 1 language-aware placement navigation closeout](handoffs/2026-06-03-lane1-language-aware-placement-navigation-closeout.md)
- [User-visible output evidence proposal](proposals/RIPR-PROP-0005-user-visible-output-evidence.md)
- [Lane 1 finding alignment burn-down plan](../plans/lane1-finding-alignment-burndown/implementation-plan.md)
- [Lane 1 value resolution audit fixes plan](../plans/lane1-value-resolution-audit-fixes/implementation-plan.md)
- [Lane 1 evidence-to-repair closeout audit](handoffs/2026-05-27-lane1-evidence-to-repair-closeout-audit.md)
- [Adoption integration cleanup plan](../plans/adoption-integration-cleanup/implementation-plan.md)
  - closed historical cleanup rail
- [Editor first-pr bridge proposal](proposals/RIPR-PROP-0010-editor-first-pr-bridge.md)
- [Start-here surface convergence proposal](proposals/RIPR-PROP-0011-start-here-surface-convergence.md)
- [Start-here surface convergence plan](../plans/start-here-surface-convergence/implementation-plan.md)
- [Editor adoption assurance proposal](proposals/RIPR-PROP-0012-editor-adoption-assurance.md)
- [Editor adoption assurance plan](../plans/editor-adoption-assurance/implementation-plan.md)
- [Editor actionable gap queue](EDITOR_ACTIONABLE_GAP_QUEUE.md)
- [Editor actionable gap queue proposal](proposals/RIPR-PROP-0013-editor-actionable-gap-queue.md)
- [Editor actionable gap queue plan](../plans/editor-actionable-gap-queue/implementation-plan.md)
- [Source-of-truth control plane proposal](proposals/RIPR-PROP-0015-source-of-truth-control-plane.md)
- [Source-of-truth stack spec](specs/RIPR-SPEC-0060-source-of-truth-stack.md)
- [Source-of-truth control plane closeout](handoffs/2026-05-23-source-of-truth-control-plane-closeout.md)
- [Codex Goals](CODEX_GOALS.md)
- [Scoped PR contract](SCOPED_PR_CONTRACT.md)
- [PR automation](PR_AUTOMATION.md)
- [Merge freshness and watcher policy](MERGE_WATCH_POLICY.md)
- [Metrics](METRICS.md)
- [Capability matrix](CAPABILITY_MATRIX.md)
- [Support tiers](status/SUPPORT_TIERS.md)
- [Learnings](LEARNINGS.md)
- [Friction log](FRICTION_LOG.md)
- [Deferred decisions](DEFERRED.md)
- [Agent handoff protocol](reference/AGENT_HANDOFF_PROTOCOL.md)
- [Handoff ledger](handoffs/README.md)
- [ADRs](adr/)
- [Specs](specs/)
- [Agent workflows](AGENT_WORKFLOWS.md)
- [RIPR swarm human workflow](RIPR_SWARM_HUMAN_WORKFLOW.md)
- [First successful PR workflow](FIRST_PR_WORKFLOW.md)
- [Agent dispatch workflow](AGENT_DISPATCH_WORKFLOW.md)
- [Editor agent integration](EDITOR_AGENT_INTEGRATION.md)
- [Editor gap cockpit workflow](EDITOR_GAP_COCKPIT_WORKFLOW.md)
- [Editor evidence workflow](EDITOR_EVIDENCE_WORKFLOW.md)
- [Editor evidence UX](EDITOR_EVIDENCE_UX.md)
- [Editor first-pr bridge plan](../plans/editor-first-pr-bridge/implementation-plan.md)
- [Editor adoption assurance plan](../plans/editor-adoption-assurance/implementation-plan.md)
- [Adoption integration cleanup rails](../plans/adoption-integration-cleanup/README.md)
  - closed historical cleanup rail
- [LLM operator guide](LLM_OPERATOR_GUIDE.md)
- [Recommendation calibration](RECOMMENDATION_CALIBRATION.md)
- [Calibrated gate policy](CALIBRATED_GATE_POLICY.md)
- [RIPR blocking readiness](BLOCKING_READINESS.md)
- [Baseline ledger workflow](BASELINE_LEDGER_WORKFLOW.md)
- [RIPR Zero reporting workflow](RIPR_ZERO_REPORTING_WORKFLOW.md)
- [PR evidence ledger workflow](PR_EVIDENCE_LEDGER_WORKFLOW.md)
- [Policy operations workflow](POLICY_OPERATIONS_WORKFLOW.md)
- [Test-oracle assistant proof report](TEST_ORACLE_ASSISTANT_PROOF_REPORT.md)
- [First useful action workflow](FIRST_USEFUL_ACTION_WORKFLOW.md)
- [Assistant loop health workflow](ASSISTANT_LOOP_HEALTH_WORKFLOW.md)
- [Assistant loop health proposal](ASSISTANT_LOOP_HEALTH_PROPOSAL.md)
- [PR review front panel workflow](PR_REVIEW_FRONT_PANEL_WORKFLOW.md)
- [PR review front panel proposal](PR_REVIEW_FRONT_PANEL_PROPOSAL.md)
- [Report packet index workflow](REPORT_PACKET_INDEX_WORKFLOW.md)
- [Report packet index proposal](REPORT_PACKET_INDEX_PROPOSAL.md)
- [PR inline comment publisher workflow](PR_INLINE_COMMENT_PUBLISHER_WORKFLOW.md)
- [PR inline comment publisher proposal](PR_INLINE_COMMENT_PUBLISHER_PROPOSAL.md)
- [Lane tracker source-of-truth model](lanes/README.md)
- [Lane 1 evidence spine tracker](lanes/LANE_1_EVIDENCE_SPINE.md)
- [Lane 1 evidence accuracy tracker](lanes/LANE_1_EVIDENCE_ACCURACY.md)
- [Lane 1 evidence quality leadership tracker](lanes/LANE_1_EVIDENCE_QUALITY_LEADERSHIP.md)
- [Lane 1 user-visible output evidence tracker](lanes/LANE_1_USER_VISIBLE_OUTPUT_EVIDENCE.md)
- [Lane 1 TypeScript preview completion tracker](lanes/LANE_1_TYPESCRIPT_PREVIEW_COMPLETION.md)
- [TypeScript enablement lane](lanes/TYPESCRIPT_ENABLEMENT.md)
- [Lane 2 policy readiness tracker](policy/POLICY_READINESS.md)
- [Lane 3 editor/LSP tracker](lanes/LANE_3_EDITOR_LSP.md)
- [Lane 4 PR / CI review cockpit tracker](lanes/LANE_4_PR_CI_REVIEW.md)

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
