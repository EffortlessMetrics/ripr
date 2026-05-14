**ripr** gap decision

## Evidence boundary

- Workspace root: current root
- Artifact root: different root
- Action: refresh only

## Gap state

- state: `wrong_root`
- diagnostics projected: `0`
- repairability: `not_projected`

## Why this matters

The report was produced for another workspace root, so paths and commands must
not be trusted in this editor session.

## Repair route

No repair route is projected for wrong-root artifacts.

## Verify and receipt

- refresh: `ripr refresh`

## Limits

- Wrong-root artifacts fail closed.
- Projection-only editor surface; no source edits, generated tests, provider calls, or mutation execution.
