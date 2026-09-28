# RIPR setup diagnosis

Status: language disabled by config.

Workspace: fixtures/editor_first_run_usability/language_disabled/workspace
Server: ripr 0.5.0
Languages: none enabled.
Artifacts: ignored for diagnostics while languages are disabled.

Next safe action: edit `ripr.toml` `[languages] enabled`, then restart the server.

Limits: read-only setup diagnosis only; no source edits, generated tests, provider calls, mutation execution, or gate decision.
