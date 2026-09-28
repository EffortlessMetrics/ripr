**ripr** gap decision

## Evidence boundary

- Language: rust
- Status: stable
- Action: refresh only

## Gap state

- state: `no_actionable_gap`
- diagnostics projected: `0`
- repairability: `not_actionable`

## Why this matters

The saved workspace has no actionable gap to repair. The editor should not
invent a packet or suggest a test.

## Repair route

No repair route is projected.

## Verify and receipt

- refresh: `ripr refresh`

## Limits

- No-action artifacts fail closed to refresh-only actions.
- Projection-only editor surface; no source edits, generated tests, provider calls, or mutation execution.
