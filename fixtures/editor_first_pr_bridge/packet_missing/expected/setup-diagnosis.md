# RIPR setup diagnosis

Status: first-pr packet missing.

First PR packet: missing at `target/ripr/first-pr/start-here.json`.

Next safe action: run `cargo xtask first-pr` after verify and receipt artifacts
exist for the current workspace.

Limits: read-only editor projection only; no source edits, generated tests,
provider calls, mutation execution, gate decisions, PR comments, or merge
readiness claims.
