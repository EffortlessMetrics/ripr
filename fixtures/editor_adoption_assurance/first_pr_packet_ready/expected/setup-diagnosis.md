# RIPR setup diagnosis

Status: first-pr packet has a top repairable Rust gap.

Compatibility: extension and server protocol are compatible; first-pr and receipt artifact schemas are supported.

Workspace: single root `fixtures/editor_adoption_assurance/first_pr_packet_ready/workspace`.

First PR packet: top repairable gap `gap:rust:pricing:discount:threshold-boundary` at `target/ripr/first-pr/start-here.json`.

Receipt: missing; movement proof has not been emitted yet.

Next safe action: inspect the diagnostic, copy the first-pr repair packet, verify, emit a receipt, then refresh.

Limits: advisory static projection only; not a gate decision; not runtime proof; no source edits, generated tests, provider calls, or mutation execution.
