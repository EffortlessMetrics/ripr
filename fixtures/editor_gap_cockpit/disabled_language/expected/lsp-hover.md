**ripr** gap decision

## Evidence boundary

- Language: python
- Status: disabled by config
- Action: refresh or enable the language explicitly

## Gap state

- state: `disabled_language`
- diagnostics projected: `0`
- repairability: `not_projected`

## Why this matters

The artifact may contain Python preview evidence, but the current workspace
configuration does not enable Python editor projection.

## Repair route

No repair route is projected while the language is disabled.

## Verify and receipt

- refresh: `ripr refresh`

## Limits

- Disabled-language evidence fails closed.
- Projection-only editor surface; no source edits, generated tests, provider calls, or mutation execution.
