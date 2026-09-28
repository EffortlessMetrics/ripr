# RIPR setup diagnosis

Status: receipt gap mismatch.

Workspace: fixtures/editor_first_run_usability/receipt_gap_mismatch/workspace
Server: ripr 0.5.0
Languages: rust enabled.
Artifacts: first useful action found; gap decision ledger found; receipt found for a different gap.

Next safe action: rerun verify and receipt for the current gap before copying repair actions.

Limits: read-only setup diagnosis only; no source edits, generated tests, provider calls, mutation execution, or gate decision.
