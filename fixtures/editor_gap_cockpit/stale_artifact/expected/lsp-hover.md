**ripr** gap decision

## Evidence boundary

- Artifact state: stale
- Action: refresh before acting

## Gap state

- state: `stale_artifact`
- diagnostics projected: `0`
- repairability: `not_projected`

## Why this matters

The gap record no longer matches the saved workspace snapshot, so repair
commands and related-test paths are withheld.

## Repair route

No repair route is projected for stale artifacts.

## Verify and receipt

- refresh: `ripr refresh`

## Limits

- Stale artifacts fail closed.
- Projection-only editor surface; no source edits, generated tests, provider calls, or mutation execution.
