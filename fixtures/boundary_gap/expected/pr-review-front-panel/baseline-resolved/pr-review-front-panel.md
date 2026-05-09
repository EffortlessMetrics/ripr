# RIPR PR Review

Status: advisory

Top issue:
- File: src/gone.rs:2
- Class: weakly_exposed
- Missing discriminator: gone == 2
- Suggested focused test: assert_eq!(gone(), 2)
- Related test: tests/gone.rs::boundary

Movement:
- New policy-eligible gaps: 1
- Baseline gaps still present: 2
- Baseline gaps resolved: 3
- Static movement: resolved
- Coverage/grip: not available

Policy:
- Mode: baseline-check
- Decision: advisory
- Gate authority: not configured

Artifacts:
- Start here: fixtures/boundary_gap/expected/pr-review-front-panel/baseline-resolved/pr-review-front-panel.md
- Evidence: fixtures/boundary_gap/expected/baseline-debt-delta/mixed/baseline-debt-delta.json
- Evidence: fixtures/boundary_gap/expected/pr-evidence-ledger/mixed/pr-evidence-ledger.json

Limits:
- Static RIPR evidence only.
- Does not run mutation testing.
- Does not edit source or generate tests.
- Gate evaluator remains pass/fail authority.
