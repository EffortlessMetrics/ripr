# RIPR setup diagnosis

Status: server version mismatch; repair actions are suppressed.

Compatibility: extension expects `ripr 0.6.x`, but the resolved server reports `ripr 0.5.0`; first-pr packet schema support is unsupported.

Workspace: single root `fixtures/editor_adoption_assurance/server_version_mismatch/workspace`.

First PR packet: not projected because the active server cannot validate the required schema.

Receipt: not projected because compatibility is unsafe.

Next safe action: use a compatible ripr server before copying repair, verify, receipt, or first-pr actions.

Limits: advisory static projection only; not a gate decision; not runtime proof; no source edits, generated tests, provider calls, or mutation execution.
