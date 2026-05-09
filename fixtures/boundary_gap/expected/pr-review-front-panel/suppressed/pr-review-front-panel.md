# RIPR PR Review

Status: advisory

Top issue:
- File: src/suppressed.rs:6
- Class: weakly_exposed
- Missing discriminator: suppressed == 6

Movement:
- New policy-eligible gaps: 0
- Acknowledged gaps: 0
- Suppressed gaps: 1
- Static movement: not available
- Coverage/grip: not available

Policy:
- Decision: suppressed
- Finding remains visible as a durable policy exception
- Gate authority: fixtures/boundary_gap/expected/pr-evidence-ledger/mixed/gate-decision.json

Artifacts:
- Start here: fixtures/boundary_gap/expected/pr-review-front-panel/suppressed/pr-review-front-panel.md
- Policy: fixtures/boundary_gap/expected/pr-evidence-ledger/mixed/gate-decision.json

Limits:
- Static RIPR evidence only.
- Does not run mutation testing.
- Does not edit source or generate tests.
- Gate evaluator remains pass/fail authority.
