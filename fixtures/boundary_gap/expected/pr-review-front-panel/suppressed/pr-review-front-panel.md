# RIPR PR Review

Status: advisory

Start here:
- State: baseline_only
- Source: gate_decision
- Identity: suppressed
- File: src/suppressed.rs:6
- Repair route: not_available
- Class: weakly_exposed
- Current evidence strength: weakly_exposed
- Missing discriminator: suppressed == 6
- Verify command: not_available
- Receipt: receipt_not_applicable
- Boundary: static advisory evidence only; not runtime proof, coverage adequacy, mutation confirmation, gate approval, or merge approval.

Movement:
- New policy-eligible gaps: 0
- Acknowledged gaps: 0
- Suppressed gaps: 1
- Static movement: not available
- Coverage/grip: not available

Policy:
- Decision: suppressed
- Finding remains visible as a durable policy exception
- Gate authority: fixtures/boundary_gap/expected/pr-review-front-panel/suppressed/gate-decision.json

Artifacts:
- Start here: fixtures/boundary_gap/expected/pr-review-front-panel/suppressed/pr-review-front-panel.md
- Policy: fixtures/boundary_gap/expected/pr-review-front-panel/suppressed/gate-decision.json

Limits:
- Static RIPR evidence only.
- Does not run mutation testing.
- Does not edit source or generate tests.
- Gate evaluator remains pass/fail authority.
