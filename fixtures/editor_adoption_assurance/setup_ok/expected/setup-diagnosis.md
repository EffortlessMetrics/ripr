# RIPR setup diagnosis

Status: setup is compatible and the Rust editor cockpit can project saved-workspace evidence.

Compatibility: extension and server protocol are compatible; supported artifact schemas include first-pr and receipt packets.

Workspace: single root `fixtures/editor_adoption_assurance/setup_ok/workspace`.

First PR packet: missing; no first-pr handoff has been generated yet.

Receipt: missing; no repair movement receipt has been emitted yet.

Next safe action: run saved-workspace analysis, verify, receipt, and first-pr before opening PR review.

Limits: advisory static projection only; not a gate decision; not runtime proof; no source edits, generated tests, provider calls, or mutation execution.
