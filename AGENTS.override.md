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
substantive exact-head inspection → review-pr
published or existing PR convergence → finish-pr
```

- `review_route:root_to_review_pr`

A candidate that appears ready moves through `review-pr` before `finish-pr` may
arm merge. Reading automated comments, seeing no unresolved threads, or seeing
green CI is remote triage, not the substantive current-head review.

Do not route Codex through `.claude/skills/**`; Claude has its own complete
provider tree under `CLAUDE.md` and `.claude/skills/**`.
