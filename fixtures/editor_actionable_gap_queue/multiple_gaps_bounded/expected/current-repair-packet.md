RIPR current repair packet

Task
Repair the top canonical gap only: gap:rust:pricing:discount:threshold-boundary.

Context
- Language: rust (stable)
- Evidence boundary: queue has multiple actionable gaps; upstream order chooses the top gap
- Related test: tests/pricing.rs::premium_customer_gets_discount
- Target assertion shape: exact boundary assertion for amount == threshold

Repair
Strengthen the top related test before moving to lower-ranked queue entries.

Verification
Run cargo xtask evidence-quality-audit.

Receipt
Emit or refresh target/ripr/agent/agent-receipt.json for this canonical gap.

Stop conditions
- Stop if the queue order or canonical gap id changes after refresh.
- Stop if another gap becomes the top repair.

Do not do
- Do not broaden scope to multiple repairs in one packet.
- Do not claim gate authority, runtime adequacy, or mutation coverage.
