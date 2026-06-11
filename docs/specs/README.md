# Specs

Specs define externally meaningful behavior for `ripr`. They are the source of
truth for the spec-test-code traceability loop.

Use specs for behavior that users, integrations, or future agents need to rely
on. Keep implementation details in architecture docs or ADRs unless they affect
observable behavior.

## Index

| Spec | Status | Topic |
| --- | --- | --- |
| [RIPR-SPEC-0001](RIPR-SPEC-0001-static-exposure-loop.md) | accepted | Static exposure loop |
| [RIPR-SPEC-0002](RIPR-SPEC-0002-fixture-laboratory.md) | accepted | Fixture laboratory |
| [RIPR-SPEC-0003](RIPR-SPEC-0003-agent-context.md) | planned | Agent context packet |
| [RIPR-SPEC-0004](RIPR-SPEC-0004-test-efficiency.md) | planned | Test efficiency and vacuity signals |
| [RIPR-SPEC-0005](RIPR-SPEC-0005-repo-seam-inventory.md) | proposed | Repo seam inventory and test grip |
| [RIPR-SPEC-0006](RIPR-SPEC-0006-mutation-calibration.md) | proposed | Mutation calibration reports |
| [RIPR-SPEC-0007](RIPR-SPEC-0007-repository-configuration.md) | proposed | Repository configuration |
| [RIPR-SPEC-0008](RIPR-SPEC-0008-sarif-ci-policy.md) | proposed | SARIF and CI policy |
| [RIPR-SPEC-0009](RIPR-SPEC-0009-defaults-first-adoption.md) | proposed | Defaults-first adoption |
| [RIPR-SPEC-0010](RIPR-SPEC-0010-agent-working-set-brief.md) | proposed | Agent working-set brief |
| [RIPR-SPEC-0011](RIPR-SPEC-0011-llm-work-loop.md) | proposed | LLM work loop |
| [RIPR-SPEC-0012](RIPR-SPEC-0012-pr-test-guidance.md) | proposed | PR test guidance annotations |
| [RIPR-SPEC-0013](RIPR-SPEC-0013-recommendation-calibration-report.md) | proposed | Recommendation calibration report |
| [RIPR-SPEC-0014](RIPR-SPEC-0014-calibrated-gate-policy.md) | proposed | Calibrated gate policy |
| [RIPR-SPEC-0015](RIPR-SPEC-0015-evidence-health-baseline.md) | proposed | Evidence health baseline |
| [RIPR-SPEC-0016](RIPR-SPEC-0016-baseline-debt-delta.md) | proposed | Baseline debt delta |
| [RIPR-SPEC-0017](RIPR-SPEC-0017-ripr-zero-reporting.md) | proposed | RIPR Zero reporting |
| [RIPR-SPEC-0018](RIPR-SPEC-0018-pr-evidence-ledger.md) | proposed | PR evidence ledger |
| [RIPR-SPEC-0019](RIPR-SPEC-0019-test-oracle-assistant-loop.md) | proposed | Test-oracle assistant loop |
| [RIPR-SPEC-0020](RIPR-SPEC-0020-first-useful-action-report.md) | proposed | First useful action report |
| [RIPR-SPEC-0021](RIPR-SPEC-0021-evidence-record.md) | proposed | Evidence record |
| [RIPR-SPEC-0022](RIPR-SPEC-0022-assistant-loop-health-report.md) | proposed | Assistant loop health report |
| [RIPR-SPEC-0023](RIPR-SPEC-0023-pr-review-front-panel-report.md) | proposed | PR review front panel report |
| [RIPR-SPEC-0024](RIPR-SPEC-0024-report-packet-index.md) | proposed | Report packet index |
| [RIPR-SPEC-0025](RIPR-SPEC-0025-pr-inline-comment-publisher.md) | proposed | PR inline comment publisher |
| [RIPR-SPEC-0026](RIPR-SPEC-0026-language-adapter-contract.md) | accepted | Language adapter contract |
| [RIPR-SPEC-0027](RIPR-SPEC-0027-typescript-preview-static-facts.md) | accepted | TypeScript preview static facts |
| [RIPR-SPEC-0028](RIPR-SPEC-0028-python-preview-static-facts.md) | proposed | Python preview static facts |
| [RIPR-SPEC-0029](RIPR-SPEC-0029-policy-readiness-report.md) | proposed | Policy readiness report |
| [RIPR-SPEC-0030](RIPR-SPEC-0030-preview-evidence-policy-boundary.md) | proposed | Preview evidence policy boundary |
| [RIPR-SPEC-0031](RIPR-SPEC-0031-lane1-evidence-quality-audit.md) | proposed | Lane 1 evidence quality audit |
| [RIPR-SPEC-0032](RIPR-SPEC-0032-lane1-evidence-quality-fixtures.md) | proposed | Lane 1 evidence quality failure fixtures |
| [RIPR-SPEC-0033](RIPR-SPEC-0033-match-arm-canonical-gap-discriminators.md) | proposed | Match-arm canonical gap discriminators |
| [RIPR-SPEC-0034](RIPR-SPEC-0034-evidence-quality-scorecard.md) | proposed | Evidence quality scorecard |
| [RIPR-SPEC-0035](RIPR-SPEC-0035-evidence-quality-benchmark-corpus.md) | proposed | Evidence quality benchmark corpus |
| [RIPR-SPEC-0036](RIPR-SPEC-0036-editor-preview-routing.md) | proposed | Editor preview routing |
| [RIPR-SPEC-0037](RIPR-SPEC-0037-editor-preview-static-limit-projection.md) | proposed | Editor preview static-limit projection |
| [RIPR-SPEC-0038](RIPR-SPEC-0038-generated-pr-ci-review-workflow.md) | proposed | Generated PR CI review workflow |
| [RIPR-SPEC-0039](RIPR-SPEC-0039-policy-operations-report.md) | proposed | Policy operations report |
| [RIPR-SPEC-0040](RIPR-SPEC-0040-static-runtime-confidence-expansion.md) | proposed | Static/runtime confidence expansion |
| [RIPR-SPEC-0041](RIPR-SPEC-0041-policy-history-ledger.md) | proposed | Policy history ledger |
| [RIPR-SPEC-0042](RIPR-SPEC-0042-policy-promotion-packets.md) | proposed | Policy promotion packets |
| [RIPR-SPEC-0043](RIPR-SPEC-0043-presentation-text-evidence.md) | proposed | Presentation text evidence |
| [RIPR-SPEC-0044](RIPR-SPEC-0044-preview-evidence-promotion-packet.md) | proposed | Preview evidence promotion packet |
| [RIPR-SPEC-0045](RIPR-SPEC-0045-finding-to-gap-alignment.md) | proposed | Finding-to-gap alignment |
| [RIPR-SPEC-0046](RIPR-SPEC-0046-gap-decision-ledger.md) | proposed | Gap decision ledger |
| [RIPR-SPEC-0047](RIPR-SPEC-0047-editor-gap-projection.md) | accepted | Editor gap projection |
| [RIPR-SPEC-0048](RIPR-SPEC-0048-config-policy-constant-evidence.md) | proposed | Config and policy constant evidence |
| [RIPR-SPEC-0049](RIPR-SPEC-0049-editor-setup-status.md) | accepted | Editor setup status |
| [RIPR-SPEC-0050](RIPR-SPEC-0050-editor-first-repair-loop.md) | accepted | Editor first repair loop |
| [RIPR-SPEC-0051](RIPR-SPEC-0051-first-successful-pr-ux.md) | accepted | First successful PR UX |
| [RIPR-SPEC-0052](RIPR-SPEC-0052-editor-first-pr-packet-projection.md) | accepted | Editor first-pr packet projection |
| [RIPR-SPEC-0053](RIPR-SPEC-0053-start-here-surface-convergence.md) | accepted | Start-here surface convergence |
| [RIPR-SPEC-0054](RIPR-SPEC-0054-editor-adoption-assurance.md) | accepted | Editor adoption assurance |
| [RIPR-SPEC-0055](RIPR-SPEC-0055-editor-actionable-gap-queue.md) | accepted | Editor actionable gap queue |
| [RIPR-SPEC-0056](RIPR-SPEC-0056-public-actionable-projection.md) | accepted | Public actionable projection |
| [RIPR-SPEC-0057](RIPR-SPEC-0057-ripr-swarm-repair-loop.md) | accepted | RIPR swarm repair loop |
| [RIPR-SPEC-0058](RIPR-SPEC-0058-ripr-swarm-external-agent-handoff.md) | accepted | RIPR swarm external agent handoff |
| [RIPR-SPEC-0059](RIPR-SPEC-0059-actionable-surface-translation.md) | accepted | Actionable surface translation |
| [RIPR-SPEC-0060](RIPR-SPEC-0060-source-of-truth-stack.md) | accepted | Source-of-truth stack |
| [RIPR-SPEC-0061](RIPR-SPEC-0061-lane1-canonical-actionability-contract.md) | proposed | Lane 1 canonical actionability contract |
| [RIPR-SPEC-0062](RIPR-SPEC-0062-cross-language-oracle-graph.md) | proposed | Cross-language oracle graph |
| [RIPR-SPEC-0063](RIPR-SPEC-0063-cross-language-evidence-router-ux.md) | proposed | Cross-language evidence router UX |
| [RIPR-SPEC-0064](RIPR-SPEC-0064-perl-fact-packet-contract.md) | proposed | Perl fact packet contract |
| [RIPR-SPEC-0065](RIPR-SPEC-0065-evidence-to-repair-use-case-roadmap.md) | proposed | Evidence-to-repair use-case roadmap |
| [RIPR-SPEC-0066](RIPR-SPEC-0066-repo-badge-use-case.md) | proposed | Repo badge use case |
| [RIPR-SPEC-0067](RIPR-SPEC-0067-pr-gate-use-case.md) | proposed | PR gate use case |
| [RIPR-SPEC-0068](RIPR-SPEC-0068-pr-review-card-use-case.md) | proposed | PR review-card use case |
| [RIPR-SPEC-0069](RIPR-SPEC-0069-lsp-agent-feedback-use-case.md) | proposed | LSP agent feedback use case |
| [RIPR-SPEC-0070](RIPR-SPEC-0070-downstream-review-consumer-use-case.md) | proposed | Downstream review consumer use case |
| [RIPR-SPEC-0071](RIPR-SPEC-0071-typescript-bun-evidence-use-case.md) | proposed | TypeScript/Bun evidence use case |
| [RIPR-SPEC-0072](RIPR-SPEC-0072-large-repo-diff-first-use-case.md) | proposed | Large-repo diff-first use case |
| [RIPR-SPEC-0073](RIPR-SPEC-0073-receipts-outcomes-route-quality-use-case.md) | proposed | Receipts, outcomes, and route quality use case |
