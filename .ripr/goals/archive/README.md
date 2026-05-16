# Goal manifest archive

`.ripr/goals/active.toml` names the current execution campaign. Its top-level
`status` is `active` while work is being executed and `closed` after closeout
when no successor campaign has been selected yet. When a campaign closes, a
copy moves here so the active file does not become a graveyard.

## Naming

```text
.ripr/goals/archive/YYYY-MM-DD-<campaign-id>.toml
```

`YYYY-MM-DD` is the date the campaign closed. `<campaign-id>` matches the
campaign id in [`docs/IMPLEMENTATION_CAMPAIGNS.md`](../../../docs/IMPLEMENTATION_CAMPAIGNS.md).

## Lifecycle

```text
proposed campaign
  -> copy into .ripr/goals/active.toml
  -> execute work items (one PR each)
  -> close campaign and set top-level status = "closed"
  -> copy closed manifest here under YYYY-MM-DD-<campaign-id>.toml
  -> closeout handoff in docs/handoffs/YYYY-MM-DD-<campaign-id>-closeout.md
  -> next campaign manifest replaces active.toml
```

The archive is read-only history after the closeout copy is corrected to its
final closed state. Future behavior changes belong in their own specs,
proposals, and campaigns.

## Agent neutrality

The active manifest is the centralized execution surface for any agent or
operator runner - Codex, Kiro, Claude Code, Cursor, or a generic agent. The
file name and schema are repository property; external runners consume it
but do not define it.
