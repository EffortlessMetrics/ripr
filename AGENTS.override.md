# Codex Repository Instructions

Read `AGENTS.md` at the repository root before modifying the repository. It is
internally coherent and remains the authority for product contract, evidence,
review, merge, release, and cleanup rules.

Codex procedures live under `.agents/skills/**`. Use the narrowest applicable
procedure:

```text
high-level outcome  → deliver-goal
selected claim/issue or existing PR → deliver-pr
missing or stale issue premise → prepare-issue
missing or weak oracle → prepare-proof
implementation/hardening → build-candidate
published or existing PR → finish-pr
```

Do not route Codex through `.claude/skills/**`; Claude has its own complete
provider tree under `CLAUDE.md` and `.claude/skills/**`.
