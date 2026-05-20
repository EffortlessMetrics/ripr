RIPR current repair packet

Task
Recheck the unchanged canonical gap: gap:rust:pricing:discount:threshold-boundary.

Context
- Language: rust (stable)
- Receipt movement: unchanged
- Related test: tests/pricing.rs::premium_customer_gets_discount

Repair
Inspect whether the assertion still misses the equality-boundary discriminator.

Verification
Run cargo xtask evidence-quality-audit.

Receipt
Refresh target/ripr/agent/agent-receipt.json after any follow-up repair.

Stop conditions
- Stop if the receipt canonical gap id no longer matches the queue.
- Stop if static evidence cannot name a repair route.

Do not do
- Do not treat unchanged static movement as runtime failure.
- Do not claim gate authority, mutation coverage, or merge readiness.
