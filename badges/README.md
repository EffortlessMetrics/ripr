# Badge endpoints

This directory contains generated Shields endpoint JSON used by README badges.
The public `ripr` / `ripr+` endpoints are user-actionable repair counters, not
seam inventory. Detailed basis and inventory reports stay in `target/`.

Regenerate:

```bash
cargo xtask badges
```

Check drift:

```bash
cargo xtask badges --check
```

Only committed `*.json` endpoint files are public badge surfaces. Detailed
reports stay in CI artifacts and `target/`.

## Public badge vocabulary (RIPR-SPEC-0066)

The public `ripr` / `ripr+` badge renders exactly one closed message,
combined with the label to read as `ripr: <message>`:

| Message | Meaning |
| --- | --- |
| `ripr: 0 actionable` | A full, current repo-scoped run found zero unresolved canonical actionable gaps. |
| `ripr: N actionable` | A full, current repo-scoped run found `N` unresolved canonical actionable gaps. |
| `ripr: limited` | The source run did not complete fully (`run_status` is a `limited_*` state); no count is safe to publish. |
| `ripr: stale` | The source report or endpoint exceeds the configured maximum age; the last count is no longer claimed as current. |
| `ripr: unknown` | No consumable source report exists, or the only basis is raw findings; no count is claimed. |

The projection fails closed: a degraded input resolves to `unknown`, `stale`,
or `limited` (precedence `unknown > stale > limited > count`) with a named
reason in the badge sidecar — never a silent clean count. `ripr: 0 actionable`
claims only zero unresolved canonical actionable gaps under a full, current,
repo-scoped run; it makes no oracle-completeness, runtime-mutation, or
coverage claim, and excludes preview-language (TypeScript/Bun, Perl) evidence.
The machine-readable projection (state plus `run_status`, `generated_at`,
`actionable_count`, `limited_reason`, `stale_age_secs`, `source_report`)
travels in the native badge JSON's `public_projection` object; see
[docs/OUTPUT_SCHEMA.md](../docs/OUTPUT_SCHEMA.md).
