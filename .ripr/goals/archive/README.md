# Goal manifest archive

`.ripr/goals/active.toml` always names exactly one active campaign — the one
the agent or operator should be executing now. When a campaign closes, the
manifest moves here so the active file does not become a graveyard.

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
  -> close campaign
  -> archive copy moves here under YYYY-MM-DD-<campaign-id>.toml
  -> closeout handoff in docs/handoffs/YYYY-MM-DD-<campaign-id>-closeout.md
  -> next campaign manifest replaces active.toml
```

The archive is read-only history. Do not edit archived manifests after the
campaign closes; future behavior changes belong in their own specs,
proposals, and campaigns.

## Agent neutrality

The active manifest is the centralized execution surface for any agent or
operator runner — Codex, Kiro, Claude Code, Cursor, or a generic agent. The
file name and schema are repository property; external runners consume it
but do not define it.
