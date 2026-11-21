# RIPR setup diagnosis

Status: top repairable first-pr gap available.

First PR packet: found at `target/ripr/first-pr/start-here.json` for
`gap:rust:pricing:discount:threshold-boundary`.

Next safe action: inspect the diagnostic, copy the first-pr repair packet,
write one focused test, run verify, emit a receipt, and refresh.

Limits: read-only editor projection only; no source edits, generated tests,
provider calls, mutation execution, gate decisions, PR comments, or merge
readiness claims.
