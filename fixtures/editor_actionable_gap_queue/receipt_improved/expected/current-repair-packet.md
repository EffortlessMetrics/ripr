RIPR current repair packet

Task
Review the repaired canonical gap: gap:rust:pricing:discount:threshold-boundary.

Context
- Language: rust (stable)
- Receipt movement: improved
- Related test: tests/pricing.rs::premium_customer_gets_discount

Repair
No additional edit is implied by the receipt; refresh the queue before selecting another repair.

Verification
Run cargo xtask evidence-quality-audit.

Receipt
Keep target/ripr/agent/agent-receipt.json with improved static movement.

Stop conditions
- Stop if the receipt canonical gap id no longer matches the queue.
- Stop if refreshed evidence changes the top gap.

Do not do
- Do not claim runtime adequacy from improved static movement.
- Do not claim gate authority, mutation coverage, or merge readiness.
