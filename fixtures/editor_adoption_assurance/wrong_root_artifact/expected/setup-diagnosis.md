# RIPR setup diagnosis

Status: wrong-root artifact; repair actions are suppressed.

Compatibility: server is compatible, but saved artifacts belong to a different root.

Workspace: active root is `fixtures/editor_adoption_assurance/wrong_root_artifact/workspace`; artifacts report `fixtures/other-repo`.

First PR packet: wrong-root state at `target/ripr/first-pr/start-here.json`.

Receipt: wrong-root state at `target/ripr/agent/agent-receipt.json`.

Next safe action: regenerate first-pr and receipt artifacts for the active workspace root.

Limits: advisory static projection only; not a gate decision; not runtime proof; no source edits, generated tests, provider calls, or mutation execution.
