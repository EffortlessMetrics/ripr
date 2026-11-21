# RIPR setup diagnosis

Status: preview adapter unavailable; repair actions are suppressed.

Compatibility: server is compatible for Rust, but the enabled Python preview adapter is unavailable.

Workspace: single root `fixtures/editor_adoption_assurance/preview_adapter_unavailable/workspace`.

First PR packet: not projected because preview-language evidence is unavailable.

Receipt: not projected because the selected preview language cannot produce safe evidence.

Next safe action: use a ripr build with the preview adapter or disable that preview language.

Limits: preview evidence is opt-in, syntax-first, advisory, static-limit labeled, and not Rust-level confidence; not a gate decision; not runtime proof; no source edits, generated tests, provider calls, or mutation execution.
