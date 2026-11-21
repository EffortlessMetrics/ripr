# RIPR setup diagnosis

Status: ambiguous multi-root workspace; repair actions are suppressed.

Compatibility: server is compatible, but root-scoped artifact projection is unsafe until one root is selected.

Workspace: multiple roots are open and no safe active root is selected.

First PR packet: not projected because the workspace root is ambiguous.

Receipt: not projected because the workspace root is ambiguous.

Next safe action: select one workspace root before copying root-scoped repair actions.

Limits: advisory static projection only; not a gate decision; not runtime proof; no source edits, generated tests, provider calls, or mutation execution.
