# RIPR setup diagnosis

Status: receipt is stale; repair proof actions are suppressed.

Compatibility: server is compatible, but the receipt is older than the evidence it claims.

Workspace: single root `fixtures/editor_adoption_assurance/stale_receipt/workspace`.

First PR packet: not projected until fresh receipt state is available.

Receipt: stale at `target/ripr/agent/agent-receipt.json`.

Next safe action: refresh saved-workspace evidence and rerun verify/receipt before copying repair proof.

Limits: advisory static projection only; not a gate decision; not runtime proof; no source edits, generated tests, provider calls, or mutation execution.
