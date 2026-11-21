# RIPR setup diagnosis

Status: first-pr packet mismatch; repair actions are suppressed.

Compatibility: server is compatible, but the packet gap identity does not match the current diagnostic.

Workspace: single root `fixtures/editor_adoption_assurance/first_pr_packet_mismatch/workspace`.

First PR packet: mismatch between diagnostic gap `gap:rust:pricing:discount:threshold-boundary` and packet gap `gap:rust:shipping:tax-boundary`.

Receipt: gap mismatch; movement proof does not belong to the current diagnostic.

Next safe action: regenerate first-pr and receipt artifacts for the current diagnostic gap.

Limits: advisory static projection only; not a gate decision; not runtime proof; no source edits, generated tests, provider calls, or mutation execution.
