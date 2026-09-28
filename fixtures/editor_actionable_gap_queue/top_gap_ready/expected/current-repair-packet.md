RIPR current repair packet

Task
Repair this one canonical gap: gap:rust:pricing:discount:threshold-boundary.

Context
- Language: rust (stable)
- Evidence boundary: static/advisory actionable gap queue
- Related test: tests/pricing.rs::premium_customer_gets_discount
- Target assertion shape: exact boundary assertion for amount == threshold

Repair
Strengthen the related test assertion for the equality boundary.

Verification
Run cargo xtask evidence-quality-audit.

Receipt
Emit or refresh target/ripr/agent/agent-receipt.json after verification.

Stop conditions
- Stop if the canonical gap id, related test, or verify command is stale.
- Stop if the repair would require broad production edits.

Do not do
- Do not generate tests automatically.
- Do not call providers.
- Do not claim gate authority, runtime adequacy, or mutation coverage.
