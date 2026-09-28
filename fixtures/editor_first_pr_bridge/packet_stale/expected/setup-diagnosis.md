# RIPR setup diagnosis

Status: first-pr packet is stale.

First PR packet: stale at `target/ripr/first-pr/start-here.json`.

Next safe action: refresh saved-workspace evidence and rerun
`cargo xtask first-pr` before acting.

Limits: read-only editor projection only; no source edits, generated tests,
provider calls, mutation execution, gate decisions, PR comments, or merge
readiness claims.
