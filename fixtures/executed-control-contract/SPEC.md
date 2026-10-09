# Executed-control contract corpus

Given a repository-owned obligation/result vocabulary for acceptance items
that require an executed discriminating control.

When a packet is validated and projected to JSON and Markdown.

Then:

- `passed` requires executed-control evidence for the named wrong implementation
- ordinary positive tests, review prose, and structural claims cannot satisfy
- `not_run`, `not_proven`, `substituted`, and `instrument_failure` stay explicit
- substitutes cannot be inferred after the fact
- #3858 / #4063 remains `not_proven` and is not rewritten as `passed`
- JSON and human projections are deterministic and independent of input order

Must Not:

- inspect live GitHub
- enforce merge eligibility
- claim runtime mutation outcomes
- treat structural discrimination as execution
